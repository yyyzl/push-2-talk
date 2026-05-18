# Phase 3 ConvertPipeline trait skeleton

## Goal

把当前 `PersonalizationEngine` 内部已经存在的 `exact_text` / `syllable_match` 分级候选收集逻辑，收拢成一个小而可测试的 `ConvertPass` / `ConvertPipeline` 骨架。这个任务不改变普通听写、助手路径、eval runner 的外部 API，只为后续完整 ConvertPipeline trait 化提供安全落点。

## What I already know

* Phase 0-7 的主要个性化能力已经落地，主方案里仍把“完整 ConvertPipeline trait 化”列为剩余增强项。
* `src-tauri/src/personalization/engine.rs` 里的 `collect_candidates` 已经按 `exact_text` 和 `syllable_match` 两段工作，并输出 `PassDiagnostics`。
* `PersonalizationEngine::convert` / `convert_with_technical_spans` 是当前 normal pipeline、assistant path、eval runner 共用的稳定入口。
* `.trellis/spec/backend/tnl-normalization.md` 已要求 future ConvertPipeline pass 复用 `SyllableLattice::windows()`，并保持 pass toggles、named-entity boost、candidate diagnostics 行为。

## Assumptions

* 本任务只做内部结构骨架，不把 TNL base normalize、LLM arbiter、disfluency cleaner 一起迁入 ConvertPipeline。
* `ConvertPass` / `ConvertPipeline` 先保持 crate-private/internal，不急于暴露公共 API。
* 为降低风险，本轮候选生成、排序、阈值、overlap selection、diagnostics shape 必须等价。

## Requirements

* 新增内部 `ConvertPass` trait，至少表达 pass 名称、启用状态、候选收集三个职责。
* 新增内部 `ConvertPipeline`，默认按固定顺序运行 `ExactTextPass` 再运行 `SyllableMatchPass`。
* `PersonalizationEngine::collect_candidates` 委托 `ConvertPipeline`，外部 `PersonalizationEngine` API 不变。
* `PassDiagnostics` 继续包含 `exact_text` 和 `syllable_match`，禁用 pass 仍输出 enabled=false、候选和 applied 为 0 的 summary。
* `SyllableMatchPass` 继续复用 `SyllableLattice::windows(self.config.max_window_tokens)`，不得重新实现 token/window 逻辑。
* NamedEntity overlap score boost、manual/common-word guard、context rank、frequency rank、dedupe 行为保持不变。
* 同步 `.trellis/spec/backend/tnl-normalization.md`，记录 ConvertPipeline skeleton 的可执行契约。

## Acceptance Criteria

* [x] `PersonalizationEngine::convert` 和 `convert_with_technical_spans` 的现有行为保持通过。
* [x] 有单测证明 pipeline 默认 pass 顺序为 `exact_text` 后 `syllable_match`。
* [x] 有单测证明禁用 syllable pass 时仍保留 exact_text summary 和 disabled syllable summary。
* [x] 有单测证明 named-entity boost 仍只影响 syllable-match candidate。
* [x] `cargo test personalization::engine` 通过。
* [x] `cargo check` 通过。
* [x] `cargo fmt --check` 通过。
* [x] `git diff --check` 通过。

## Definition of Done

* GitNexus impact analysis 在编辑 `PersonalizationEngine` 相关符号前运行。
* `trellis-before-dev` 在实现前读取相关规范。
* `trellis-check` 在提交前运行。
* 新知识同步到 `.trellis/spec/backend/tnl-normalization.md`。
* 本任务变更独立提交，不包含既有并行脏文件。

## Out of Scope

* 不迁移 `TnlEngine::normalize` 到 ConvertPipeline。
* 不引入新的 replacement pass。
* 不调整 apply threshold、rank_score、candidate score、LLM 仲裁逻辑。
* 不删除 `AppConfig.dictionary`，不处理 Phase 5 剩余迁移。
* 不启动 Phase 8 reranker。

## Technical Notes

* 主要文件：`src-tauri/src/personalization/engine.rs`。
* 相关规范：`.trellis/spec/backend/tnl-normalization.md`。
* 本地研究记录：`research/current-state.md`。
