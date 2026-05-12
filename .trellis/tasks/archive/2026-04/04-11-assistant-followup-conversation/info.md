# AI 助手多轮对话与追问功能 - Technical Design

## Summary

将 AI 助手模式从单轮无状态交互升级为多轮对话。后端引入 `ConversationSession` 管理对话历史，前端将结果面板从单结果卡片重构为对话流视图。核心交互规则：面板开=追问，面板关=新对话。

## Architecture

### 后端变更范围

```
src-tauri/src/
├── assistant_processor.rs  ← 新增 process_followup() 方法
├── pipeline/assistant.rs   ← 分支逻辑：新会话 vs 追问
├── lib.rs                  ← ConversationSession 替代 PendingAssistantResult
│                              新增/修改 IPC commands
└── (openai_client.rs)      ← 无需修改，已支持 Vec<ChatMessage>
```

### 前端变更范围

```
src/
├── windows/
│   └── ResultPanelWindow.tsx  ← 重构为对话流视图
├── types/
│   └── assistant-result.ts    ← 新增 ConversationTurn 等类型
└── components/
    └── MarkdownRenderer.tsx   ← 无需修改，复用
```

### 不变的部分

- hotkey_service.rs — 热键检测逻辑不变
- clipboard_manager.rs — 选中文本捕获逻辑不变
- streaming_recorder.rs / audio_recorder.rs — 录音逻辑不变
- asr/ — ASR 转写逻辑不变
- tnl/ — 文本正规化逻辑不变
- overlay 窗口 — 录音状态显示不变

## Key Components

### 1. ConversationSession（后端新增）

```rust
struct ConversationTurn {
    user_instruction: String,
    selected_text: Option<String>,
    assistant_response: String,
    asr_time_ms: u64,
    llm_time_ms: u64,
}

struct ConversationSession {
    id: String,                         // UUID
    turns: Vec<ConversationTurn>,
    system_prompt_mode: PromptMode,     // QA | TextProcessing, 首轮锁定
    target_hwnd: Option<isize>,         // 首轮触发时的目标窗口
    created_at: Instant,
}

enum PromptMode { QA, TextProcessing }
```

替换 AppState 中的：
```rust
- pending_assistant_result: Arc<Mutex<Option<PendingAssistantResult>>>
+ conversation_session: Arc<Mutex<Option<ConversationSession>>>
```

### 2. AssistantProcessor 扩展（后端修改）

保留现有方法（首轮调用），新增：
```rust
pub async fn process_followup(
    &self,
    history: &[ConversationTurn],
    new_instruction: &str,
    new_selected_text: Option<&str>,
    prompt_mode: PromptMode,
) -> Result<String>
```

构建的 messages 数组：
```
[system_prompt]                       ← prompt_mode 决定
[user₁ (+ selected_text₁ if any)]    ← 历史轮次
[assistant₁]
[user₂ (+ selected_text₂ if any)]
[assistant₂]
...
[userₙ (+ new_selected_text if any)] ← 本次追问
```

滑动窗口：当 turns.len() > MAX_CONVERSATION_TURNS (20) 时，只发送最近 20 轮。

### 3. Pipeline 分支逻辑（后端修改）

```
pipeline/assistant.rs 入口：
  conversation_session.lock()
    ├─ None → 新会话路径
    │   创建 session → process() / process_with_context()
    │   → push turn → show panel → emit turn_complete
    │
    └─ Some(session) → 追问路径
        emit turn_pending → process_followup()
        → push turn → emit turn_complete (不 show panel)
```

### 4. IPC Commands 变更（后端修改）

| Command | 变更 | 说明 |
|---------|------|------|
| `get_pending_assistant_result` | 重命名/替换为 `get_conversation_state` | 返回完整会话（所有轮次） |
| `dismiss_assistant_result` | 重命名/替换为 `dismiss_conversation` | 清除 session + 写入历史 + 隐藏面板 |
| `copy_assistant_result` | 修改为 `copy_latest_reply` | 复制最后一轮 assistant_response |
| 新增 | `copy_full_conversation` | 格式化复制整个对话 |
| `paste_assistant_result` | 修改为 `paste_latest_reply` | 粘贴最后一轮回复到目标窗口 |

### 5. 事件变更

| 事件 | 类型 | 说明 |
|------|------|------|
| `assistant_turn_pending` | 新增 | 追问录音完成后立即发出，前端显示用户消息+loading |
| `assistant_turn_complete` | 替代 `assistant_result_ready` | 一轮完成，携带 turn 数据 + is_followup 标记 |
| `assistant_turn_error` | 新增 | LLM 调用失败，前端显示错误气泡 |

### 6. ResultPanelWindow 重构（前端）

**State 设计：**
```typescript
interface ConversationTurn {
  user_instruction: string;
  selected_text?: string;
  assistant_response: string;
  asr_time_ms: number;
  llm_time_ms: number;
}

// Component state
const [turns, setTurns] = useState<ConversationTurn[]>([]);
const [pendingTurn, setPendingTurn] = useState<PendingTurnInfo | null>(null);
const [isAtBottom, setIsAtBottom] = useState(true);
```

