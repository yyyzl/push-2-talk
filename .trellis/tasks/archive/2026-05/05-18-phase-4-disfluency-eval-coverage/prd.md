# Phase 4 Disfluency Eval Coverage

## Goal

将 Phase 4 言语流畅化纳入正式 ASR eval 闭环，避免当前只靠 Rust 单元测试覆盖清洗规则。`eval_asr` 应能按 disfluency mode 对 `raw_asr_text` 做与运行时一致的早期清洗，再进入个性化二次解码，并用正式 case 覆盖填充词清理、Aggressive 重复字/false start，以及保守模式误删保护。

## What I Already Know

* `src-tauri/src/tnl/disfluency.rs` 已有 `clean_disfluency(text, mode)` 和 `DisfluencyMode::{Off, Conservative, Aggressive}`。
* 普通听写路径先跑 `TnlEngine::normalize`，随后再跑 personalization；当前 `src-tauri/src/bin/eval_asr.rs` 只直接调用 `PersonalizationEngine::convert`，没有模拟 disfluency 预清洗。
* `.trellis/spec/backend/tnl-normalization.md` 已要求 disfluency 先于 Unicode normalization、tokenization、technical span、phonetic replacement 等步骤运行。
* `tests/asr_eval/cases/` 当前只有技术词和误伤保护样本，没有口语填充词样本。

## Requirements

* `eval_asr` 增加 disfluency mode 配置，默认使用 Conservative，以贴近运行时默认行为。
* `eval_asr` 在调用 `PersonalizationEngine::convert` 前先执行 disfluency 清洗；`Off` 模式必须保持原文本。
* eval 配置和 diagnostics 顶层 payload 应记录当前 disfluency mode，便于后续报告复现。
* 新增正式 eval cases 覆盖：
  * Conservative 句首填充词清理后仍能命中个性化纠错。
  * Aggressive 的重复字/false start 清理。
  * `这个东西`、`嗯哼` 等误删保护。
* 文档同步 Phase 4 当前状态，说明 eval gate 已覆盖 disfluency。

## Acceptance Criteria

* [x] `cargo test --bin eval_asr` 通过。
* [x] `cargo run --bin eval_asr -- --suite ../tests/asr_eval --allow-quality-gate-failure` 通过并包含新增 disfluency cases。
* [x] `cargo run --bin eval_asr -- --suite ../tests/asr_eval --disfluency-mode off --allow-quality-gate-failure` 可运行，且 Off 模式可用于验证不清洗路径。
* [x] `npx tsx scripts/asr-eval-readiness.ts --suite tests/asr_eval --allow-not-ready --json` 仍能读取新增 cases。
* [x] 文档和 spec 与新 CLI 行为一致。

## Definition of Done

* Tests added/updated.
* 最小相关 Rust 测试通过。
* ASR eval smoke 通过。
* GitNexus impact / detect changes 按项目要求执行。
* 只提交本任务文件，不包含既有并行脏文件。

## Technical Approach

优先采用最小 API 暴露：让 `eval_asr` 可以复用 TNL disfluency 清洗函数，而不是复制规则。CLI 增加 `--disfluency-mode off|conservative|aggressive`；single eval 和 sweep 都使用相同 mode。新增 case 文件可以命名为 `disfluency_mini.json`，避免改动既有技术词 fixture。

## Decision (ADR-lite)

**Context**: Phase 4 已有运行时实现，但 ASR eval gate 当前绕过 TNL disfluency，导致后续 Phase 7/8 gate 不能反映口语化清洗收益或误伤风险。

**Decision**: 在 `eval_asr` 中加入轻量 disfluency 预清洗，并把 mode 写入 eval config/diagnostics。

**Consequences**: eval runner 更贴近运行时链路；同时会让 case 总数上升，readiness gate 的正式样本数量也会随之更新。该任务不改变线上默认配置。

## Out of Scope

* 不新增复杂 disfluency 规则。
* 不引入主观整洁度打分模型。
* 不启动 Phase 8 reranker。
* 不删除 `AppConfig.dictionary` 或处理 Phase 5 剩余数据库清理。

## Technical Notes

* Relevant spec: `.trellis/spec/backend/tnl-normalization.md`
* Likely files: `src-tauri/src/bin/eval_asr.rs`, `src-tauri/src/tnl/mod.rs`, `tests/asr_eval/cases/*.json`, `ASR_PERSONALIZATION_QUALITY_LEAP.md`
* Existing unrelated dirty files must remain untouched: `AGENTS.md`, `CLAUDE.md`, `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `.trellis/.runtime/`
