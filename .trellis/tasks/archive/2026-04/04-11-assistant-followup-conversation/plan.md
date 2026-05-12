# AI 助手多轮对话与追问功能 - Execution Plan

## Inputs

- PRD: `prd.md`
- Design: `info.md`
- Relevant specs:
  - `.trellis/spec/frontend/component-guidelines.md` — Push+Poll 双策略、透明窗口拖拽、IPC 调用防抖、操作后 state 清理
  - `.trellis/spec/backend/error-handling.md` — RAII Guard 立即释放、Pending 状态全路径补发完成事件
  - `.trellis/spec/guides/cross-layer-thinking-guide.md` — IPC 命令名匹配验证、Hidden WebView 事件丢失、capabilities 注册

## File Map

- Modify: `src-tauri/src/assistant_processor.rs` — 新增 `build_followup_messages()` + `process_followup()`
- Modify: `src-tauri/src/openai_client.rs` — 确保 `chat()` 方法为 `pub`
- Modify: `src-tauri/src/pipeline/assistant.rs` — 新增会话感知分支逻辑
- Modify: `src-tauri/src/lib.rs` — `ConversationSession` 替代 `PendingAssistantResult`、IPC 命令重写、事件重写、管道结果处理重写
- Modify: `src/types/assistant-result.ts` — 新增 `ConversationTurn`、`ConversationPayload` 等类型
- Modify: `src/windows/ResultPanelWindow.tsx` — 重构为对话流视图
- Modify: `src/windows/result-panel-actions.ts` — 扩展键盘操作（如有需要）
- Test: `src-tauri/src/assistant_processor.rs` (`#[cfg(test)] mod tests`) — 消息构建、滑动窗口截断
- Test: `tests/assistant-result.test.ts` — 对话格式化、工具函数

## Execution Slices

### Slice 1: 多轮消息构建与对话格式化（纯逻辑，全 TDD）

**Goal**

实现 `build_followup_messages()` 纯函数和 `format_conversation_for_copy()` 纯函数。这是整个功能的计算核心，不依赖 Tauri 或网络，完全可测试。

**Files**

- `src-tauri/src/assistant_processor.rs`
- `src-tauri/src/openai_client.rs` (可能需要将 `chat()` 改为 `pub`)
- `src-tauri/src/lib.rs` (数据结构定义)

#### Step 1.1: 定义数据结构

- [ ] 在 `src-tauri/src/lib.rs` 中定义 `ConversationTurn`、`ConversationSession`、`PromptMode`

  ```rust
  #[derive(Clone, serde::Serialize)]
  pub struct ConversationTurn {
      pub user_instruction: String,
      pub selected_text: Option<String>,
      pub assistant_response: String,
      pub asr_time_ms: u64,
      pub llm_time_ms: u64,
  }

  pub enum PromptMode { QA, TextProcessing }

  pub struct ConversationSession {
      pub id: String,
      pub turns: Vec<ConversationTurn>,
      pub system_prompt_mode: PromptMode,
      pub target_hwnd: Option<isize>,
      pub created_at: std::time::Instant,
  }
  ```

- [ ] 确认编译通过
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | head -20`
  - Expected: no errors (新结构暂未被引用)

#### Step 1.2: 编写消息构建的失败测试

- [ ] 在 `src-tauri/src/assistant_processor.rs` 底部新增 `#[cfg(test)] mod tests` 块，编写以下测试：

  **测试 1: `test_build_followup_messages_basic`**
  - 给定 1 轮历史（QA 模式，无选中文本），追问 1 条纯语音
  - 断言 messages 数组长度 = 5 (system + user₁ + assistant₁ + user₂)
  - 等等... 实际上是 4: [system, user₁, assistant₁, user₂]
  - 断言 messages[0].role = System
  - 断言 messages[1].role = User, content 包含历史用户指令
  - 断言 messages[2].role = Assistant, content 包含历史 AI 回复
  - 断言 messages[3].role = User, content = 新指令

  **测试 2: `test_build_followup_messages_with_selected_text`**
  - 给定 1 轮历史，追问时带有新选中文本
  - 断言最后一条 user message 包含 `"【选中的文本】"` 和 `"【用户指令】"`

  **测试 3: `test_build_followup_messages_sliding_window`**
  - 给定 25 轮历史（超过 MAX_CONVERSATION_TURNS=20）
  - 断言发送的 messages 只包含最近 20 轮 + system prompt
  - 断言 messages 总长度 = 1 (system) + 20*2 (user+assistant) + 1 (new user) = 42

  **测试 4: `test_build_followup_messages_text_processing_mode`**
  - 给定 TextProcessing 模式
  - 断言 system prompt 使用 `text_processing_system_prompt`

  **测试 5: `test_format_conversation_for_copy`**
  - 给定 2 轮对话（第 1 轮有选中文本，第 2 轮无）
  - 断言输出的 Markdown 格式正确：包含 `**问**:`、`**答**:`、`> 选中文本:`、`---` 分隔线

