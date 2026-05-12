# AI 助手异步结果面板 — 技术设计

## Summary

将 AI 助手模式的结果投递从同步自动粘贴改为异步结果面板。后端 pipeline 完成后不再直接调用 text_inserter，而是将结果存入 AppState 并通过 Tauri 事件通知前端。前端新增 ResultPanelWindow 渲染 Markdown 并提供操作按钮。

## Architecture

### 改造前

```
AssistantPipeline
  → LLM 完成
  → hide_overlay_and_restore_focus(target_hwnd)
  → insert_text_with_context(result, has_selection, clipboard_guard)
  → emit("transcription_complete")
```

### 改造后

```
AssistantPipeline
  → 捕获选中文本后立即恢复剪贴板（ClipboardGuard.restore()）
  → LLM 完成
  → hide_overlay()（不恢复焦点）
  → 存入 AppState.pending_assistant_result
  → emit("assistant_result_ready", payload)
  → ResultPanelWindow 弹出
  → 用户点击「粘贴到原窗口」
    → invoke("paste_assistant_result")
    → restore_focus(target_hwnd) → clipboard write → Ctrl+V
  → emit("transcription_complete")
```

### 系统组件图

```
┌──────────────────┐    assistant_result_ready     ┌──────────────────────┐
│   Rust Backend   │ ──────────────────────────→   │  ResultPanelWindow   │
│                  │                               │  (新增 Tauri 窗口)    │
│  AppState {      │    paste_assistant_result      │                      │
│    pending_result│ ←────────────────────────────  │  [粘贴] [复制] [关闭] │
│  }               │                               │                      │
│                  │    copy_assistant_result       │  Markdown 渲染       │
│  pipeline/       │ ←────────────────────────────  │  react-markdown      │
│   assistant.rs   │                               │                      │
│                  │    dismiss_assistant_result    │                      │
│  focus.rs        │ ←────────────────────────────  │                      │
│  win32_input.rs  │                               └──────────────────────┘
└──────────────────┘
```

## Key Components

### 1. 后端：PendingAssistantResult 状态

**文件**: `src-tauri/src/lib.rs` (AppState 扩展)

```rust
struct PendingAssistantResult {
    id: String,
    result_text: String,
    instruction: String,           // ASR 转写的语音指令
    selected_text: Option<String>, // 发起时的选中文本
    has_selection: bool,
    target_hwnd: Option<isize>,
    asr_time_ms: u64,
    llm_time_ms: u64,
    created_at: Instant,
}

// AppState 新增字段
pending_assistant_result: Arc<Mutex<Option<PendingAssistantResult>>>
```

### 2. 后端：pipeline/assistant.rs 改造

**改造点**: 移除末尾的 `insert_result()` 调用，改为存储 + 发事件

```rust
// 改造前
let inserted = Self::insert_result(&result, has_selection, clipboard_guard);

// 改造后
let pending = PendingAssistantResult {
    id: nanoid::nanoid!(),
    result_text: result.clone(),
    instruction: asr_instruction.clone(),
    selected_text: context.selected_text.clone(),
    has_selection,
    target_hwnd,
    asr_time_ms,
    llm_time_ms: llm_start.elapsed().as_millis() as u64,
    created_at: Instant::now(),
};

// 存入 AppState
{
    let mut lock = state.pending_assistant_result.lock().unwrap();
    *lock = Some(pending.clone());
}

// 通知前端
app.emit("assistant_result_ready", AssistantResultPayload::from(&pending))?;

// 显示结果面板窗口
show_result_panel_window(app).await;
```

### 3. 后端：剪贴板即时释放改造

**文件**: `src-tauri/src/pipeline/assistant.rs`

```rust
// 改造前：clipboard_guard 传到 pipeline 末尾
let pipeline_result = pipeline.process(
    &app, processor, clipboard_guard, ...
).await;

// 改造后：捕获选中文本后立即释放
let (selected_text, _guard_dropped) = {
    let (guard, text) = clipboard_manager::get_selected_text()?;
    let text = text.clone();
    // guard 在此 scope 结束时 drop，自动恢复剪贴板
    (text, ())
};

// pipeline 不再接收 clipboard_guard
let pipeline_result = pipeline.process(
    &app, processor, /* 无 guard */ ...
).await;
```

### 4. 后端：新增 Tauri Commands

**文件**: `src-tauri/src/lib.rs`

