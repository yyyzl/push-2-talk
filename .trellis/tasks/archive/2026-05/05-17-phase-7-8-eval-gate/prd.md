# Phase 7 to Phase 8 evaluation gate

## Goal

在 Phase 0-7 已完成主要本地个性化解码能力之后，先跑一次可复现的评测门槛，判断是否有足够证据启动 Phase 8 本地 reranker。这个任务不直接引入 ONNX / KenLM / 本地 LLM，而是产出当前 mini suite 的 Phase 7 gate 报告、诊断文件和后续决策。

## What I already know

* `ASR_PERSONALIZATION_QUALITY_LEAP.md` 明确 Phase 8 是可选项，只在 Phase 0-7 全部做完且评测集仍有明显残留问题时才上。
* 现有 `eval_asr` runner 支持默认 suite、diagnostics 输出、threshold/window sweep、禁用 pass 的 ablation。
* 当前 `tests/asr_eval/cases/` 只有 26 条 mini suite 样本，不是 80-120 条正式真实样本集。
* 现有 `tests/asr_eval/baseline_report.md` 已记录 Week 1 mini eval，结果是 26/26 通过，主要验证本地 second-decoding 固定文本闭环。
* Phase 7 HotwordCompiler 已审计完成；剩余风险是没有用评测数据量化是否值得进入 Phase 8。

## Assumptions

* 本轮以“当前可复现 mini suite”作为 Phase 7 gate，不伪造真实语音/ASR 样本。
* 如果 mini suite 没有残留失败，Phase 8 暂不启动；后续应先补 Phase 0B 真实样本，再重新决策。
* 本任务可以产出报告和文档同步，不需要改动运行时解码逻辑。

## Requirements

* 跑默认 `eval_asr` suite，记录准确率、候选分布、延迟和 quality gate。
* 跑 diagnostics 输出，确认 schema v4 文件生成并包含 metrics / quality_gate / eval_config。
* 跑 threshold/window sweep，记录当前阈值和窗口对结果的影响。
* 跑禁用 `syllable_match` 的 ablation，保留允许失败输出，量化本地二次解码贡献。
* 新增一份 Phase 7 gate 报告，明确“是否进入 Phase 8”的结论和限制。
* 同步 `ASR_PERSONALIZATION_QUALITY_LEAP.md` 的 Phase 7 / Phase 8 决策状态。

## Acceptance Criteria

* [x] 默认 suite 命令通过 quality gate。
* [x] diagnostics 文件成功生成并可解析。
* [x] sweep 命令完成并输出对比表。
* [x] ablation 命令完成并展示禁用 syllable match 后的退化情况。
* [x] 报告文件记录命令、关键指标、结论和未覆盖风险。
* [x] 主方案文档同步 Phase 7 gate 状态，且不宣称已经完成 80-120 条正式真实样本。

## Definition of Done

* 相关评测命令已运行。
* 文档和报告只包含本任务产生或确认的信息。
* `cargo check` 或等价最小编译检查通过。
* GitNexus detect_changes 在提交前运行。
* 本任务变更被独立提交，不包含既有并行脏文件。

## Out of Scope

* 不实现 Phase 8 `LocalRerankerPass`。
* 不引入 ONNX / KenLM / 本地小模型依赖。
* 不伪造或合成 80-120 条真实 ASR 样本。
* 不删除 `AppConfig.dictionary`，不做 Phase 5 剩余清理。

## Technical Notes

* Runner: `src-tauri/src/bin/eval_asr.rs`
* Suite: `tests/asr_eval/`
* Existing baseline: `tests/asr_eval/baseline_report.md`
* Main plan: `ASR_PERSONALIZATION_QUALITY_LEAP.md`
