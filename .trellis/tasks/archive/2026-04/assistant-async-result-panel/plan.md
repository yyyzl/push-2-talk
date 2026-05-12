# AI 助手异步结果面板 — 执行计划

## Inputs

- PRD: `prd.md`
- Design: `info.md`
- Relevant specs:
  - `.trellis/spec/frontend/component-guidelines.md` — 组件约定（未填充，参照现有 NotificationWindow 模式）
  - `.trellis/spec/backend/error-handling.md` — 错误处理约定

## File Map

### Create

- `result-panel.html` — 结果面板窗口 HTML 入口
- `src/windows/result-panel-main.tsx` — React 入口（挂载 ResultPanelWindow）
- `src/windows/ResultPanelWindow.tsx` — 结果面板主组件（Markdown 渲染 + 操作按钮）
- `src/components/MarkdownRenderer.tsx` — 可复用 Markdown 渲染组件（react-markdown + 语法高亮）
- `src/types/assistant-result.ts` — AssistantResultPayload 类型定义
- `tests/assistantResultPanel.test.ts` — 前端类型与逻辑测试

### Modify

- `src-tauri/tauri.conf.json` — 新增 `result_panel` 窗口配置
- `vite.config.ts` — 新增 `result-panel` 多页入口
- `package.json` — 新增 `react-markdown`、`remark-gfm`、`react-syntax-highlighter` 依赖
- `src-tauri/src/lib.rs` — AppState 新增 `pending_assistant_result` 字段；新增 3 个 Tauri commands；注册命令；新增 `show_result_panel_window` 函数
- `src-tauri/src/pipeline/assistant.rs` — 移除 `insert_result()` + `hide_overlay_and_restore_focus()` 调用，改为存结果 + 发事件

### Test

- `tests/assistantResultPanel.test.ts` — payload 类型验证、截断逻辑
- `npm run test:ts` — 运行全部 TS 测试
- `cd src-tauri && cargo check` — Rust 编译检查

## Execution Slices

### Slice 1: 前端基础设施 — 依赖安装 + HTML 入口 + Vite 配置 + 窗口配置

**Goal**

建立结果面板窗口的前端骨架：npm 依赖安装、HTML 入口文件、Vite 多页构建配置、Tauri 窗口声明。此 slice 结束后，`npm run tauri dev` 能识别新窗口（虽然内容为空白）。

**Files**

- `package.json`
- `result-panel.html`
- `src/windows/result-panel-main.tsx`
- `vite.config.ts`
- `src-tauri/tauri.conf.json`

**Steps**

- [ ] 安装 npm 依赖
  - Command: `cd G:/RustProject/push-2-talk && npm install react-markdown remark-gfm react-syntax-highlighter @types/react-syntax-highlighter`

- [ ] 创建 `result-panel.html`
  - 参照 `notification.html` 结构
  - `<div id="result-panel-root"></div>`
  - `<script type="module" src="/src/windows/result-panel-main.tsx"></script>`
  - 包含字体 preconnect、透明背景样式

- [ ] 创建 `src/windows/result-panel-main.tsx`
  - 参照 `src/windows/notification-main.tsx`
  - 导入 `../index.css`
  - 挂载 `ResultPanelWindow` 到 `result-panel-root`（暂用空 div 占位）

- [ ] 修改 `vite.config.ts`
  - 在 `build.rollupOptions.input` 中添加 `"result-panel": resolve(__dirname, "result-panel.html")`

- [ ] 修改 `src-tauri/tauri.conf.json`
  - 在 `app.windows` 数组末尾添加窗口配置：
    ```json
    {
      "label": "result_panel",
      "title": "AI Result",
      "url": "result-panel.html",
      "width": 520,
      "height": 620,
      "minWidth": 400,
      "minHeight": 300,
      "decorations": false,
      "transparent": true,
      "alwaysOnTop": true,
      "skipTaskbar": false,
      "visible": false,
      "resizable": true,
      "center": true,
      "focus": true
    }
    ```

- [ ] 验证构建
  - Command: `cd G:/RustProject/push-2-talk && npx vite build --mode development 2>&1 | head -20`
  - Expected: 构建成功，输出包含 `result-panel.html`

---

### Slice 2: 前端类型 + MarkdownRenderer 组件

**Goal**

定义 `AssistantResultPayload` 类型，实现可复用的 `MarkdownRenderer` 组件（react-markdown + GFM + 语法高亮），并通过测试验证。

**Files**