- [ ] 运行测试并确认全部失败（函数尚未实现）
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo test --lib assistant_processor::tests 2>&1 | tail -20`
  - Expected: 编译失败（函数不存在）

#### Step 1.3: 实现 `build_followup_messages()`

- [ ] 在 `src-tauri/src/assistant_processor.rs` 中实现：

  ```rust
  const MAX_CONVERSATION_TURNS: usize = 20;

  pub fn build_followup_messages(
      system_prompt: &str,
      history: &[ConversationTurn],
      new_instruction: &str,
      new_selected_text: Option<&str>,
  ) -> Vec<Message> { ... }
  ```

  逻辑：
  - 第一条固定为 `Message::system(system_prompt)`
  - 遍历 history（如果 > MAX_CONVERSATION_TURNS，取最后 20 个），每轮生成：
    - `Message::user(format_user_content(&turn.user_instruction, turn.selected_text.as_deref()))`
    - `Message::assistant(&turn.assistant_response)`
  - 最后追加新的 `Message::user(format_user_content(new_instruction, new_selected_text))`

- [ ] 实现辅助函数 `format_user_content(instruction, selected_text)`:
  - 有选中文本: `"【选中的文本】\n{selected}\n\n【用户指令】\n{instruction}"`
  - 无选中文本: 直接返回 instruction

- [ ] 确保 `openai_client.rs` 中 `Message` 结构和 `chat()` 方法对 `assistant_processor` 可见
  - 如果 `chat()` 是 `pub(crate)` 或 private，改为 `pub`
  - 如果 `Message::assistant()` 构造函数不存在，添加一个

#### Step 1.4: 实现 `format_conversation_for_copy()`

- [ ] 在 `src-tauri/src/lib.rs` 中（或 `assistant_processor.rs` 中）实现：

  ```rust
  pub fn format_conversation_for_copy(turns: &[ConversationTurn]) -> String { ... }
  ```

  格式：
  ```
  **问**: {instruction}
  > 选中文本: {selected_text}  // 仅当存在时

  **答**: {response}

  ---

  **问**: {instruction}

  **答**: {response}
  ```

#### Step 1.5: 运行全部测试确认通过

- [ ] 运行测试
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo test --lib assistant_processor::tests 2>&1`
  - Expected: 5 tests passed

- [ ] 运行整体编译检查
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -10`
  - Expected: no errors

---

### Slice 2: 后端集成（Session 管理 + Pipeline 分支 + IPC 命令）

**Goal**

将 `PendingAssistantResult` 替换为 `ConversationSession`，重写 pipeline 分支逻辑和所有 IPC 命令，使后端完整支持多轮对话。

**Files**

- `src-tauri/src/lib.rs` (主要改动)
- `src-tauri/src/pipeline/assistant.rs`
- `src-tauri/src/assistant_processor.rs` (新增 `process_followup()`)

#### Step 2.1: 编写 `process_followup()` 方法

- [ ] 在 `assistant_processor.rs` 中新增：

  ```rust
  pub async fn process_followup(
      &self,
      history: &[ConversationTurn],
      new_instruction: &str,
      new_selected_text: Option<&str>,
      prompt_mode: &PromptMode,
  ) -> Result<String>
  ```

  实现：
  - 根据 `prompt_mode` 选择 `qa_system_prompt` 或 `text_processing_system_prompt`
  - 调用 `build_followup_messages()` 构建消息数组
  - 调用 `self.client.chat(&messages, ChatOptions::for_smart_command()).await`
  - 包装超时逻辑（复用 `ASSISTANT_TIMEOUT_SECS = 300`）

- [ ] 编译检查
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -10`
  - Expected: no errors

#### Step 2.2: 替换 AppState 中的 Pending → Session

