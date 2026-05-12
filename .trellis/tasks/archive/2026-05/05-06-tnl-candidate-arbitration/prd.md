# brainstorm: TNL 候选仲裁系统

## Goal

在现有 ASR 热词、TNL 规范化、LLM 词库增强和自动学习能力之上，设计一个低延迟的候选系统：本地规则负责召回可能的热词误识别，后端对候选做置信度和风险分层，必要时用小模型仲裁，最终在前端可观察地呈现自动替换、模型仲裁和保守保留的结果。

## What I already know

* 用户场景是按下说话，ASR 加可选大模型润色的端到端延迟目标最好在 1.x 秒，最长可接受约 2 秒。
* 典型问题包括 `Cloud Code` 被识别为 `Claude Code` 的音近热词纠错，以及 `Cursor`、`Gemini-3-Flash` 等中英混合技术名词。
* 现有项目已经有 ASR hotwords/context 注入、本地 TNL、LLM 词库增强、自动词库学习、内置词库领域选择。
* 用户特别强调需要考虑前后端交互，而不是只做后端文本改写。

## Assumptions (temporary)

* MVP 应优先复用现有 `Vec<String>` 词库和 TNL 能力，避免第一步就做大规模词库 schema 迁移。
* 候选系统应该默认保守：高置信本地替换，中置信才进入 LLM 仲裁，低置信不动。
* 候选仲裁不应替代现有语句润色；它应该在 TNL 后、全文润色前或作为词库增强的轻量路径运行。
* 前端至少需要展示或记录候选决策摘要，便于用户理解为什么某个词被替换或没被替换。

## Open Questions

* 无阻塞问题；等待用户确认后进入实现。

## Requirements (evolving)

* 后端生成结构化候选，包含原片段、目标词、位置、来源规则、分数、风险、证据。
* 后端区分自动应用、需要仲裁、拒绝/保留三类结果。
* LLM 只判断候选，不自由改写全文；请求体必须短、可超时、可回退。
* 前后端类型需要同步，前端能消费候选决策结果用于历史记录、调试或轻量提示。
* 端到端延迟预算需要被设计约束保护：无候选不调用 LLM，有候选也要设置短超时和候选数量上限。
* 前端交互必须保留现有 `transcription_complete` 兼容性，用可选诊断字段扩展，而不是改变现有文本字段语义。
* MVP 前端可见性限定为历史记录展示：低打扰，用于先验证候选替换效果和诊断价值。
* MVP 开关策略绑定现有“词库增强”：只有开启词库增强且存在中置信候选时才调用 LLM 候选仲裁；本地高置信 TNL 仍由 TNL 开关控制。

## Acceptance Criteria (evolving)

* [ ] `Cloud Code` 在热词库包含 `Claude Code` 且开发上下文明确时，可以稳定得到 `Claude Code`。
* [ ] 当原始片段已经精确命中词库合法词时，音近匹配不得再把它替换成另一个相近词，例如词库同时包含 `Grok` 和 `Groq` 时，输入 `Grok` 必须保留为 `Grok`。
* [ ] 常见英文词和普通中文短语不会因为相似度边缘命中而被误替换。
* [ ] 无候选文本不增加 LLM 请求。
* [ ] LLM 仲裁失败或超时时，系统保守回退到本地高置信结果。
* [ ] 历史记录可查看本次文本处理中的候选/替换决策摘要。
* [ ] 关闭词库增强时，不产生额外 LLM 候选仲裁请求。

## Definition of Done

* Tests added/updated (unit/integration where appropriate)
* Lint / typecheck / CI green
* Docs/notes updated if behavior changes
* Rollout/rollback considered if risky

## Out of Scope (explicit)

* 第一版不要求替换 ASR 服务商或引入新的 ASR 引擎。
* 第一版不要求让 LLM 做全文自由润色的替代方案。
* 第一版不默认做大型词库管理 UI 重写，除非后续确认 MVP 需要。
* 第一版不在实时悬浮窗或当前结果面板中展示候选状态，避免打断按键听写主流程。

## Technical Notes

* 需要重点分析 `src-tauri/src/tnl/`、`src-tauri/src/pipeline/normal.rs`、`src-tauri/src/llm_post_processor.rs`、`src-tauri/src/learning/`、`src/types/index.ts`、`src/hooks/useTauriEventListeners.ts`、历史记录相关组件。
* 需要用 GitNexus 查看 TNL、普通听写 pipeline、配置热更新、历史记录事件等执行流，确认候选结果应该从哪个边界返回给前端。

## Research References

* [`research/codebase-candidate-system-analysis.md`](research/codebase-candidate-system-analysis.md) — 当前代码链路、GitNexus 影响范围、候选系统后端分层和前端交互建议。

## Research Notes

### Current constraints

* `TnlEngine.normalize` 影响范围为 CRITICAL，需要严格保持现有行为兼容并补充回归测试。
* 后端/前端 `TranscriptionResult` 影响范围很大，候选诊断应作为可选字段附加。
* `PipelineResult` 是后端管道向前端事件转换前的自然扩展点。

### Recommended MVP approach

* 保留现有高置信 TNL 替换路径。
* 新增候选召回/评分结构，输出中置信候选。
* 新增轻量 LLM 候选仲裁，仅返回 JSON 决策。
* `transcription_complete` 携带可选 `tnl_diagnostics`。
* 前端只在历史记录中展示候选摘要，不打扰实时听写主流程。

## Decision (ADR-lite)

**Context**: 候选系统需要前端可观察，但按键听写的主体验要求低打扰和低延迟。

**Decision**: MVP 只在历史记录中展示候选/替换决策摘要；不在实时悬浮窗或当前结果面板展示候选状态。

**Consequences**: 实现范围更小，适合先验证算法效果；实时调试反馈较弱，后续如果候选系统稳定，可再扩展到结果面板或设置页诊断视图。

### Switch Strategy

**Context**: 候选仲裁可能调用 LLM，涉及延迟和用户对网络请求的预期。

**Decision**: MVP 绑定现有“词库增强”开关，不新增独立 UI 开关。开启词库增强时，候选仲裁可作为词库增强的轻量前置步骤运行；关闭词库增强时，不发起 LLM 候选仲裁。

**Consequences**: 配置复杂度低，符合用户预期；本地高置信 TNL 仍可独立运行。后续如果候选仲裁足够稳定，再考虑拆成独立开关或高级设置。