- `src/types/assistant-result.ts`
- `src/components/MarkdownRenderer.tsx`
- `tests/assistantResultPanel.test.ts`

**Steps**

- [ ] 编写测试（先于实现）
  - File: `tests/assistantResultPanel.test.ts`
  - 测试 1: `AssistantResultPayload` 类型字段完整性 — 静态导入类型并断言必要字段存在
  - 测试 2: `truncateText` 工具函数 — 超过限制长度时截断并添加省略号
  - 测试 3: `formatDuration` 工具函数 — 毫秒转可读格式（如 `1200` → `1.2s`，`65000` → `1m 5s`）

- [ ] 运行测试确认失败
  - Command: `cd G:/RustProject/push-2-talk && npx tsx --test tests/assistantResultPanel.test.ts 2>&1`
  - Expected: 模块找不到错误（文件尚不存在）

- [ ] 创建 `src/types/assistant-result.ts`
  - 定义 `AssistantResultPayload` 接口：
    - `id: string`
    - `result_text: string`
    - `instruction: string`
    - `selected_text?: string`
    - `has_selection: boolean`
    - `asr_time_ms: number`
    - `llm_time_ms: number`
  - 导出 `truncateText(text: string, maxLength: number): string` 工具函数
  - 导出 `formatDuration(ms: number): string` 工具函数

- [ ] 创建 `src/components/MarkdownRenderer.tsx`
  - Props: `content: string`、`className?: string`
  - 使用 `react-markdown` + `remarkGfm` 插件
  - 代码块使用 `react-syntax-highlighter` 的 `Prism` + 亮/暗主题
  - 自定义渲染器：
    - `code` — 区分行内代码和代码块，代码块使用语法高亮
    - `pre` — 代码块容器样式
    - `a` — 链接使用 `var(--steel)` 颜色
    - `table` — 使用项目设计系统的边框颜色
  - 样式遵循设计系统：正文 `font-serif`，代码 `font-mono`，代码块背景 `#F5F4F0` / 暗色 `#2A2A28`

- [ ] 运行测试确认通过
  - Command: `cd G:/RustProject/push-2-talk && npx tsx --test tests/assistantResultPanel.test.ts 2>&1`
  - Expected: 3 个测试全部通过

- [ ] 运行全量 TS 测试确认无回归
  - Command: `cd G:/RustProject/push-2-talk && npm run test:ts 2>&1`
  - Expected: 全部通过

---

### Slice 3: ResultPanelWindow 前端组件

**Goal**

实现完整的 `ResultPanelWindow` 组件：监听 `assistant_result_ready` 事件、渲染上下文信息 + Markdown 内容 + 操作按钮、支持键盘快捷键、支持亮/暗主题。

**Files**

- `src/windows/ResultPanelWindow.tsx`
- `src/windows/result-panel-main.tsx`（更新：引用真实组件）

**Steps**

- [ ] 实现 `ResultPanelWindow.tsx`

  **组件结构**:
  ```
  ResultPanelWindow
  ├── TitleBar（自定义标题栏，可拖动，data-tauri-drag-region）
  │   ├── 图标 + "AI 助手结果"
  │   ├── 耗时显示
  │   └── 关闭按钮 (X)
  ├── ContextSection（上下文信息区）
  │   ├── 语音指令显示
  │   └── 选中文本摘要（truncateText 截断到 100 字符）
  ├── ContentArea（Markdown 渲染区，可滚动）
  │   └── MarkdownRenderer
  └── ActionBar（操作栏）
      ├── 复制按钮（次要，左侧）
      └── 粘贴到原窗口按钮（主要，右侧，crail 色）
  ```

  **事件监听**:
  - `listen("assistant_result_ready", handler)` — 接收结果并更新状态
  - `listen("config_updated", handler)` — 监听主题变化

  **操作处理**:
  - 「粘贴到原窗口」→ `invoke("paste_assistant_result")` → 处理返回值（成功/降级提示）
  - 「复制」→ `invoke("copy_assistant_result")` → 显示 "已复制" 反馈（按钮文字临时变化 2 秒）
  - 「关闭」→ `invoke("dismiss_assistant_result")`

  **键盘快捷键**:
  - `Enter` → 粘贴到原窗口
  - `Escape` → 关闭
  - 不覆盖 `Ctrl+C`，保留 WebView 原生文本选择复制行为
  - 使用 `useEffect` + `window.addEventListener("keydown", handler)`

  **主题支持**:
  - 监听 `config_updated` 事件获取主题
  - 根据 `theme` 值切换 `.theme-dark` 类（参照 OverlayWindow 模式）

  **视觉样式（Tailwind）**:
  - 窗口外壳: `rounded-xl overflow-hidden shadow-2xl`
  - 标题栏: `bg-[var(--sand)]` / 暗色 `bg-[#1E1E1D]`，使用 `data-tauri-drag-region` 支持拖动
  - 内容区: `overflow-y-auto max-h-[400px]`
  - 主按钮: `bg-[var(--crail)] text-white hover:opacity-90`
  - 次按钮: `border border-[var(--sand)] text-[var(--ink)]` / 暗色 `border-[#333]`
  - 图标: 使用 `lucide-react`（Copy, ClipboardPaste, X, MessageSquare, FileText）