- [ ] 在 `lib.rs` 中：
  - 移除 `PendingAssistantResult` 结构
  - 移除 `AssistantResultPayload` 结构
  - 将 AppState 中 `pending_assistant_result: Arc<Mutex<Option<PendingAssistantResult>>>` 替换为 `conversation_session: Arc<Mutex<Option<ConversationSession>>>`
  - 新增 `is_assistant_processing: Arc<AtomicBool>` 字段（阻止追问期间重复触发）

- [ ] 新增前端 payload 类型：

  ```rust
  #[derive(Clone, serde::Serialize)]
  struct ConversationTurnPayload {
      user_instruction: String,
      selected_text: Option<String>,
      has_selection: bool,
      assistant_response: String,
      asr_time_ms: u64,
      llm_time_ms: u64,
  }

  #[derive(Clone, serde::Serialize)]
  struct ConversationStatePayload {
      session_id: String,
      turns: Vec<ConversationTurnPayload>,
  }

  #[derive(Clone, serde::Serialize)]
  struct TurnPendingPayload {
      user_instruction: String,
      selected_text: Option<String>,
      has_selection: bool,
  }

  #[derive(Clone, serde::Serialize)]
  struct TurnCompletePayload {
      session_id: String,
      turn: ConversationTurnPayload,
      is_followup: bool,
  }

  #[derive(Clone, serde::Serialize)]
  struct TurnErrorPayload {
      session_id: String,
      error_message: String,
  }
  ```

#### Step 2.3: 重写 IPC 命令

- [ ] **`get_conversation_state`** (替代 `get_pending_assistant_result`):
  ```rust
  #[tauri::command]
  async fn get_conversation_state(
      state: tauri::State<'_, AppState>,
  ) -> Result<Option<ConversationStatePayload>, String>
  ```
  从 `conversation_session` 读取，返回完整会话状态。不 take()，只 clone/map。

- [ ] **`dismiss_conversation`** (替代 `dismiss_assistant_result`):
  ```rust
  #[tauri::command]
  async fn dismiss_conversation(
      app: AppHandle,
      state: tauri::State<'_, AppState>,
  ) -> Result<(), String>
  ```
  - `session = conversation_session.lock().take()`
  - 将所有 turns 格式化后发 `transcription_complete` 事件（用于 History 记录）
  - `hide_result_panel_window()`

- [ ] **`copy_latest_reply`** (替代 `copy_assistant_result`):
  ```rust
  #[tauri::command]
  async fn copy_latest_reply(
      state: tauri::State<'_, AppState>,
  ) -> Result<(), String>
  ```
  - 读取 session 最后一轮的 `assistant_response`，复制到剪贴板

- [ ] **`copy_full_conversation`** (新增):
  ```rust
  #[tauri::command]
  async fn copy_full_conversation(
      state: tauri::State<'_, AppState>,
  ) -> Result<(), String>
  ```
  - 调用 `format_conversation_for_copy(&session.turns)` → 复制到剪贴板

- [ ] **`paste_latest_reply`** (替代 `paste_assistant_result`):
  ```rust
  #[tauri::command]
  async fn paste_latest_reply(
      app: AppHandle,
      state: tauri::State<'_, AppState>,
  ) -> Result<String, String>
  ```
  - 读取最后一轮 response
  - **take() session**（粘贴 = 会话结束）
  - 发 `transcription_complete`
  - 隐藏面板 → 恢复焦点 → 粘贴
  - 触发学习观察（如启用）

- [ ] 更新 `invoke_handler()` 注册，移除旧命令名，注册新命令名

- [ ] 更新 `stop_app()` 清理逻辑：
  - `state.conversation_session.lock().unwrap().take()`
  - `state.is_assistant_processing.store(false, Ordering::SeqCst)`

#### Step 2.4: 重写管道结果处理（lib.rs 中 assistant pipeline 完成后的逻辑）

- [ ] **新对话路径** (session = None):
  - 创建 `ConversationSession { id: uuid, turns: vec![], system_prompt_mode, target_hwnd, created_at }`
  - 调用现有 `process()` / `process_with_context()`
  - 构建 `ConversationTurn`，push 进 `session.turns`
  - 存入 AppState: `*lock = Some(session)`
  - `show_result_panel_window()` (居中定位)
  - 100ms 等待
  - `emit("assistant_turn_complete", TurnCompletePayload { is_followup: false, ... })`

