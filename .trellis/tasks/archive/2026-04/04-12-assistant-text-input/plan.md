# AI 助手文本输入追问 - Execution Plan

## Inputs

- PRD: `prd.md`
- Design: `info.md`
- Relevant specs:
  - `.trellis/spec/frontend/component-guidelines.md` — Tauri 二级窗口 Push+Poll 模式、StrictMode 防重复、IPC 命令注册
  - `.trellis/spec/frontend/quality-guidelines.md` — 前端质量规范

## File Map

- Modify: `src-tauri/src/lib.rs` — 新增 `send_text_question` IPC 命令 + 注册到 invoke_handler
- Modify: `src/types/assistant-result.ts` — 新增 `formatTimingDisplay` 纯函数
- Modify: `src/windows/ResultPanelWindow.tsx` — 新增 `TextInputBar` 子组件 + 修改 `AssistantBubble` 耗时显示
- Modify: `tests/assistantResultPanel.test.ts` — 新增 `formatTimingDisplay` 测试用例

## Execution Slices

### Slice 1: 耗时显示纯函数 + 测试

**Goal**

提取 `formatTimingDisplay` 纯函数并用测试覆盖，为 Slice 3 的 AssistantBubble 修改提供可测试基础。

**Files**

- `src/types/assistant-result.ts`
- `tests/assistantResultPanel.test.ts`

**Steps**

- [ ] 在 `tests/assistantResultPanel.test.ts` 末尾追加 `formatTimingDisplay` 测试用例

  行为描述：
  - `formatTimingDisplay(1100, 1200)` → `"ASR 1.1s · LLM 1.2s · 总计 2.3s"`（语音轮次，完整显示）
  - `formatTimingDisplay(0, 1200)` → `"LLM 1.2s"`（文本轮次，只显示 LLM）
  - `formatTimingDisplay(0, 0)` → `"LLM 0.0s"`（边界：两者都为 0）

- [ ] 运行测试确认失败（函数尚未导出）

  命令: `npm run test:ts 2>&1 | tail -20`

  预期: `formatTimingDisplay is not a function` 或类似导入错误

- [ ] 在 `src/types/assistant-result.ts` 末尾新增 `formatTimingDisplay` 函数

  ```typescript
  export function formatTimingDisplay(asrTimeMs: number, llmTimeMs: number): string {
    if (asrTimeMs > 0) {
      const totalTime = asrTimeMs + llmTimeMs;
      return `ASR ${formatDuration(asrTimeMs)} · LLM ${formatDuration(llmTimeMs)} · 总计 ${formatDuration(totalTime)}`;
    }
    return `LLM ${formatDuration(llmTimeMs)}`;
  }
  ```

- [ ] 重新运行测试确认通过

  命令: `npm run test:ts 2>&1 | tail -20`

  预期: 所有测试 PASS

---

### Slice 2: 后端 `send_text_question` IPC 命令

**Goal**

新增后端 IPC 命令，接收文本字符串直接调用 `process_followup`，跳过录音/ASR/TNL。

**Files**

- `src-tauri/src/lib.rs`

**Steps**

- [ ] 在 `lib.rs` 中 `dismiss_conversation` 函数之后、`show_notification_window` 函数之前，新增 `send_text_question` 命令

  函数签名:
  ```rust
  #[tauri::command]
  async fn send_text_question(
      text: String,
      app: AppHandle,
      state: tauri::State<'_, AppState>,
  ) -> Result<(), String>
  ```

  逻辑步骤:
  1. `text.trim()` 为空 → 返回 `Err("输入内容不能为空")`
  2. 从 `state.conversation_session` 读取 `(session_id, history, prompt_mode)`，不存在 → 返回 `Err("当前没有活跃的对话会话")`
  3. 从 `state.assistant_processor` 取 processor，不存在 → 返回 `Err("AI 助手未配置")`
  4. `is_assistant_processing` 原子 CAS(false→true)，失败 → 返回 `Err("正在处理中，请稍候")`
  5. 发 `assistant_turn_pending` 事件: `{ user_instruction: text.trim(), selected_text: None, has_selection: false }`
  6. 调用 `processor.process_followup(&history, text.trim(), None, &prompt_mode).await`
  7. 成功 → 构造 `ConversationTurn { asr_time_ms: 0, llm_time_ms, ... }` → 推入 session → 发 `assistant_turn_complete { is_followup: true }`
  8. 失败 → 发 `assistant_turn_error`
  9. `is_assistant_processing.store(false, Ordering::SeqCst)`