- [ ] 更新 `src/windows/result-panel-main.tsx` — 导入真实 `ResultPanelWindow` 替换占位 div

- [ ] 验证 Vite 构建
  - Command: `cd G:/RustProject/push-2-talk && npx vite build --mode development 2>&1 | tail -10`
  - Expected: 构建成功，无 TypeScript 错误

---

### Slice 4: 后端 — AppState 扩展 + 新 Tauri Commands + 窗口管理

**Goal**

在 Rust 后端添加 `PendingAssistantResult` 数据结构、AppState 新字段、3 个新 Tauri commands（paste/copy/dismiss）、以及 `show_result_panel_window` / `hide_result_panel_window` 辅助函数。

**Files**

- `src-tauri/src/lib.rs`
- `src-tauri/src/clipboard_manager.rs`（新增 `copy_to_clipboard` 辅助函数）

**Steps**

- [ ] 在 `clipboard_manager.rs` 中新增 `copy_to_clipboard` 公开函数
  - 签名: `pub fn copy_to_clipboard(text: &str) -> Result<()>`
  - 实现: `arboard::Clipboard::new()?.set_text(text.to_string())?; Ok(())`
  - 位置: 在 `insert_text_with_context` 函数之前
  - 原因: 现有模块只有 `insert_text_with_context`（写入剪贴板 + Ctrl+V），缺少纯粹的"只写入剪贴板"函数

- [ ] 在 `lib.rs` 中定义 `PendingAssistantResult` 结构体和 `AssistantResultPayload` 序列化结构体
  - 位置: 在 `AppState` 定义之前或之后
  - `PendingAssistantResult` 字段:
    - `id: String` — UUID v4
    - `result_text: String`
    - `instruction: String`
    - `selected_text: Option<String>`
    - `has_selection: bool`
    - `target_hwnd: Option<isize>`
    - `asr_time_ms: u64`
    - `llm_time_ms: u64`
  - `AssistantResultPayload`（`#[derive(Clone, serde::Serialize)]`）:
    - 与前端 `AssistantResultPayload` 类型字段对应
    - 不含 `target_hwnd`（不暴露给前端）

- [ ] 在 `AppState` 结构体中添加字段
  - `pending_assistant_result: Arc<Mutex<Option<PendingAssistantResult>>>`
  - 在 `AppState` 初始化处（搜索 `AppState {` 初始化块）添加默认值 `Arc::new(Mutex::new(None))`

- [ ] 实现 `show_result_panel_window` 辅助函数
  - 参照 `show_notification_window` 实现模式
  - 获取 `result_panel` 窗口 → 使用 `find_monitor_at_cursor` 定位 → 居中显示 → `.show()` + `.set_focus()`
  - 与 notification 不同：结果面板需要 `set_focus()` 因为用户需要交互

- [ ] 实现 `hide_result_panel_window` 辅助函数
  - 获取 `result_panel` 窗口 → `.hide()`

- [ ] 实现 `paste_assistant_result` command
  - `#[tauri::command] async fn paste_assistant_result(app: AppHandle, state: State<'_, AppState>) -> Result<String, String>`
  - 从 `pending_assistant_result` 取出（`.take()`）
  - 检查 `target_hwnd` 是否有效（`win32_input::is_window_valid`）
  - 有效: 先隐藏面板窗口 → 等 50ms → `restore_focus_with_verify(hwnd, 3)` → 等 100ms → `insert_text_with_context(text, has_selection, None)` → 触发学习观察 → 发送 `transcription_complete` → 返回 `"已粘贴"`
  - 无效: 复制到剪贴板 → 隐藏面板 → 返回 `"原窗口已关闭，已复制到剪贴板"`