- [ ] **追问路径** (session = Some):
  - 设置 `is_assistant_processing = true`
  - `emit("assistant_turn_pending", TurnPendingPayload { ... })`
  - 从 session 中 clone turns + system_prompt_mode
  - 调用 `process_followup()`
  - 成功: 构建 turn → push 进 session.turns → `emit("assistant_turn_complete", { is_followup: true })`
  - 失败: `emit("assistant_turn_error", { error_message })`（不 push turn）
  - 设置 `is_assistant_processing = false`

- [ ] **热键入口处增加 guard**:
  - 检查 `is_assistant_processing`，如果为 true 则直接 return（不录音）

- [ ] **不再调用 `show_result_panel_window()`**（追问路径），面板已打开

#### Step 2.5: 编译与基础验证

- [ ] 编译检查
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo check 2>&1 | tail -20`
  - Expected: no errors

- [ ] 运行已有 Rust 测试
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo test 2>&1 | tail -20`
  - Expected: all tests pass (包括 Slice 1 的测试)

---

### Slice 3: 前端对话流视图

**Goal**

将 `ResultPanelWindow.tsx` 从单结果卡片重构为对话流视图，支持多轮渲染、Push+Poll 双策略、智能滚动。

**Files**

- `src/types/assistant-result.ts`
- `src/windows/ResultPanelWindow.tsx`
- `src/windows/result-panel-actions.ts`
- `tests/assistant-result.test.ts` (新增/修改)

#### Step 3.1: 编写前端类型和工具函数的失败测试

- [ ] 在 `tests/assistant-result.test.ts` 中新增测试：

  **测试 1: `formatConversationForCopyTs_basic`**
  - 给定 2 轮 `ConversationTurn`（第 1 轮有 selected_text，第 2 轮无）
  - 断言输出包含 `**问**:` 和 `**答**:` 和 `---`
  - 断言第 1 轮包含 `> 选中文本:`
  - 断言第 2 轮不包含 `> 选中文本:`

  **测试 2: `formatConversationForCopyTs_single_turn`**
  - 给定 1 轮，断言输出不包含 `---` 分隔线

- [ ] 运行测试确认失败
  - Command: `cd /g/RustProject/push-2-talk && npm run test:ts 2>&1 | tail -20`
  - Expected: 新测试失败（函数不存在）

#### Step 3.2: 更新前端类型定义

- [ ] 修改 `src/types/assistant-result.ts`:

  ```typescript
  // 保留原有 AssistantResultPayload（可能其他地方引用）或标记 deprecated

  export interface ConversationTurn {
    user_instruction: string;
    selected_text?: string;
    has_selection: boolean;
    assistant_response: string;
    asr_time_ms: number;
    llm_time_ms: number;
  }

  export interface ConversationStatePayload {
    session_id: string;
    turns: ConversationTurn[];
  }

  export interface TurnPendingPayload {
    user_instruction: string;
    selected_text?: string;
    has_selection: boolean;
  }

  export interface TurnCompletePayload {
    session_id: string;
    turn: ConversationTurn;
    is_followup: boolean;
  }

  export interface TurnErrorPayload {
    session_id: string;
    error_message: string;
  }

  // 前端格式化函数（用于"复制全部"的前端预览，实际复制由后端执行）
  export function formatConversationForCopy(turns: ConversationTurn[]): string { ... }
  ```

- [ ] 实现 `formatConversationForCopy()`:
  - 遍历 turns
  - 每轮输出 `**问**: {instruction}\n`，如有 selected_text 追加 `> 选中文本: {text}\n`
  - 追加 `\n**答**: {response}\n`
  - 轮次间用 `\n---\n\n` 分隔

#### Step 3.3: 运行前端测试确认通过

- [ ] 运行测试
  - Command: `cd /g/RustProject/push-2-talk && npm run test:ts 2>&1 | tail -20`
  - Expected: 新增的 2 个测试通过

#### Step 3.4: 重写 ResultPanelWindow.tsx 为对话流视图

- [ ] **State 改造**:
  ```typescript
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [turns, setTurns] = useState<ConversationTurn[]>([]);
  const [pendingTurn, setPendingTurn] = useState<TurnPendingPayload | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [theme, setTheme] = useState("light");
  const [copyFeedback, setCopyFeedback] = useState<"latest" | "all" | false>(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);
  ```

