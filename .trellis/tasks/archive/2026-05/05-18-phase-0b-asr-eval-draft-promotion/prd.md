# Phase 0B ASR eval draft promotion

## Goal

补齐 Phase 0B 样本入口的下一步：草稿生成后必须经过人工复核，只有标记为 approved 的 draft case 才能被提升为正式 eval case。这个任务继续避免“自动把真实运行文本塞进质量门槛”，同时让 80-120 条真实样本的整理流程可执行。

## What I already know

* `scripts/asr-eval-draft.ts` 已能从 history JSON / runtime personalization diagnostics 生成 draft cases。
* `.trellis/spec/backend/tnl-normalization.md` 已约束 draft intake 不能直接写进正式 `tests/asr_eval/cases/`。
* 当前 draft case 的 `notes` 已提示需要人工确认 `expected_text`，但缺少机器可验证的 approval 标记。
* 现有 `eval_asr` 会忽略 JSON 里的未知字段；为了正式 cases 干净，promotion 输出应只保留 eval case 字段。

## Assumptions

* 人工复核通过在 draft JSON 中设置 `review_status: "approved"`。
* 未复核、拒绝或字段不完整的 draft 不应出现在 promotion 输出里。
* Promotion 仍是离线脚本能力，不接 UI，不自动读取 WebView localStorage。

## Requirements

* draft 生成时新增 `review_status: "needs_review"` 和 `review_notes: ""`，让人工复核入口明确。
* 同一 CLI 新增 `--promote <draft.json>` 模式：
  * 只读取 draft JSON；
  * 只提升 `review_status = "approved"` 的记录；
  * 输出正式 eval case shape，去掉 `review_status` / `review_notes` 等 draft-only 字段；
  * 保留 `audio_id`、`raw_asr_text`、`expected_text`、`user_final_text`、`category`、`notes` 等核心字段。
* CLI 需要拒绝混用 `--promote` 和 `--history` / `--diagnostics`。
* CLI promotion 模式在没有 approved case 时失败并不写出空正式文件。
* 单测覆盖 draft 默认 review 状态、approved promotion、未 approved 跳过、promotion CLI 输出。
* 更新 spec 和主方案文档，明确 Phase 0B 现在有 draft + approved promotion 两段式流程。

## Acceptance Criteria

* [x] 新生成的 history/diagnostics draft case 带 `review_status: "needs_review"`。
* [x] `--promote` 只输出 approved draft，并去掉 draft-only 字段。
* [x] 未 approved 的 draft 不会进入正式输出。
* [x] `--promote` 与 `--history` / `--diagnostics` 混用时失败。
* [x] 没有 approved case 时 promotion 失败且不写空文件。
* [x] 单测覆盖核心转换和 CLI promotion。
* [x] 文档/spec 同步两段式流程。

## Definition of Done

* `npm run test:ts -- asrEvalDraft.test.ts` 通过。
* `npm run build` 通过。
* `cargo check` 通过。
* `git diff --check` 通过。
* GitNexus impact/detect_changes 在提交前运行。
* 本任务变更独立提交，不包含既有并行脏文件。

## Out of Scope

* 不设计 review UI。
* 不自动修改或合并现有正式 case 文件。
* 不自动采集真实语音、history 或 diagnostics 内容。
* 不修改 Rust `eval_asr` runner。

## Technical Notes

* Core module: `scripts/asrEvalDraftCore.ts`
* CLI entry: `scripts/asr-eval-draft.ts`
* Tests: `tests/asrEvalDraft.test.ts`
* Eval intake spec: `.trellis/spec/backend/tnl-normalization.md`