- [ ] 实现 `copy_assistant_result` command
  - `#[tauri::command] async fn copy_assistant_result(state: State<'_, AppState>) -> Result<(), String>`
  - 读取 `pending_assistant_result`（不 take，用户可能还要粘贴）
  - 调用 `clipboard_manager::copy_to_clipboard(&text)`（Slice 4 新增的辅助函数）

- [ ] 实现 `dismiss_assistant_result` command
  - `#[tauri::command] async fn dismiss_assistant_result(app: AppHandle, state: State<'_, AppState>) -> Result<(), String>`
  - 从 `pending_assistant_result` 取出（`.take()`）
  - 隐藏面板窗口

- [ ] 在 `.invoke_handler()` 注册新命令
  - 在命令列表中添加: `paste_assistant_result`, `copy_assistant_result`, `dismiss_assistant_result`

- [ ] 验证 Rust 编译
  - Command: `cd G:/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -5`
  - Expected: 编译通过（可能有未使用 import 的警告，无错误）

---

### Slice 5: 后端 — Assistant Pipeline 改造（核心链路）

**Goal**

改造 `pipeline/assistant.rs`，将末尾的"自动粘贴"替换为"存结果 + 发事件 + 弹窗"。同时改造 `lib.rs` 中的 `handle_assistant_mode` 和 `on_stop` 回调，实现剪贴板即时释放。

**Files**

- `src-tauri/src/pipeline/assistant.rs`
- `src-tauri/src/lib.rs`（`handle_assistant_mode` 函数和 `on_stop` 中的 AI 助手分支）

**Steps**

- [ ] 改造 `pipeline/assistant.rs` 的 `process()` 方法

  **移除（第 125-158 行附近）**:
  - 移除 `super::focus::hide_overlay_and_restore_focus(app, target_hwnd).await` 调用
  - 移除 `Self::insert_result(&result, has_selection, clipboard_guard)` 调用
  - 移除学习观察触发逻辑（学习观察将在 `paste_assistant_result` command 中触发）

  **替换为**:
  - 隐藏 overlay（仅隐藏，不恢复焦点）：`hide_overlay_window(app).await` 或通过事件
  - 构建 `PendingAssistantResult` 对象
  - 存入 `AppState.pending_assistant_result`
  - 通过 `app.emit("assistant_result_ready", payload)` 通知前端
  - 调用 `show_result_panel_window(app).await`

  **修改 `process()` 签名**:
  - 移除 `clipboard_guard: Option<ClipboardGuard>` 参数
  - 新增 `app_state: &State<'_, AppState>` 参数（用于存储 pending result），或者直接通过 `app.state::<AppState>()` 获取

  **修改 `insert_result` 方法**:
  - 保留方法但标记为辅助函数，它将在 `paste_assistant_result` command 中被调用
  - 或者将其内联到 command 中

  **修改返回值**:
  - `PipelineResult.inserted` 改为 `false`（因为不再在 pipeline 内插入）
  - 新增字段或使用现有 `original_text` 字段传递 `instruction`

- [ ] 改造 `lib.rs` 中的剪贴板捕获（`on_stop` 回调，第 2020-2036 行附近）

  **改造前**:
  ```rust
  let (clipboard_guard, selected_text) = match clipboard_manager::get_selected_text() {
      Ok((guard, text)) => (Some(guard), text),
      Err(e) => (None, None),
  };
  // clipboard_guard 传递给 handle_assistant_mode
  ```

  **改造后**:
  ```rust
  let selected_text = match clipboard_manager::get_selected_text() {
      Ok((guard, text)) => {
          // guard 在此 scope 结束时 drop，立即恢复剪贴板
          text
      }
      Err(e) => {
          tracing::warn!("捕获选中文本失败: {}，继续处理但无上下文", e);
          None
      }
  };
  // 不再传递 clipboard_guard
  ```

- [ ] 改造 `handle_assistant_mode` 函数签名
  - 移除 `clipboard_guard: Option<clipboard_manager::ClipboardGuard>` 参数
  - 对应修改 `pipeline.process()` 调用（不传 clipboard_guard）

- [ ] 改造 `handle_assistant_mode` 的结果处理（第 2306-2344 行附近）
  - 移除 `Ok` 分支中的 `hide_overlay_window(&app).await`（已在 pipeline 内完成）
  - 结果事件 `transcription_complete` 的发送时机调整：
    - pipeline 内不再发 `transcription_complete`（因为还没插入）
    - `transcription_complete` 改为在 `paste_assistant_result` command 成功后发送