- [ ] **Push 模式 — 监听 3 个新事件**:
  - `assistant_turn_complete`:
    - `is_followup = false` → 重置 turns 为 `[turn]`，设置 sessionId
    - `is_followup = true` → append turn 到 turns，清除 pendingTurn 和 errorMessage
  - `assistant_turn_pending`: 设置 pendingTurn（显示用户消息 + loading）
  - `assistant_turn_error`: 设置 errorMessage，清除 pendingTurn

- [ ] **Poll 模式 — 使用 `get_conversation_state` 替代**:
  - 无 turns 且无 sessionId 时，每 300ms 轮询 `get_conversation_state`
  - 返回数据后设置 sessionId + turns

- [ ] **布局重构**:
  ```
  <TitleBar>  🤖 AI 助手  ✕
  <ConversationArea ref={containerRef} onScroll={handleScroll}>
    {turns.map((turn, i) => (
      <>
        {i > 0 && <Divider />}
        <UserBubble instruction={turn.user_instruction} selectedText={turn.selected_text} />
        <AssistantBubble response={turn.assistant_response} timing={turn.asr_time_ms + turn.llm_time_ms} />
      </>
    ))}
    {pendingTurn && (
      <>
        <Divider />
        <UserBubble instruction={pendingTurn.user_instruction} selectedText={pendingTurn.selected_text} />
        <LoadingBubble />
      </>
    )}
    {errorMessage && <ErrorBubble message={errorMessage} />}
  </ConversationArea>
  {!isAtBottom && <ScrollToBottomFab onClick={scrollToBottom} />}
  <ActionBar>
    <CopyLatestButton />  {/* invoke("copy_latest_reply") */}
    <CopyAllButton />     {/* invoke("copy_full_conversation") */}
  </ActionBar>
  ```

- [ ] **Dismiss 操作改为 `dismiss_conversation`**:
  ```typescript
  const handleDismiss = async () => {
    await invoke("dismiss_conversation");
    setTurns([]);
    setSessionId(null);
    setPendingTurn(null);
    setErrorMessage(null);
  };
  ```

- [ ] **滚动逻辑实现**:
  ```typescript
  const handleScroll = () => {
    const el = containerRef.current;
    if (!el) return;
    setIsAtBottom(el.scrollHeight - el.scrollTop - el.clientHeight < 30);
  };

  const scrollToBottom = () => {
    containerRef.current?.scrollTo({
      top: containerRef.current.scrollHeight,
      behavior: "smooth",
    });
  };

  // 新 turn 到达时自动滚动
  useEffect(() => {
    if (isAtBottom) scrollToBottom();
  }, [turns.length, pendingTurn]);
  ```

- [ ] **Paste 操作** (如仍保留):
  ```typescript
  const handlePaste = async () => {
    if (isPasting) return;
    setIsPasting(true);
    try {
      await invoke("paste_latest_reply");
      // paste = 会话结束，清空 state
      setTurns([]);
      setSessionId(null);
    } finally {
      setIsPasting(false);
    }
  };
  ```

- [ ] **视觉样式要点**:
  - UserBubble: 左侧 🎤 图标 + 指令文本；如有 selected_text 显示 📎 + 截断预览
  - AssistantBubble: 复用 `<MarkdownRenderer>` 渲染回复，右下角小字显示耗时
  - LoadingBubble: 脉冲动画 + "AI 思考中..."
  - ErrorBubble: ⚠️ 图标 + 错误信息 + 红色边框
  - Divider: 细线 + "追问" 文字居中
  - ScrollToBottomFab: 固定在滚动区域右下角，半透明圆形按钮 + ↓ 箭头

#### Step 3.5: 前端编译与视觉验证

- [ ] TypeScript 编译检查
  - Command: `cd /g/RustProject/push-2-talk && npx tsc --noEmit 2>&1 | tail -20`
  - Expected: no type errors

- [ ] 运行全部前端测试
  - Command: `cd /g/RustProject/push-2-talk && npm run test:ts 2>&1 | tail -20`
  - Expected: all tests pass

---

### Slice 4: 集成验证与边界处理

**Goal**

全链路编译通过、手动测试核心流程、处理边界情况。

**Files**

- `src-tauri/src/lib.rs` (微调)
- `src/windows/ResultPanelWindow.tsx` (微调)

#### Step 4.1: 全项目构建验证

