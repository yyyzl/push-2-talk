# 接入助手语音中置信个性化仲裁

## Goal

让 AI 助手语音指令路径和普通听写路径一样，能把本地个性化二次解码产生的中置信候选交给 LLM 候选仲裁，而不是只应用高置信本地替换、丢弃中置信候选。

## Background

当前普通听写路径已经具备完整链路：

1. ASR 文本进入 TNL。
2. 本地个性化二次解码生成高置信替换或中置信候选。
3. 中置信候选合并进 `TnlDiagnostics`。
4. `LlmPostProcessor::arbitrate_tnl_candidates` 进行 bounded LLM 仲裁。
5. 真实 `AppliedLlm` / `RejectedLlm` 会作为弱反馈写回 correction pair。

AI 助手生产路径 `handle_assistant_mode` 当前只运行本地高置信替换，没有把中置信候选交给 LLM 仲裁；这会导致同一条 borderline correction pair 在听写里可以被修正，在助手语音指令里仍可能漏修。

## Requirements

- AI 助手语音指令路径必须在 `assistant_turn_pending`、usage stats、conversation history 和 `AssistantProcessor::process_turn` 之前完成候选仲裁。
- 仲裁必须复用既有 bounded candidate prompt、候选数量上限、重叠 span 保护和 JSON 决策解析逻辑，不复制一套不一致的 prompt/替换规则。
- 仲裁使用 AI 助手当前配置的 LLM 客户端；如果未配置助手 LLM，保持现有错误提示。
- 仲裁失败、超时或返回无效 JSON 时，必须保守跳过，继续用本地个性化后的文本进入助手主请求。
- LLM 仲裁耗时应计入该轮 `llm_time_ms`，避免历史记录低估助手处理耗时。
- 对 `PersonalizationCorrectionPair` 的真实 LLM apply/reject 结果，继续写回 correction pair 弱反馈；跳过、超时、缺失 decision、重叠本地拒绝不写反馈。
- 不改变普通听写路径行为。

## Acceptance Criteria

- `AssistantProcessor` 可复用现有候选仲裁实现，不新增第二套 prompt。
- `handle_assistant_mode` 先合并 TNL 和个性化 diagnostics，再执行助手候选仲裁。
- 中置信候选被 LLM 接受后，进入助手主 LLM 的 `user_instruction` 已是仲裁后的文本。
- 相关 Rust 单测覆盖：
  - 助手个性化 helper 导出 `PendingLlm` diagnostics。
  - LLM 仲裁 helper 仍由普通听写和助手共享。
  - LLM 仲裁反馈提取逻辑覆盖助手路径可用的 diagnostics。
- 运行目标 Rust 测试与 `cargo check`。

## Out of Scope

- 不引入完整 ConvertPipeline trait 化。
- 不改变前端结果面板结构。
- 不把 TNL diagnostics 展示到助手 UI。
- 不新增本地小模型或 reranker。