- [ ] 验证 Rust 编译
  - Command: `cd G:/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -10`
  - Expected: 编译通过

- [ ] 运行全量 TS 测试确认无回归
  - Command: `cd G:/RustProject/push-2-talk && npm run test:ts 2>&1`
  - Expected: 全部通过

---

### Slice 6: 端到端验证 + 边界情况处理

**Goal**

进行端到端手动验证，修复边界情况（目标窗口关闭降级、连续请求覆盖、暗色主题），确保所有验收标准通过。

**Files**

- `src/windows/ResultPanelWindow.tsx`（可能的微调）
- `src-tauri/src/lib.rs`（可能的微调）

**Steps**

- [ ] 手动测试: 基本流程
  - 启动应用（管理员权限）
  - 在记事本中选择文本 → Alt+Space → 说指令 → 等待 → 验证结果面板弹出
  - 验证 Markdown 渲染正确（标题、代码块、列表）
  - 点击"粘贴到原窗口" → 验证记事本中文本被插入
  - AC1 ✓, AC2 ✓

- [ ] 手动测试: 复制功能
  - 触发 AI 助手 → 等待结果 → 点击"复制"
  - 在其他应用中 Ctrl+V → 验证文本正确
  - AC3 ✓

- [ ] 手动测试: 关闭功能
  - 触发 AI 助手 → 等待结果 → 按 Esc
  - 验证面板隐藏
  - AC4 ✓

- [ ] 手动测试: 代码高亮
  - 使用指令 "写一段 Python 代码"
  - 验证代码块有语法高亮
  - AC5 ✓

- [ ] 手动测试: 暗色主题
  - 切换到暗色主题
  - 触发 AI 助手 → 验证面板使用暗色样式
  - AC6 ✓

- [ ] 手动测试: 剪贴板即时释放
  - 复制一段文本 → 触发 AI 助手（选中其他文本）→ 在等待期间尝试 Ctrl+V → 验证粘贴的是之前复制的内容（不是选中的文本）
  - AC7 ✓

- [ ] 手动测试: 目标窗口关闭降级
  - 在记事本中触发 AI 助手 → 等待期间关闭记事本 → 结果面板弹出后点击"粘贴到原窗口"
  - 验证降级为复制到剪贴板 + 显示提示
  - AC8 ✓

- [ ] 处理发现的问题
  - 根据手动测试结果修复 bug
  - 常见问题预判：
    - 面板窗口位置不在屏幕中央 → 调整 `show_result_panel_window` 定位逻辑
    - 粘贴后面板未隐藏 → 检查 `hide_result_panel_window` 时序
    - 键盘快捷键不响应 → 检查 focus 状态和 keydown listener

- [ ] 最终回归验证
  - Command: `cd G:/RustProject/push-2-talk && npm run test:ts 2>&1`
  - Expected: 全部通过
  - Command: `cd G:/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -5`
  - Expected: 编译通过
  - 手动测试听写模式（Normal Pipeline）不受影响

## Risks / Watch Items

- **pipeline/assistant.rs 改造是核心风险点** — Slice 5 改动了 AI 助手模式的关键路径，需要特别小心剪贴板生命周期和焦点恢复时序
- **Tauri 窗口 focus 竞争** — 结果面板 `set_focus()` 后再粘贴时需要先隐藏面板再恢复目标焦点，时序必须正确（隐藏 → 50ms → 恢复焦点 → 100ms → Ctrl+V）
- **react-markdown 与 Tailwind 的样式冲突** — Markdown 渲染的 HTML 标签（h1-h6, p, ul, ol, table）可能被 Tailwind 的 preflight reset 影响，需要在 MarkdownRenderer 中显式指定样式
- **has_selection 异步后失效** — 用户选中文本后离开，再回来粘贴时选区已丢失。设计已决定接受此限制（在光标位置插入）
- **学习观察触发** — 从 pipeline 移到 `paste_assistant_result` command 后，需确保 `start_learning_observation` 的参数正确传递

## Ready-to-Execute Summary

- First slice to start with: **Slice 1**（前端基础设施）
- Blocking dependencies: 无。Slice 1-3 是前端工作，Slice 4 是后端工作，二者可并行。Slice 5 依赖 Slice 4。Slice 6 依赖全部完成。
- 推荐执行顺序: 1 → 2 → 3 → 4 → 5 → 6
- 可并行的 slices: (1, 4) 可并行；(2, 3) 必须串行