- [ ] Rust 后端编译
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo build 2>&1 | tail -20`
  - Expected: build succeeded

- [ ] 前端 + Tauri 全量构建
  - Command: `cd /g/RustProject/push-2-talk && npm run tauri build 2>&1 | tail -30`
  - Expected: build succeeded

#### Step 4.2: 验证 IPC 命令注册完整性

- [ ] 确认所有新命令在 `invoke_handler()` 中注册:
  - `get_conversation_state`
  - `dismiss_conversation`
  - `copy_latest_reply`
  - `copy_full_conversation`
  - `paste_latest_reply`

- [ ] 确认所有旧命令已移除:
  - `get_pending_assistant_result`
  - `dismiss_assistant_result`
  - `copy_assistant_result`
  - `paste_assistant_result`

- [ ] 确认前端 `invoke()` 调用的命令名与后端 `#[tauri::command]` 一一对应
  - 搜索前端所有 invoke 调用: 确保无旧命令名残留

#### Step 4.3: 验证 Pending State 全路径补发

根据 spec `error-handling.md` 要求，确认 ConversationSession 的 5 条清理路径：

| 路径 | 触发 | 是否发 transcription_complete |
|------|------|------|
| Dismiss (用户关闭面板) | `dismiss_conversation` | ✅ inserted=false |
| Paste (用户粘贴) | `paste_latest_reply` | ✅ inserted=true |
| Overwrite (新对话覆盖旧会话) | 新 pipeline 结果到达时 session!=None 且面板关闭 | ✅ 为旧会话发 inserted=false |
| App Stop | `stop_app()` | ❌ 不需要（中断的会话不记录历史） |
| Error | LLM 失败 | ❌ 不发（会话仍活跃，用户可重试） |

- [ ] 在代码中逐一确认上述路径的实现

#### Step 4.4: 运行全部自动化测试

- [ ] Rust 测试
  - Command: `cd /g/RustProject/push-2-talk/src-tauri && cargo test 2>&1 | tail -20`
  - Expected: all tests pass

- [ ] 前端测试
  - Command: `cd /g/RustProject/push-2-talk && npm run test:ts 2>&1 | tail -20`
  - Expected: all tests pass

#### Step 4.5: 手动测试清单

以下测试需要以管理员权限运行 `npm run tauri dev`：

- [ ] **单轮兼容 (R10)**: 按助手热键 → 录音 → 面板弹出结果 → 复制 → 关闭 → 体验与改动前一致
- [ ] **新对话→追问→关闭 (R1+R3)**: 按热键 → 首轮结果 → 再按热键录音 → 追问结果追加 → 关闭面板
- [ ] **纯语音追问 (R2)**: 面板打开 → 不选任何文本 → 按热键 → 追问成功
- [ ] **带选中文本追问 (R2)**: 面板打开 → 在其他窗口选文本 → 按热键 → 追问携带新上下文
- [ ] **滚动行为 (R4)**: 多轮追问 → 内容超出面板高度 → 自动滚到底 → 手动上滚 → 再追问 → 浮标出现 → 点击浮标回到底部
- [ ] **面板保持可见 (R5)**: 追问录音期间面板不隐藏、不移动
- [ ] **处理中阻止重复 (R8)**: AI 思考中快速再按热键 → 无响应
- [ ] **LLM 失败 (R9)**: 断网后追问 → 错误气泡显示 → 恢复网络再追问 → 成功
- [ ] **关闭写入历史 (R1)**: 多轮对话后关闭 → 主窗口 History 页面有记录

## Risks / Watch Items

- **`openai_client.rs` 中 `chat()` 方法可见性**: 如果是 private，Slice 1 Step 1.3 需要先改为 `pub`
- **`Message::assistant()` 构造函数**: 当前可能只有 `Message::system()` 和 `Message::user()`，需要确认是否存在 `Message::assistant()` 并在缺失时添加
- **Hidden WebView 首轮事件丢失**: 必须保留 Poll 模式作为 fallback（spec 明确要求）
- **大量旧命令名的前端引用**: 替换时需搜索全项目确保无遗漏，包括测试文件
- **History 记录格式变更**: 多轮对话的 `transcription_complete` payload 格式与单轮不同，需确认 History 页面兼容

## Ready-to-Execute Summary

- First slice to start with: **Slice 1**（纯逻辑 TDD，零风险，建立信心）
- Blocking dependencies: 无。Slice 1 → Slice 2 → Slice 3 → Slice 4 为顺序依赖
- 预估工作量: Slice 1 (小) → Slice 2 (大) → Slice 3 (大) → Slice 4 (中)
