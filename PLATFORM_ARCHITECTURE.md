# Windows / macOS 平台能力边界

本轮目标是双平台架构与 macOS 可运行原型。Windows 现有原生实现保留；macOS 功能必须通过实机验收后才能宣称正式支持。

## 依赖方向

React / Tauri commands → 听写、助手、学习流程 → platform 能力接口 → Windows 或 macOS 原生实现。

- `platform::desktop()` 是进程级、不可变的工厂入口，按编译目标选择 `DesktopBackend`。业务层不判断操作系统，不解释原生句柄。
- `TargetAccess` 定义目标有效性、焦点判断和恢复契约。`prepare_target` 以失败关闭策略恢复并验证焦点；实际注入前再次检查，拒绝向未知目标发送按键。
- `InputTarget` 是不透明且可比较的会话目标。Windows 内部映射 HWND；macOS 内部映射 AX 应用、窗口及输入元素。不得持久化目标或通过前端传入任意原生 ID。
- 热键服务和音频控制通过平台模块选择具体实现，保留现有调用外观。初期避免同时重写成熟 Windows 热键状态机。
- ASR、LLM、TNL、词库规则、历史记录与音频编码保持共用。

## 原生实现

### Windows

`platform/windows/` 保存原来的 `GetAsyncKeyState` 热键服务、`SendInput` / 焦点管理、带 COM/超时保护的 UIA 读取和 Audio Session 静音管理。除 UIA 的模块路径外，原生实现本轮不改变。

### macOS

`native.m` 是一个小型 Objective-C C ABI 桥接层，用 AppKit、ApplicationServices、Core Graphics 和 AVFoundation；不引入 sidecar 进程。Rust 侧只传不透明 token、文本和状态。

- AX 引用由 ARC 持有，所有 AX 操作在专用串行队列执行，设置每次消息超时。
- NSWorkspace 激活在主队列调度，使用已有安装、禁止新实例及版本替换；异步回调检查原进程与目标仍存在，Rust 在截止时间内验证应用、窗口和输入元素，而不是只比较 PID。
- 对原生窗口标签，保留 AX 标签引用，按身份在窗口直属标签组内查找；切换标签不等于文档关闭。标签已移除或选择失败时仍停止自动输入。隐藏应用恢复、原生标签切换和独立窗口切换均已实机通过。
- 全屏可能隐藏原生标签组：仅在原窗口仍处于应用窗口列表、全屏属性为真、没有可见标签组、原窗口及输入元素仍被该应用选中时允许继续；实际输入仍须通过全局应用焦点校验。该补充分支的原生回归已通过，`50b570bb` 包两次全屏实测分别回填 77、78 字；悬浮窗全屏显示仍未验证。
- 原生目标注册表最多保留 128 个不同目标，token 单调增长、不复用；失效/被淘汰的目标停止自动插入。安全文本字段拒绝捕获和读取。
- 热键使用被动 Core Graphics session event tap，按顺序保存键状态，避免轮询遗漏短按边沿；事件消费、状态机与业务回调分离。500ms 权限检查和键状态校正处理撤销授权及丢失释放；队列溢出丢弃过期事件并重置。按住、切换、锁定、优先级和恢复场景有纯逻辑测试，原生事件消费者另有不发送系统按键的独立测试。
- 文本读取当前支持目标输入元素的 AXValue。未暴露该属性的应用跳过学习，不使用全选/复制来打断用户。
- 复制/粘贴发送 Cmd+C / Cmd+V。等待用户释放物理修饰键，不伪造释放用户仍按住的键。
- Overlay 设置跨 Spaces / fullscreen auxiliary；尚未替换 Tauri 底层窗口为 NSPanel，跨应用全屏行为仍需实机验证。
- 普通启动显示主窗口，`--minimized` 保持静默启动；Dock 的 reopen 事件恢复主窗口。这些窗口生命周期行为封装在 Mac 适配层。
- 其他应用静音明确标记不可用，设置页禁用。不得用系统总静音冒充此能力。Core Audio taps 留待独立验证。

## 权限与配置

`get_platform_status` 返回支持能力和当前权限状态；`request_platform_permission` 仅由用户点击授权按钮调用。启动服务先检查权限，不提前初始化录音/ASR。前端窗口重新获得焦点时刷新权限，并提供手动刷新。Mac 权限页在三项权限齐全后启用“启动服务”，复用现有服务控制器；权限不完整时按钮禁用。

macOS 首次使用需要麦克风、辅助功能、输入监控权限。系统可能要求重新启动应用；授权不自动等于服务已启动。发布包必须稳定 bundle identifier 和签名身份，以免升级后权限失效。

