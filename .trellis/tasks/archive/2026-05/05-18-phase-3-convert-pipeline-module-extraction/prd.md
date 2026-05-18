# Phase 3 ConvertPipeline Module Extraction

## Goal

把已经接管候选收集、排序、阈值、overlap 选择和文本替换的 `ConvertPipeline` 从 `PersonalizationEngine` 文件中抽成独立内部模块，为后续 Phase 4~8 增加更多本地输入法式 pass 留出清晰边界，同时保持现有运行时入口和转换行为完全不变。

## What I Already Know

- `PersonalizationEngine::convert` 和 `convert_with_technical_spans` 仍是生产入口。
- 当前 `engine.rs` 内部已有 `ConvertContext`、`ConvertPass`、`ConvertPipeline`、`ExactTextPass`、`SyllableMatchPass`。
- `ConvertPipeline::run` 已拥有完整转换流程：候选收集、排序、阈值判断、overlap 跳过、反向替换、pass applied 计数。
- `.trellis/spec/backend/tnl-normalization.md` 已记录 ConvertPipeline 相关契约，但还描述为主要位于 `engine.rs` 内部。

## Requirements

- 新增 `src-tauri/src/personalization/convert_pipeline.rs`，承载 pipeline 私有实现。
- `engine.rs` 保留公共类型、配置和 `PersonalizationEngine` API，转换时只构造 lattice/windows/context 并委托 pipeline。
- `ConvertPipeline::default()` pass 顺序保持 `exact_text` -> `syllable_match`。
- 候选诊断、pass summary、阈值、overlap、named entity 保护、context rank bonus 行为保持不变。
- 迁移或保留测试，确保模块抽离后 pass 顺序和完整 conversion result 仍被覆盖。
- 同步 roadmap/spec 文档，明确 pipeline 已抽到 dedicated internal module。

## Acceptance Criteria

- [x] `PersonalizationEngine` 对外 API 不变。
- [x] `ConvertPipeline` 相关测试仍通过，并可覆盖新模块。
- [x] 相关 personalization/assistant 测试通过。
- [x] `cargo fmt --check` 和 `cargo check` 通过。
- [x] GitNexus 变更检测显示影响范围符合预期。

## Out of Scope

- 不新增新的 personalization pass。
- 不改变 TNL/LLM 仲裁策略。
- 不改变配置 schema、默认阈值或运行时开关。
- 不改前端 UI。

## Technical Notes

- 主要文件：`src-tauri/src/personalization/engine.rs`、`src-tauri/src/personalization/mod.rs`、新增 `convert_pipeline.rs`。
- 文档同步：`ASR_PERSONALIZATION_QUALITY_LEAP.md`、`.trellis/spec/backend/tnl-normalization.md`。
- 后端 spec 适用：`.trellis/spec/backend/tnl-normalization.md`，shared guide 适用 code reuse thinking。