```rust
/// 粘贴结果到原窗口
#[tauri::command]
async fn paste_assistant_result(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let pending = {
        state.pending_assistant_result.lock().unwrap().take()
    }.ok_or("无待处理的结果")?;

    // 检查目标窗口是否仍有效
    if let Some(hwnd) = pending.target_hwnd {
        if win32_input::is_window_valid(hwnd) {
            // 恢复焦点 + 粘贴
            hide_result_panel_window(&app).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            win32_input::restore_focus_with_verify(hwnd, 3);
            tokio::time::sleep(Duration::from_millis(100)).await;
            // 使用已有的 insert_text_with_context（guard 传 None，不需要恢复剪贴板）
            clipboard_manager::insert_text_with_context(&pending.result_text, pending.has_selection, None)?;
            // 触发学习观察等后续流程...
            return Ok("已粘贴".into());
        }
    }

    // 降级：窗口无效，复制到剪贴板
    // 注意：clipboard_manager 中没有 copy_to_clipboard，需新增此辅助函数
    clipboard_manager::copy_to_clipboard(&pending.result_text)?;
    hide_result_panel_window(&app).await;
    Ok("原窗口已关闭，已复制到剪贴板".into())
}

/// 复制结果到剪贴板
#[tauri::command]
async fn copy_assistant_result(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pending = state.pending_assistant_result.lock().unwrap();
    if let Some(ref p) = *pending {
        // 注意：需新增 clipboard_manager::copy_to_clipboard 辅助函数
        clipboard_manager::copy_to_clipboard(&p.result_text)?;
    }
    Ok(())
}

/// 关闭结果面板
#[tauri::command]
async fn dismiss_assistant_result(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.pending_assistant_result.lock().unwrap().take();
    hide_result_panel_window(&app).await;
    Ok(())
}
```

### 5. 前端：ResultPanelWindow

**新增文件**:
- `src/windows/ResultPanelWindow.tsx` — 主组件
- `src/result-panel-main.tsx` — React 入口
- `result-panel.html` — HTML 入口

**新增依赖**:
```json
{
  "react-markdown": "^9.0.0",
  "remark-gfm": "^4.0.0",
  "react-syntax-highlighter": "^15.6.1",
  "@types/react-syntax-highlighter": "^15.5.13"
}
```

### 6. Tauri 窗口配置

**文件**: `src-tauri/tauri.conf.json`

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
  "focusable": true
}
```

## Data / Control Flow

### 完整时序图

```
User                 Backend                    ResultPanelWindow
 │                      │                              │
 │ Alt+Space            │                              │
 │─────────────────────→│                              │
 │                      │ GetForegroundWindow()        │
 │                      │ save target_hwnd             │
 │                      │                              │
 │ 说话...释放按键       │                              │
 │─────────────────────→│                              │
 │                      │ get_selected_text()          │
 │                      │ ★ 立即恢复剪贴板              │
 │                      │                              │
 │                      │ ASR 转写                      │
 │                      │ TNL 规范化                    │
 │                      │ AssistantProcessor.process() │
 │                      │ ... (可能很慢) ...            │
 │                      │                              │
 │  (用户切到其他窗口)    │                              │
 │                      │                              │
 │                      │ LLM 完成                      │
 │                      │ hide_overlay()                │
 │                      │ 存入 pending_result           │
 │                      │ emit("assistant_result_ready")│
 │                      │──────────────────────────────→│
 │                      │ show_result_panel_window()    │
 │                      │                              │ 渲染 Markdown
 │                      │                              │ 显示操作按钮
 │                      │                              │
 │                      │  用户点击「粘贴到原窗口」       │
 │                      │←─────────────────────────────│
 │                      │ invoke("paste_assistant_result")
 │                      │                              │
 │                      │ hide_result_panel()           │
 │                      │ restore_focus(saved_hwnd)     │
 │                      │ clipboard write + Ctrl+V      │
 │←─────────────────────│                              │
 │  文本被插入           │                              │
 │                      │ emit("transcription_complete")│
 │                      │ 触发学习观察                   │
