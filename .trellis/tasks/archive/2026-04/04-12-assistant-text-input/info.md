# AI 助手文本输入追问 - Technical Design

## Summary

在现有语音追问链路旁开一个"文本旁路"：用户在结果面板输入框打字 → 前端 invoke `send_text_question` → 后端跳过录音/ASR/TNL，直接复用 `process_followup()` → 事件推送 → 前端展示新轮次。改动极小，不触碰任何现有模块的内部逻辑。

## Architecture

```
                 现有链路（语音追问）
                 ┌─────────────────────────────────────┐
  热键 → 录音 → ASR → TNL → ┐                         │
                             ↓                         │
                    user_instruction (String)           │
                             ↓                         │
  新增链路（文本追问）        ├─→ process_followup() ──→ 推入 session → 发事件
  输入框 → Enter ──────────→ ┘                         │
                    (跳过 ASR/TNL)                      │
                 └─────────────────────────────────────┘
```

改动文件清单：

| 层 | 文件 | 改动 |
|---|---|---|
| 后端 IPC | `src-tauri/src/lib.rs` | 新增 `send_text_question` 命令（~60 行） |
| 前端 UI | `src/windows/ResultPanelWindow.tsx` | 新增 `TextInputBar` 子组件 + 处理中状态联动（~80 行） |
| 前端 UI | `src/windows/ResultPanelWindow.tsx` | `AssistantBubble` 耗时显示条件调整（~5 行） |

**不改动的文件**：`assistant_processor.rs`、`pipeline/assistant.rs`、`assistant-result.ts`、`tauri.conf.json`、事件类型定义。

## Key Components

### 1. 后端 `send_text_question` IPC 命令

```rust
#[tauri::command]
async fn send_text_question(
    text: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String>
```

逻辑步骤：
1. **前置校验**：`conversation_session` 必须存在，否则返回 Err
2. **并发保护**：`is_assistant_processing` 原子 CAS(false→true)，失败则返回 Err
3. **取上下文**：从 session 获取 `(session_id, history, prompt_mode)`；取 `assistant_processor`
4. **发 pending 事件**：`assistant_turn_pending { user_instruction: text, selected_text: None, has_selection: false }`
5. **调 LLM**：`processor.process_followup(&history, &text, None, &prompt_mode).await`
6. **成功**：构造 `ConversationTurn { asr_time_ms: 0, llm_time_ms, ... }` → 推入 session → 发 `assistant_turn_complete`
7. **失败**：发 `assistant_turn_error`
8. **清理**：`is_assistant_processing.store(false)`

需在 `tauri::Builder` 的 `.invoke_handler()` 中注册此命令。

### 2. 前端 `TextInputBar` 子组件

位置：`ResultPanelWindow.tsx` 内部子组件，与 `UserBubble`、`LoadingBubble` 等平级。

```tsx
function TextInputBar({
  isDark,
  isProcessing,   // LLM 处理中（语音或文本）
  onSend,         // (text: string) => void
}: { ... })
```

- 使用 `<textarea>` 单行高度，内容多时自动扩展（max 3 行）
- Enter 发送，Shift+Enter 换行
- Esc 不被 textarea 吞掉（stopPropagation 仅在需要时）
- 发送后清空输入框

### 3. `AssistantBubble` 耗时显示调整

```tsx
// 现有逻辑
<span>ASR {formatDuration(asrTimeMs)} · LLM {formatDuration(llmTimeMs)} · 总计 {formatDuration(totalTime)}</span>

// 调整为
{asrTimeMs > 0
  ? <span>ASR {formatDuration(asrTimeMs)} · LLM {formatDuration(llmTimeMs)} · 总计 {formatDuration(totalTime)}</span>
  : <span>LLM {formatDuration(llmTimeMs)}</span>
}
```

## Data / Control Flow

### 文本追问完整时序

```
[用户]          [ResultPanel]       [Tauri IPC]        [lib.rs]              [AssistantProcessor]
  |                  |                  |                  |                        |
  |-- 输入文本+Enter-->|                  |                  |                        |
  |                  |-- invoke -------->|                  |                        |
  |                  |  "send_text_      |-- 校验session -->|                        |
  |                  |   question"       |-- CAS 并发锁 --->|                        |
  |                  |                   |                  |                        |
  |                  |<---- emit --------|<- turn_pending --|                        |
  |                  |  (显示用户气泡     |                  |-- process_followup --->|
  |                  |   + LoadingBubble) |                  |                        |
  |                  |                   |                  |                        |
  |                  |                   |                  |<--- Ok(response) ------|
  |                  |                   |                  |                        |
  |                  |<---- emit --------|<- turn_complete -|                        |
  |                  |  (显示AI回复       |                  |                        |
  |                  |   恢复输入框)      |                  |                        |
```

### 与语音追问的并发保护

两条路径共享同一个 `is_assistant_processing: AtomicBool`：

```
语音追问正在执行 → 用户打字发送 → send_text_question CAS 失败 → 返回 Err → 前端提示
文本追问正在执行 → 用户按语音热键 → handle_assistant_mode CAS 失败 → 忽略热键
```

## Risks and Edge Cases

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 用户发文本同时按语音热键 | 同时两个请求 | `is_assistant_processing` 原子 CAS 保证互斥 |
| LLM 处理中用户按 Esc 关闭面板 | session 被清空 | 后端 LLM 回调检查 session 存在性，不存在则丢弃结果（现有逻辑 lib.rs:2529-2537） |
| 打字到一半面板被关闭 | 输入内容丢失 | 可接受，与关闭浏览器标签页行为一致 |
| 空白输入 | 无意义 LLM 调用 | 前端 trim 后校验非空 |
| textarea 内 Esc 键被吞 | 面板无法关闭 | textarea 的 keydown handler 中 Esc 不 preventDefault，让事件冒泡到 window listener |

## Testing Strategy

全部手动测试，不新增自动化测试：

| 测试项 | 验证方式 |
|--------|---------|
| 语音新对话 → 打字追问 → 正确展示 | 端到端 |
| 打字追问期间按语音热键 → 被拒 | 端到端 |
| LLM 处理中输入框禁用 → 完成后恢复 | 视觉验证 |
| Enter 发送 / Shift+Enter 换行 / 空内容不发送 | 交互验证 |
| Esc 在输入框聚焦时关闭面板 | 交互验证 |
| 文本轮次耗时只显示 "LLM x.xs" | 视觉验证 |
| LLM 失败 → 错误气泡 → 再次打字重试 | 端到端 |
| 语音追问流程无退化 | 回归验证 |

核心逻辑 `process_followup`、消息构建、滑动窗口已被上一个任务的单元测试覆盖，无需重复。

## Out of Scope

- 纯文本新对话（不通过语音，从零开始打字）
- 文本追问携带选中文本
- 输入历史记忆 / 自动补全
- 流式响应（Streaming）
