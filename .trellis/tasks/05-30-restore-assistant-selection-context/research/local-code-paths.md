# 本地代码调研：AI 助手选区上下文

## 调研范围

- 后端热键停止后的 AI 助手路径：`src-tauri/src/lib.rs`
- LLM 消息构造：`src-tauri/src/assistant_processor.rs`
- 默认提示词与配置迁移：`src-tauri/src/config.rs`、`src/constants/index.ts`
- 结果面板和文本追问：`src/windows/ResultPanelWindow.tsx`
- 历史设计记录：`docs/architecture/flows/assistant-and-learning.md`、`.trellis/tasks/archive/2026-04/assistant-async-result-panel/info.md`、`.trellis/workspace/yyyzl/journal-1.md`

## 当前执行链路

1. 用户按 AI 助手热键后，后端在停止录音阶段等待 100ms，再调用 `clipboard_manager::get_selected_text()` 捕获选区。
2. 捕获成功后，当前实现立即 `drop(guard)`，恢复用户剪贴板。这符合 `.trellis/spec/backend/error-handling.md` 的约束。
3. `handle_assistant_mode()` 先做 ASR/TNL/个性化，再判断是否已有 `conversation_session`。
4. 没有会话时，根据 `selected_text.is_some()` 选择 `PromptMode::TextProcessing` 或 `PromptMode::QA`，并创建新会话。
5. 有会话时，读取会话的 `system_prompt_mode`，即首轮锁定的 PromptMode；本轮新选区虽然传入 `process_turn`，但系统提示词仍沿用首轮模式。
6. `AssistantProcessor::process_turn()` 调 `build_turn_messages()`，后者会把历史轮次和本轮指令都转成 OpenAI messages。
7. `format_user_content()` 当前只做：
   - 有选区：`【选中的文本】...【用户指令】...`
   - 无选区：直接使用 instruction
8. 文本追问 `send_text_question()` / `run_text_question_task()` 永远传 `selected_text: None`，只能依赖历史选区。

## 关键发现

### 1. 选区没有完全丢失，但可能被“旧会话模式”弱化

代码已经把 `selected_text` 存入 `TurnPendingPayload`、`ConversationTurn`，并在 `build_turn_messages()` 中发给 LLM。因此不是底层捕获彻底失效。

风险在于：会话打开后再次选择新文本并按热键，会进入追问路径，系统提示词仍使用首轮 `PromptMode`。如果首轮是 QA，本轮即使带选区，也仍是 QA 系统提示词；如果首轮是 TextProcessing，后续无选区的普通追问也仍是文本处理提示词。

### 2. 默认文本处理提示词太窄

后端 `DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT` 强调“直接输出处理后的结果，不要添加多余解释”。这适合润色/翻译，但不适合用户选中一段代码/文字后问“这里有什么问题”“帮我分析一下”“这段是什么意思”。

前端 `DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt` 和后端默认常量不完全一致，也需要同步。

### 3. 结果面板改造改变了交互语义

历史设计从“单轮处理后自动插入/替换”变成“弹出结果面板，支持多轮追问”。多轮能力本身没有错，但它让“再次按热键”从旧体验的“新的选区任务”变成“当前会话追问”。如果用户心智仍是旧的单轮选区助手，就会感觉能力丢了。

### 4. 不能通过恢复旧 ClipboardGuard 生命周期来修

异步结果面板设计明确要求捕获后立即释放剪贴板。恢复旧的 guard 持有会导致慢模型期间剪贴板被占用，是明确禁止的退化。

## GitNexus 影响分析

- `handle_assistant_mode`：LOW。直接上游为 `start_app`，间接涉及服务重启和托盘 ASR 切换流程。
- `send_text_question`：LOW。图谱未发现 Rust 上游，实际由 Tauri IPC 前端调用。
- `build_turn_messages`：LOW。直接影响 `build_followup_messages` 和相关单元测试。
- `format_user_content`：LOW。直接影响 `build_turn_messages`，适合作为小范围修复点。
- `process_turn`：HIGH。直接影响 `handle_assistant_mode` 和 `run_text_question_task`，还关联联网搜索、流式输出、取消与工具上下文。应尽量避免改签名或大幅改控制流。
- `DEFAULT_ASSISTANT_TEXT_PROCESSING_PROMPT`：LOW。图谱未发现上游，但实际由 `AssistantConfig::default()` 与前端默认配置使用。

## 修复建议

推荐先走最小高收益修复：

1. 把 `format_user_content()` 的选区消息改成更强语义，例如：

   ```text
   【本轮选中文本（主要上下文）】
   ...

   【用户问题或指令】
   ...
   ```

2. 更新默认文本处理提示词为“选区上下文助手”：
   - 有选区时，始终先读选区；
   - 判断用户意图是编辑类还是问答/解释/分析类；
   - 编辑类直接输出处理结果；
   - 问答/解释/分析类围绕选区回答；
   - 指令不明确时，基于选区给出最可能有用的结果，必要时提出一个简短澄清问题。

3. 添加 Rust 单元测试：
   - `format_user_content` 应标记本轮选区是主要上下文；
   - `build_followup_messages` 在历史有旧选区、本轮有新选区时，本轮消息必须包含“本轮选中文本/主要上下文”；
   - 默认文本处理 prompt 包含“回答问题/解释/分析”和“编辑类直接输出结果”等约束。

4. 同步前端默认配置，避免新用户和配置修复路径拿到旧 prompt。

## 后续可选增强

- 增加“新选区时自动新会话”偏好设置。
- 让 `ConversationTurn` 记录每轮 prompt mode，彻底解除“首轮锁定”限制。
- 为文本输入追问增加“引用最近选区/首轮选区”的显式按钮，但这涉及 UI，不建议放进第一刀。