```

### 事件 Payload 类型

```typescript
// 前端类型定义
interface AssistantResultPayload {
  id: string;
  result_text: string;       // LLM 输出（Markdown 格式）
  instruction: string;       // 用户语音指令
  selected_text?: string;    // 原始选中文本
  has_selection: boolean;
  asr_time_ms: number;
  llm_time_ms: number;
}
```

## UI 设计

### 布局结构

```
┌─────────────────────────────────────────────────┐
│  🤖 AI 助手结果          耗时 29.5s    ⊗ Close │  ← 自定义标题栏 (可拖动)
├─────────────────────────────────────────────────┤
│                                                 │
│  💬 "把这段翻译成英文"                            │  ← 指令区 (折叠)
│  📎 原文: "你好世界，这是一段测试..."              │  ← 选中文本摘要
│                                                 │
├─────────────────────────────────────────────────┤
│                                                 │
│  ┌─────────────────────────────────────────┐   │
│  │                                         │   │
│  │  ## Hello World                         │   │  ← Markdown 渲染区
│  │                                         │   │     (可滚动)
│  │  This is a translation of the text:     │   │
│  │                                         │   │
│  │  > Hello world, this is a test...       │   │
│  │                                         │   │
│  │  ```python                              │   │
│  │  print("hello world")                   │   │
│  │  ```                                    │   │
│  │                                         │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
├─────────────────────────────────────────────────┤
│                                                 │
│   [📋 复制]                 [📌 粘贴到原窗口]    │  ← 操作栏
│                                                 │
└─────────────────────────────────────────────────┘
```

### 视觉规范

| 元素 | 亮色 | 暗色 |
|------|------|------|
| 窗口背景 | `var(--paper)` #FAF9F5 | `var(--ink)` #141413 |
| 标题栏背景 | `var(--sand)` #E8E6DC | #1E1E1D |
| 内容区背景 | white | #1A1A19 |
| 代码块背景 | #F5F4F0 | #2A2A28 |
| 主操作按钮 | `var(--crail)` #D97757 文字白 | 同 |
| 次操作按钮 | `var(--sand)` 边框 | #333 边框 |
| 指令文字 | `var(--stone-dark)` | #888 |
| 正文字体 | Lora / Noto Serif SC | 同 |
| 代码字体 | JetBrains Mono | 同 |

### 窗口尺寸策略

- 默认: 520 × 620
- 最小: 400 × 300
- 可调整大小
- 首次居中显示
- 内容区自适应高度（有最大高度限制，超出滚动）

## Risks and Edge Cases

### R1: 目标窗口已关闭

- **检测**: `win32_input::is_window_valid(hwnd)`
- **降级**: 复制到剪贴板 + 在结果面板显示提示 "原窗口已关闭，已复制到剪贴板"

### R2: 连续发起多个请求

- **V1 策略**: 新结果覆盖旧结果
- 旧结果未处理时，新请求完成会替换面板内容
- 可在面板标题区简短提示 "新结果已覆盖上次内容"

### R3: 结果面板获得焦点后影响粘贴

- 粘贴流程: 先 `hide_result_panel` → 等 50ms → `restore_focus(target_hwnd)` → 等 100ms → `Ctrl+V`
- 隐藏面板后再恢复目标焦点，避免面板抢焦点

### R4: 有选中文本时的粘贴行为

- 恢复焦点后，原窗口的文本选中状态可能已丢失
- **方案**: 如果 `has_selection = true`，先模拟全选原区域（但这不可靠）
- **实际方案**: 直接在光标位置插入，不保证替换原选中文本
- 这是异步模式的固有限制，需在 UI 中提示用户

### R5: Markdown 内容包含特殊字符

- react-markdown 默认安全，不执行 HTML
- 不需要额外的 sanitize 处理

### R6: 超长结果内容

- 内容区限制最大高度并启用滚动
- 复制功能复制完整文本（不截断）

## Testing Strategy

### 手动测试矩阵

| 场景 | 验证点 |
|------|--------|
| 快速模型（< 3s） | 结果面板正常弹出，操作按钮可用 |
| 慢速模型（> 30s） | 等待期间用户可正常操作，面板弹出后可粘贴 |
| 无选中文本（Q&A 模式） | 指令区不显示"原文"，粘贴在光标位置 |
| 有选中文本（文本处理） | 指令区显示原文摘要 |
| 目标窗口已关闭 | 降级为复制到剪贴板 + 提示 |
| 暗色主题 | 所有元素正确渲染 |
| Markdown 代码块 | 语法高亮正常 |
| 超长内容 | 滚动正常，复制完整 |
| 连续请求 | 新结果覆盖旧结果 |
| 键盘操作 | Enter/Esc 快捷键正常 |

### TypeScript 测试

- Markdown 渲染组件的 props 类型测试
- Payload 类型一致性测试

## Out of Scope

- **LLM 流式响应** — 当前 openai_client.rs 不支持流式，改造是独立的大任务
- **多结果队列** — V1 单结果足够
- **窗口位置记忆** — 每次居中即可
- **自动判断快/慢切换** — 统一走面板
- **听写模式改造** — 听写模式（Normal Pipeline）保持不变