**布局结构：**
```
TitleBar          — 🤖 AI 助手 + ✕ 关闭
ConversationView  — 滚动区域，渲染 TurnBubble × N + PendingBubble
ScrollToBottomFab — 浮标按钮（仅 !isAtBottom 时显示）
ActionBar         — 复制最新回复 / 复制全部
```

**滚动逻辑：**
```typescript
const containerRef = useRef<HTMLDivElement>(null);

const onScroll = () => {
  const el = containerRef.current;
  if (!el) return;
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 30;
  setIsAtBottom(atBottom);
};

// 新消息到达时
useEffect(() => {
  if (isAtBottom && containerRef.current) {
    containerRef.current.scrollTo({ top: containerRef.current.scrollHeight, behavior: 'smooth' });
  }
}, [turns.length, pendingTurn]);
```

## Data / Control Flow

### 流程 A：新对话（首轮）

```
User presses Alt+Space (面板关闭, session = None)
  → hotkey_service triggers assistant on_start
  → 100ms wait → clipboard capture (selected text if any)
  → recording starts, overlay shows
  → user releases → recording stops → ASR
  → pipeline checks session = None → 新会话路径
  → create ConversationSession
  → call process() / process_with_context()
  → build turn, push to session.turns
  → show_result_panel_window() (居中定位)
  → emit("assistant_turn_complete", { turn, is_followup: false })
  → frontend renders first turn
```

### 流程 B：追问（后续轮次）

```
User presses Alt+Space (面板打开, session = Some)
  → hotkey_service triggers assistant on_start
  → 100ms wait → clipboard capture (new selected text, if any)
  → recording starts, overlay shows (面板保持可见不动)
  → user releases → recording stops → ASR
  → pipeline checks session = Some → 追问路径
  → emit("assistant_turn_pending", { user_instruction, selected_text })
    → frontend immediately shows user bubble + "AI 思考中..." placeholder
    → auto-scroll to bottom if isAtBottom
  → call process_followup(history, instruction, selected_text, mode)
  → build turn, push to session.turns
  → emit("assistant_turn_complete", { turn, is_followup: true })
    → frontend replaces placeholder with AI response
    → auto-scroll if isAtBottom, otherwise show fab
```

### 流程 C：关闭面板

```
User presses Esc / clicks ✕
  → frontend calls dismiss_conversation()
  → backend: session = conversation_session.lock().take()
  → format all turns → emit("transcription_complete") for history
  → hide_result_panel_window()
```

### 流程 D：复制操作

| 操作 | 行为 |
|------|------|
| 复制最新回复 | 只复制最后一轮 assistant_response |
| 复制全部 | 格式化输出全部对话（Markdown 格式） |

复制全部格式：
```markdown
**问**: 用户指令
> 选中文本: ...（如有）

**答**: AI 回复

---

**问**: 追问指令

**答**: AI 回复
```

## Risks and Edge Cases

| 风险 | 概率 | 影响 | 缓解方案 |
|------|------|------|---------|
| LLM 调用期间用户再按热键 | 高 | 重复请求 | `is_assistant_processing: AtomicBool` 阻止重复触发 |
| 追问时 LLM 失败 | 中 | 对话中断 | 错误气泡显示在对话流中，不写入 turns，可重试 |
| 对话超出 token 限制 | 低 | LLM 报错 | 滑动窗口截断，最多发送 20 轮 |
| 面板录音期间被意外关闭 | 低 | 上下文丢失 | session 被 take()，pipeline 降级为新对话 |
| 首轮 target_hwnd 失效 | 低 | 粘贴失败 | 现有 fallback：降级为复制到剪贴板 |
| 面板内选中文本后按热键 | 中 | 需正确捕获 | 面板 focusable=true，Ctrl+C 自然捕获面板内选中文本 |
| pull 模式兼容 | 中 | 首轮可能丢事件 | `get_conversation_state` 替代原 polling，返回完整会话 |

## Testing Strategy

### 单元测试（自动化）

| 测试项 | 模块 | 方式 |
|--------|------|------|
| `process_followup()` 正确构建多轮 messages | assistant_processor.rs | Rust unit test |
| 滑动窗口截断（>20 轮保留最近 20） | assistant_processor.rs | Rust unit test |
| 首轮 PromptMode 锁定 | pipeline/assistant.rs | Rust unit test |
| user message 格式（有/无选中文本） | assistant_processor.rs | Rust unit test |
| 对话历史格式化（复制全部） | lib.rs | Rust unit test |
| isAtBottom 滚动判断 | ResultPanelWindow.tsx | TS unit test |

### 集成/手动测试

| 测试项 | 覆盖 |
|--------|------|
| 新对话 → 追问 → 追问 → 关闭 | 全流程 |
| 纯语音追问 vs 带选中文本追问 | R2 |
| 面板内选中 AI 文本后追问 | R2 边界 |
| 多轮后滚动浮标行为 | R4 |
| LLM 失败 → 错误气泡 → 重试 | R9 |
| 快速连按热键 | R8 |
| 20+ 轮对话 | R7 |
| 单轮使用不追问直接关闭 | R10 回归 |

## Out of Scope

- 流式响应（Streaming）
- 对话持久化 / 跨会话恢复
- 对话分支 / 编辑历史消息
- 键盘文字输入追问
