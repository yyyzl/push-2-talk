# PushToTalk

**把说的话输入到当前应用，也可以用语音修改选中的文字。**

PushToTalk 是面向 Windows 和 macOS 的开源桌面语音输入工具。按下快捷键开始录音，结束后进行语音识别，并把结果写回原来的输入位置；需要时，可以交给大语言模型润色、翻译或回答问题。

[下载版本](https://github.com/yyyzl/push-2-talk/releases/latest) · [更新记录](https://github.com/yyyzl/push-2-talk/releases) · [反馈问题](https://github.com/yyyzl/push-2-talk/issues) · [开发文档](docs/development/README.md)

## 能做什么

- **日常听写**：按住说话、松开结束，或使用按一次开始、再按一次结束的模式。识别结果可以直接输入，也可以先经过语句润色。
- **语音处理文本**：选中一段文字，说“翻译成英文”“整理成三点”等指令。助手通过结果面板展示回答，支持继续对话、复制和回填。
- **切换识别服务**：支持豆包输入法、千问、豆包语音和 SiliconFlow SenseVoice。千问可以选择 Audio 3.1、Audio 3.0 或旧版 Qwen3，并保留已有模型配置。
- **按场景配置模型**：接入 OpenAI 兼容的 LLM 服务，分别为润色、助手和词库学习选择提供商；支持自定义提示词、连接测试和模型适配的思考选项。
- **个人词库与纠词学习**：维护人名、产品名、专业术语和纠错关系；开启学习后，根据本轮输入后的修改给出建议。文本读取能力取决于目标应用。
- **使用反馈与记录**：录音悬浮窗、提示音、托盘快捷切换，以及最近 50 条本地历史。助手可按配置使用联网搜索。

语音识别和 LLM 使用你选择的在线服务。软件开源不代表第三方服务永久免费；可用性、配额和计费以服务商为准。

## 安装

从 [GitHub Releases](https://github.com/yyyzl/push-2-talk/releases) 选择系统对应的安装包，具体文件和版本说明以发布页面为准。

| 平台 | 安装包 | 首次使用 |
| --- | --- | --- |
| Windows 10 / 11，64 位 | `*_x64-setup.exe`，NSIS 安装器 | 安装后按项目现有方式以管理员身份运行 |
| macOS 12 及以上，Apple Silicon | `*_aarch64.dmg` | 拖入“应用程序”，完成麦克风、辅助功能、输入监控授权 |
| macOS 12 及以上，Intel | `*_x64.dmg` | 选择 Intel 包，并完成上述三项授权 |

### Windows

运行安装器完成安装，然后启动 PushToTalk。全局热键和跨应用输入沿用当前的管理员运行方式。更新使用 NSIS 安装包，不提供 MSI。

### macOS

打开 DMG，把 PushToTalk 拖入“应用程序”后再启动。根据应用中的权限提示，在系统设置里允许：

- **麦克风**：录制语音。
- **辅助功能**：读取选区、恢复输入目标并回填文字。
- **输入监控**：监听全局快捷键。

完成授权后回到应用刷新权限状态，再启动服务；系统要求重开时，请完全退出后重新打开。

**当前 macOS 分发采用 ad-hoc 签名，未做 Apple Developer ID 签名与公证。** 首次打开可能被系统拦截。确认安装包来自本仓库后，可按 [Apple 的打开说明](https://support.apple.com/zh-cn/102445)，在“隐私与安全性”中处理该应用的打开许可。升级后也可能需要重新授权；如果系统显示已开启、应用却仍报告未授权，可在对应权限列表中移除旧条目，再添加“应用程序”中的当前版本。

## 第一次使用

1. **选择识别引擎。** 默认是豆包输入法，首次使用自动获取设备凭据，无需手填 API Key；服务不可用时可以改用其他引擎。
2. **配置需要的服务。** 千问和 SenseVoice 使用各自的 API Key；豆包语音使用 App ID 与 Access Token。只有需要润色、助手或模型辅助学习时，才需要配置相应 LLM。
3. **检查快捷键与权限。** 先用一段普通文本试用，确认不会与系统或其他软件的快捷键冲突。
4. **保存并启动服务。** 有些页面自动保存，有些选项需要点击保存，以页面状态提示为准。
5. **把光标放回目标输入框。** 按快捷键说一句话，结束后等待识别与回填。

默认快捷键如下；已有配置会继续保留，也可以在“快捷键”页面修改。

| 操作 | Windows | macOS |
| --- | --- | --- |
| 听写 | 左 `Ctrl` + 左 `Win` | 左 `Control` + 左 `Command` |
| AI 助手 | 左 `Alt` + `Space` | 左 `Option` + `Space` |
| 松手模式 | `F2` | `F2`，部分键盘需要同时按 `Fn` |

macOS 暂未提供独立的 Fn / Globe 快捷键绑定。若功能键被系统占用，可以直接改用其他组合键。

### 识别引擎怎么选

| 引擎 | 支持的方式 | 配置说明 |
| --- | --- | --- |
| 豆包输入法 | 实时 | 自动获取凭据；当前不配置备用引擎 |
| 千问 Qwen | HTTP、实时 | 配置 DashScope API Key，选择模型；两种方式有各自对应的模型 |
| 豆包语音 Doubao | HTTP、实时 | 配置相应服务的 App ID 与 Access Token |
| SiliconFlow SenseVoice | HTTP | 配置 SiliconFlow API Key，也可在支持的组合中用作备用引擎 |

第三方服务的错误或额度不足会影响识别。可以先切换引擎或连接方式，再排查对应账号和网络；不要反复清空本地配置。

## 升级与数据

Windows 旧版可以使用新安装器覆盖升级，应用包含配置迁移逻辑。此次整合保留已有的识别服务、模型选择、快捷键和 LLM 配置，不要求重新初始化。旧版兼容验证基线为 v1.6.1，验证范围见 [Windows 验收记录](docs/development/windows-validation-handoff.md)。

升级前建议完全退出应用，并备份数据目录；跨平台复制配置或降级并不等同于同平台覆盖升级，当前不承诺自动完成这些迁移。

| 数据 | Windows | macOS |
| --- | --- | --- |
| 配置、个人词库、纠词等 | `%APPDATA%\PushToTalk\` | `~/Library/Application Support/PushToTalk/` |
| 历史记录 | WebView 本地存储，位于 `%LOCALAPPDATA%\com.pushtotalk.app\` | WebView 本地存储，独立于上述配置文件 |

**只备份 `config.json` 不等于备份全部数据。** 个人词库还有 SQLite 文件，历史记录保存在 WebView 中，最近记录最多保留 50 条。备份 Windows 数据时应同时保留上表中的两个目录。

若升级或切换后历史暂时为空，先完全退出应用，再重新打开确认；仍未恢复时，保留原目录并反馈问题，不要先清空数据或反复重装。

API Key 等凭据保存在本地配置中。识别音频会发送到所选 ASR 服务；启用润色、助手、学习判断或联网搜索时，相关文本或查询会发送到相应服务。分享日志或配置前请移除密钥、个人文本与其他敏感信息。

## 平台差异与已知限制

Windows 与 macOS 共用配置和语音处理逻辑，通过各自的原生接口处理快捷键、焦点、剪贴板和文本输入。

- **录音时静音其他应用**目前仅 Windows 支持，macOS 对应选项不可用。
- macOS 已有 TextEdit、Chrome、Safari 等场景的实机验收，但全屏悬浮窗、多屏、设备切换以及各应用的文本读取兼容性仍需继续验证。
- 自动回填依赖目标窗口和输入控件仍然有效；焦点无法确认、目标关闭或安全控件拒绝读取时，不能保证自动插入。
- 词库学习依赖应用暴露可读取的文本，不是所有编辑器和网页输入框都支持。
- 更新包的 Tauri 签名与 Apple 的应用签名、公证是两套机制；有更新签名不代表 macOS 包已获 Apple 公证。

具体通过项和未覆盖项见 [平台架构与验证范围](PLATFORM_ARCHITECTURE.md) 和 [macOS 实机验收](MACOS_ATDD.md)。

## 本地开发

前端使用 React、TypeScript、Vite 和 Tailwind CSS；桌面层使用 Tauri 2 与 Rust。请在目标系统上编译和测试，不用单个平台的构建结果代替另一平台验证。

需要 Node.js 22、Rust stable，以及系统原生构建工具：Windows 使用 Visual Studio C++ Build Tools、WebView2 和 CMake；macOS 使用 Xcode Command Line Tools 和 CMake。使用仓库锁文件安装依赖。

```sh
git clone https://github.com/yyyzl/push-2-talk.git
cd push-2-talk
npm ci
npm run tauri dev
```

Windows 开发时沿用管理员运行方式；macOS 开发包也需要系统权限。`npm run dev` 仅启动前端，不能代替桌面原生功能验证。

常用检查：

```sh
npm run test:ts
npx playwright install chromium
npm run test:ui
npm run build
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --features cli-tools --manifest-path src-tauri/Cargo.toml
```

构建本地验证包（不生成自动更新签名产物）：

```sh
npm run tauri build -- --config .github/tauri.prototype.conf.json -- --locked
```

产物位于 `src-tauri/target/release/bundle/`；Windows 为 NSIS，macOS 为 `.app` 和 DMG。macOS 配置由 Tauri 自动合并，本地验证包使用 ad-hoc 签名。正式发布需要另外生成更新签名，流程见 [发布工作流](.github/workflows/release.yml)。

Opus 构建使用 CMake；仓库已设置旧版 Opus 对 CMake 4 的兼容选项。更多原生依赖、macOS 静态链接和验收命令见 [平台开发说明](PLATFORM_ARCHITECTURE.md#开发与测试)。

### 代码与文档入口

| 目录 / 文档 | 内容 |
| --- | --- |
| `src/pages/`、`src/components/`、`src/hooks/` | 主窗口页面、组件和前端状态 |
| `src/windows/` | 录音悬浮窗、学习通知和助手结果面板 |
| `src-tauri/src/application/`、`pipeline/` | 录音生命周期、听写与助手流程 |
| `src-tauri/src/platform/` | Windows / macOS 原生能力 |
| `src-tauri/src/asr/`、`tnl/`、`learning/` | 识别、文本规范化和词库学习 |
| `tests/`、`scripts/` | 回归测试、构建与验收工具 |
| [开发文档索引](docs/development/README.md) | 配置、数据库、事件、ASR 和窗口约定 |
| [贡献约定](AGENTS.md) | 代码风格、测试要求和提交规范 |

## 反馈与贡献

欢迎通过 [Issues](https://github.com/yyyzl/push-2-talk/issues) 提交问题或建议。问题报告请附系统与架构、应用版本、识别引擎、目标应用，以及可复现的步骤；涉及输入失败时，请说明使用的是听写还是助手模式。

提交代码前运行与改动相关的测试。涉及快捷键、剪贴板或跨应用输入的修改，请标明实际验证的平台和场景。

本项目使用 [MIT License](LICENSE)。
