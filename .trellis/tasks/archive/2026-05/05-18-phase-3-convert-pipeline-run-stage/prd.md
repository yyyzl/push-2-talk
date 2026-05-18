# Phase 3 ConvertPipeline run stage

## Goal

继续推进 Phase 3 ConvertPipeline trait 化：在上一任务已经让 `ConvertPipeline` 接管 pass 候选收集之后，本任务让 `ConvertPipeline` 进一步接管候选排序、阈值判定、overlap 选择、文本替换和 pass applied count 更新。`PersonalizationEngine` 仍保留现有 public API，只负责构造 context 并委托 pipeline。

## What I already know

* 当前 `ConvertPipeline` 已有 `ExactTextPass` 和 `SyllableMatchPass`，默认顺序稳定为 `exact_text` → `syllable_match`。
* `PersonalizationEngine::convert_with_technical_spans` 仍直接负责候选排序、`apply_threshold` 判定、overlap skip、`replace_range`、`update_pass_applied_counts`。
* `.trellis/spec/backend/tnl-normalization.md` 当前 skeleton 契约要求 pass 不直接 mutate output，也要求 selection/replacement 不能分散到各 pass。
* 现有 `personalization::engine` 单测覆盖 exact fallback、syllable alias、named entity boost、overlap rank、threshold 不被 rank 绕过、pass summaries。

## Assumptions

* 本任务只移动职责边界，不改变候选排序规则、分数、阈值、诊断字段或 runtime API。
* `ConvertPipeline::run` 可以作为内部 API 返回完整 `ConversionResult`。
* 空文本早退继续留在 `PersonalizationEngine`，保持现有 default diagnostics 行为。

## Requirements

* 新增 `ConvertPipeline::run(&self, context: &ConvertContext<'_>) -> ConversionResult`。
* `run` 内部调用 `collect_candidates`，然后完成现有排序、阈值判定、overlap 选择、文本替换、applied count 更新。
* `PersonalizationEngine::convert_with_technical_spans` 构造 `SyllableLattice`、windows 和 `ConvertContext` 后直接调用 `ConvertPipeline::default().run(&context)`。
* `ConvertPass` 仍只负责收集候选，不能直接应用替换。
* `ConvertContext` 必须包含 source text、windows、store、config、technical spans。
* `PassDiagnostics` 的 `candidate_count` / `applied_count` 和 serialized pass names 保持兼容。
* 更新 `.trellis/spec/backend/tnl-normalization.md`，把 skeleton 契约从 collect-only 推进为 pipeline owns selection/replacement。

## Acceptance Criteria

* [x] `PersonalizationEngine::convert` 和 `convert_with_technical_spans` 现有行为全量保持。
* [x] 有单测直接覆盖 `ConvertPipeline::run` 产生完整 `ConversionResult`。
* [x] 现有 pass order、disabled summary、named entity boost、overlap rank、threshold 单测继续通过。
* [x] `cargo test personalization::engine` 通过。
* [x] `cargo fmt --check` 通过。
* [x] `cargo check` 通过。
* [x] `git diff --check` 通过。

## Definition of Done

* GitNexus impact analysis 在编辑相关符号前运行。
* `trellis-before-dev` 在实现前读取相关规范。
* `trellis-check` 在提交前运行。
* 新契约同步到 `.trellis/spec/backend/tnl-normalization.md`。
* 本任务独立提交，不包含既有并行脏文件。

## Out of Scope

* 不把 `TnlEngine::normalize`、disfluency cleaner 或 LLM arbiter 迁入 ConvertPipeline。
* 不拆分 `engine.rs` 到新模块。
* 不新增任何候选 pass。
* 不调整分数、阈值、rank 或 eval metrics。
* 不处理 `AppConfig.dictionary` 删除或 Phase 8 reranker。

## Technical Notes

* 主要文件：`src-tauri/src/personalization/engine.rs`。
* 相关规范：`.trellis/spec/backend/tnl-normalization.md`。
* 本地研究记录：`research/current-state.md`。