本地 ad-hoc 调试包重新构建后，系统设置可能仍显示辅助功能已开启，而 `AXIsProcessTrusted` 返回未授权。本轮实机日志确认过旧、新 cdhash 不匹配；关闭应用，在系统设置中移除该应用的旧辅助功能条目，再添加当前 .app 后恢复正常。仅切换开关或重复添加已有条目不一定更新签名记录。不要用绕过权限检查的方法处理此情况。

输入监控请求也可能只打开设置页而未自动登记应用；可用设置页的“+”选择当前 .app，再按系统提示退出并重开。

Tauri 的 `macos-private-api` feature 与 `app.macOSPrivateApi` 同时在公共配置声明（构建脚本校验公共 dependency 的 features），仅 macOS 使用其透明窗口能力。Mac 打包配置在 `tauri.macos.conf.json`，Windows 继续 NSIS。

Tauri JS API / Rust runtime 固定在 2.11.x，updater 两端固定在 2.12.x，避免 Cargo 自动升级 minor 后与 npm lockfile 不匹配。升级时应同步更新两端并重新验证两个平台。

现有快捷键数据仍使用 `meta_left/right`、`alt_left/right` 等平台中立键名；显示名称在前端适配为 Win/Alt 或 Cmd/Option。现有用户配置不重置。默认 Ctrl+Meta 和 Alt/Option+Space 可以修改，Fn/Globe 暂未增加。

## 开发与测试

Windows 按原流程构建。macOS 需要 Xcode Command Line Tools、Rust、Node 和 Opus 构建依赖。

```sh
npm ci
# 使用 CMake 3.x；CMake 4 可设置下文的兼容变量，或预装静态 Opus。
npm run test:ts
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

已有 Homebrew Opus 时，可为本机同架构验证设置 `OPUS_LIB_DIR=$(brew --prefix opus)` 和 `OPUS_STATIC=1`。发行构建应从源码构建 Opus，确保最低系统版本/架构一致；不得把开发机 Homebrew dylib 当作用户运行时依赖。

Windows CI 使用 CMake 4 时，为旧版 `audiopus_sys` 内置 Opus 项目设置环境变量 `CMAKE_POLICY_VERSION_MINIMUM=3.5`（PowerShell：`$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"`）。这是 CMake 提供的旧项目策略兼容设置，不关闭豆包输入法或 Opus；该设置下 Windows 编译及测试已在 CI 通过。

本机原型打包命令（ad-hoc 签名，关闭更新产物；不用于正式分发）：

```sh
OPUS_LIB_DIR=$(brew --prefix opus) OPUS_STATIC=1 npm run tauri build -- --debug --bundles app --config '{"bundle":{"createUpdaterArtifacts":false,"macOS":{"signingIdentity":"-"}}}'
```

输出位于 `src-tauri/target/debug/bundle/macos/PushToTalk.app`。Mac 配置由 Tauri 自动合并。

剪贴板集成测试标为 ignored，因为会修改操作系统剪贴板；在专用测试会话显式运行。核心自动测试不读取第三方应用数据、不录音、不模拟全局输入。

`.github/workflows/platform-check.yml` 提供 Windows/macOS 编译与测试矩阵，不发布版本。原 release workflow 仍只发布 Windows。正式 Mac 发布还需要 Developer ID 签名、公证、更新产物、Intel/Apple Silicon 架构策略和升级验证；本轮不修改正式发布渠道。

该矩阵也验证默认功能的 debug 原型打包：Windows 生成 NSIS 安装包，macOS 生成 ad-hoc 签名的 `.app`，检查签名及 Homebrew 动态库依赖后压缩保存。构建只合并 `.github/tauri.prototype.conf.json`，不启用 ATDD，也不生成更新产物。Actions 附件保留 7 天，供验收使用，不属于正式发布；打包成功不等于安装、升级或 Gatekeeper 验收通过。

## 验收清单

- 两平台：按住松开、切换、锁定结束/取消、热键配置过程中暂停监听、快速重复操作。
- 输入：原窗口关闭、窗口切换、目标应用多个窗口、焦点恢复失败、修饰键仍按住、粘贴后剪贴板恢复、富文本/非文本剪贴板（现有文本级恢复仍有限制）。
- Mac：首次授权、拒绝、撤销、系统要求重启；Safari/Chrome/VS Code/TextEdit/常用聊天应用；全屏/Spaces、多屏与不同缩放；设备断开和蓝牙麦克风切换。
- 学习：同目标去重、旧任务取消、失焦停止观察、AXValue 不支持时跳过、超时后主流程仍可继续。
- 分发：包内麦克风声明、静态 Opus、签名身份稳定、干净 Mac 安装与升级。

尚未完成上述实机矩阵前，本实现是原型，不承诺跨应用兼容性或 Windows 零回归。

