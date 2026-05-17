# Phase 0B ASR eval sample intake

## Goal

为 Phase 0B 的 80-120 条真实 ASR 评测集补一个低风险样本入口：从前端导出的 history JSON 或运行时 personalization diagnostics JSON 生成“待人工复核”的 eval draft。这个任务不把草稿直接并入正式 `tests/asr_eval/cases/`，避免未经确认的真实文本污染质量门槛。

## What I already know

* Phase 7 → Phase 8 mini gate 已通过 26/26，但结论是样本规模不足，下一步应扩展 Phase 0B 真实样本。
* 现有正式 eval case schema 需要 `raw_asr_text`、`expected_text`、`user_final_text`、`category` 等字段。
* 前端 history 存在 `originalText`、`polishedText`、`mode`、`success`、`tnlDiagnostics` 等字段，但 history 位于 WebView localStorage，适合通过导出文件喂给离线工具。
* 后端 runtime personalization diagnostics 有 `source_text`、`output_text`、`changed`、candidate/pass summary 等字段，可作为草稿线索，但不能自动代表用户最终正确文本。
* 当前 `scripts/` 只有 icon 转换脚本；TS 测试用 `tsx --test tests/*.test.ts`。

## Assumptions

* 本轮实现一个 repo-local Node/tsx 工具，而不是改 Rust eval runner。
* 工具输出 draft JSON，默认放在 `tests/asr_eval/drafts/`，由人工确认后再复制/整理到正式 cases。
* `expected_text` 可以从 history `polishedText` 或 diagnostics `output_text` 预填，但必须在 `notes` 中标明需要人工确认。

## Requirements

* 新增可复用的 TypeScript 模块，能把 history record / runtime diagnostics 转换为 eval draft case。
* 新增 CLI 脚本，支持：
  * `--history <file>` 读取前端 history JSON 数组；
  * `--diagnostics <file-or-dir>` 读取单个 diagnostics JSON 或目录内 `personalization-*.json`；
  * `--out <file>` 写入 draft JSON；
  * `--limit <n>` 限制输出数量；
  * `--prefix <id-prefix>` 控制 `audio_id` 前缀。
* 草稿过滤规则：
  * 跳过空 `raw_asr_text`；
  * history 默认只收 `success = true` 且 `mode = "normal"` 的记录；
  * diagnostics 默认只收 `changed = true` 或有 applied candidate 的记录；
  * 对 `raw_asr_text + expected_text` 做去重。
* 草稿输出必须是合法 eval case shape，但 `audio_wav_path` 为 `null`，`provider` 明确标记来源为 draft。
* 增加 TypeScript 单测覆盖 history、diagnostics、去重、CLI 输出。
* 更新 ASR 方案文档，说明 Phase 0B 当前进入“样本草稿入口已完成，仍需人工确认真实样本”的状态。

## Acceptance Criteria

* [x] `scripts/asr-eval-draft.ts` 可从 history JSON 生成 draft cases。
* [x] `scripts/asr-eval-draft.ts` 可从 runtime diagnostics JSON/目录生成 draft cases。
* [x] 单测覆盖核心转换逻辑和 CLI 写文件路径。
* [x] 生成的 draft case 字段能被现有 eval schema 理解，且不会默认写入正式 cases。
* [x] 主方案文档同步 Phase 0B intake 状态，且不宣称真实样本集已完成。

## Definition of Done

* `npm run test:ts -- asrEvalDraft.test.ts` 或等价目标测试通过。
* `npm run build` 或相关 TypeScript 检查通过；若脚本不在 build include 内，至少运行全量 TS runtime tests。
* `cargo check` 保持通过。
* GitNexus impact/detect_changes 在改动/提交前运行。
* 本任务变更独立提交，不包含既有并行脏文件。

## Out of Scope

* 不自动从 WebView localStorage 读取 history。
* 不把 draft 自动并入 `tests/asr_eval/cases/`。
* 不收集音频文件或上传任何真实内容。
* 不修改 `eval_asr` Rust runner 的质量门槛。

## Technical Notes

* 正式 eval case 示例：`tests/asr_eval/cases/*.json`
* Runtime diagnostics writer：`src-tauri/src/personalization/runtime_diagnostics.rs`
* Frontend history type：`src/types/index.ts` 中的 `HistoryRecord`
* 建议输出目录：`tests/asr_eval/drafts/`