- [ ] 在 `tauri::generate_handler![]` 宏中注册 `send_text_question`（添加在 `dismiss_conversation` 之后）

- [ ] 编译验证

  命令: `cd "G:/RustProject/push-2-talk/src-tauri" && cargo check 2>&1 | tail -20`

  预期: 编译成功，无错误

---

### Slice 3: 前端 TextInputBar 组件 + AssistantBubble 修改

**Goal**

在 ResultPanelWindow 底部添加文本输入框，修改 AssistantBubble 使用 `formatTimingDisplay`。

**Files**

- `src/windows/ResultPanelWindow.tsx`

**Steps**

- [ ] 修改 `AssistantBubble` 组件的耗时显示部分

  将现有的硬编码格式化逻辑替换为:
  ```tsx
  import { formatTimingDisplay } from "../types/assistant-result";
  // ...
  <span>{formatTimingDisplay(asrTimeMs, llmTimeMs)}</span>
  ```

  删除 AssistantBubble 内部的 `const totalTime = asrTimeMs + llmTimeMs;` 行（不再需要）。

- [ ] 在 `ResultPanelWindow` 主组件中添加 `isProcessing` 状态

  `isProcessing` 为 `true` 的条件：`pendingTurn !== null`（已有 pending 状态说明语音或文本追问正在进行）。
  无需新增 state 变量，直接用 `!!pendingTurn` 派生。

- [ ] 在 `ResultPanelWindow.tsx` 文件底部（`ErrorBubble` 之后）新增 `TextInputBar` 子组件

  ```tsx
  function TextInputBar({
    isDark,
    isProcessing,
    onSend,
  }: {
    isDark: boolean;
    isProcessing: boolean;
    onSend: (text: string) => void;
  }) {
    // ...
  }
  ```

  实现要点：
  - 使用 `<textarea>` + `rows={1}` + CSS `resize: none` + `max-height: 72px`（约 3 行）+ `overflow-y: auto`
  - Enter 发送（trim 后非空时），Shift+Enter 换行
  - Esc 键不 preventDefault，让事件冒泡到 window listener 处理关闭
  - `isProcessing` 为 true 时 textarea 和按钮 `disabled`
  - 发送按钮使用 lucide-react 的 `SendHorizontal` 图标（或 `ArrowUp`），禁用时降低透明度
  - 发送成功后清空 textarea
  - placeholder: "输入追问..."

- [ ] 在 `ResultPanelWindow` 的 JSX 中插入 `TextInputBar`

  位置：在 "查看最新回复" 浮标之后、操作栏 `<div>` 之前。

  ```tsx
  <TextInputBar
    isDark={isDark}
    isProcessing={!!pendingTurn}
    onSend={handleTextSend}
  />
  ```

- [ ] 在 `ResultPanelWindow` 主组件中添加 `handleTextSend` 回调

  ```tsx
  const handleTextSend = useCallback(async (text: string) => {
    try {
      await invoke("send_text_question", { text });
    } catch (err) {
      console.error("[ResultPanel] 文本追问失败:", err);
    }
  }, []);
  ```

  注意：不需要手动设置 pending 状态或更新 turns — 后端会通过 `assistant_turn_pending` 和 `assistant_turn_complete` 事件推送更新，现有的 Push 监听器已经能处理。

- [ ] 在文件顶部的 lucide-react import 中添加 `SendHorizontal`（或 `ArrowUp`）

- [ ] 在文件顶部的 assistant-result import 中添加 `formatTimingDisplay`

- [ ] 编译 + 类型检查验证

  命令: `cd "G:/RustProject/push-2-talk" && npx tsc --noEmit 2>&1 | tail -20`

  预期: 无类型错误

- [ ] 全量测试验证

  命令: `npm run test:ts 2>&1 | tail -20`

  预期: 所有测试 PASS（包括 Slice 1 的新测试）

---

## Risks / Watch Items

- **textarea 内 Esc 键被吞**: textarea 的 keydown handler 中只对 Enter/Shift+Enter 做处理，Esc 不做任何拦截，自然冒泡到 window 级 listener
- **Enter 键冲突**: 当前 `result-panel-actions.ts` 的 `getKeyboardAction("Enter")` 已返回 `null`（不拦截），不冲突
- **is_assistant_processing 并发**: 文本追问和语音追问共享同一个原子标志，天然互斥

## Ready-to-Execute Summary

- First slice to start with: Slice 1（耗时显示纯函数 + 测试）
- Blocking dependencies: 无（3 个 Slice 可按顺序执行，Slice 3 依赖 Slice 1 的函数和 Slice 2 的后端命令）