## 本轮验证记录（2026-09-25）

- 基于远端 main 的 `725aab0`，在独立 `codex/macos-platform` worktree 开发。
- TypeScript 最新 86 项在 Windows/macOS CI 均通过，前端 production build 均通过；新增 LF/CRLF 参数化用例覆盖 Windows checkout 换行。
- Apple Silicon Mac 默认功能（含豆包输入法 Opus）`cargo check` 通过；Rust library 210 项通过、4 项桌面剪贴板测试忽略，另有 6 + 6 + 3 项独立纯逻辑测试入口通过（与 library 中测试重复），16 项原生验收通过。
- debug .app 打包及本机 ad-hoc 签名验证通过；检查动态链接列表，无 Homebrew 动态库依赖。
- Windows 原生四个模块逐字对比保留（UIA 仅调整模块引用路径）；`9d70b71` 的 Windows/macOS CI 均通过 Rust 编译与自动测试，见 [Platform checks #36144098644](https://github.com/yyyzl/push-2-talk/actions/runs/36144098644)。Windows 桌面行为回归仍未执行。
- 解锁后实机启动成功；修复了首次启动仅隐藏窗口、没有 Mac Dock reopen 处理的问题。原生 WebView 的权限页、Mac 键名和不支持功能的禁用状态已确认。关闭到后台后自动化重新访问时，主窗口恢复、服务继续运行且进程未重复；未单独验收鼠标点击 Dock。
- 麦克风、辅助功能、输入监控三项授权正确读取并进入运行中，真实录音和豆包输入法识别多轮成功。最新完成实机验证的 `d4079775` 包连续三轮分别回填 80、85、83 字，其中隐藏应用、切换原生标签都正确恢复；独立窗口切换另回填 79 字，旁路文档未改变。关闭目标防误写再次通过。辅助功能撤销后停止录音并保留结果，恢复后再回填 78 字；系统剪贴板纯文本前后相等。取消录音、无语音错误反馈此前已通过。全屏新缺陷及修复状态详见 `MACOS_ATDD.md`，物理全局热键仍未获实机证据。
- 空识别被错误记为成功的问题已通过测试复现并修正：普通听写入口拒绝空白，阻止空粘贴和成功历史；正常文本及原始错误保持不变。另补充 Mac PostEvent 权限校验，拒绝时不误报按键已发送。两项修复均已打入测试包；无语音实测通过，PostEvent 实测为允许。不能把旧验收的目标准备问题归因于发送权限。
- 全屏修复包 `50b570bb` 的实机回归通过全屏输入、隐藏后恢复、连续输入和关闭目标防误写。完整 Rust 串行回归、16 项原生验收、默认 Rust 编译、默认前端 production build 均通过；测试音频已暂停，应用与三项权限保留可用。完整双平台 ATDD 尚未全绿，具体未覆盖项在验收报告中列明。
- `836237b` 的 [双平台完整 CI](https://github.com/yyyzl/push-2-talk/actions/runs/36145662582) 通过，包含默认功能的 Windows NSIS 与 Mac `.app` 打包；Mac 签名及动态依赖检查也通过。验收包可在该运行的附件中下载，保留 7 天，未发布版本。另在当前本机包实测快捷键配置取消及非法键位拒绝后，真实听写仍可回填 80 个非空白字符。

## 改动量与后续工作

这是中等偏大的系统集成改造，共用的 ASR、LLM、TNL 与业务规则可以保留。当前新增的平台边界、Mac 原生实现、原生测试及权限/平台前端约 1,600 行（含纯逻辑测试，另有验收驱动与构建配置），同时搬迁四个 Windows 原生模块并调整听写、助手、学习等调用点。行数仅说明本轮规模，不代表正式适配已完成。

下一阶段的主要工作是跨应用输入与 AX 兼容性验证、全屏/Spaces 窗口体验、音频设备与权限生命周期、Windows 实机回归；“静音其他应用”及签名、公证、双架构分发应作为独立事项推进。具体工期取决于目标应用矩阵和首轮实机发现的问题，不宜仅凭编译通过承诺完成时间。

助手模式驱动和经用户授权的本地 LLM 配置已补齐。TextEdit 的真实问答、选区翻译、标签恢复、关闭保护和静音反馈已通过。无录音诊断定位了 Chrome 垂直标签的实际深度，修复后真实录音中的跨标签恢复、回填前关闭保护和 TextEdit 回归均已通过，双平台 CI 也通过。物理快捷键、悬浮窗/全屏视觉、Windows 桌面行为及分发等矩阵仍未完成，具体证据见 `MACOS_ATDD.md`。本地配置及凭据不进入仓库或 CI 原型。
