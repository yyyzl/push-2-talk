# Phase 0B Readiness Report Output

## Goal

增强刚新增的 ASR eval readiness gate，让它能把当前正式样本数量门槛检查结果写入报告文件，便于 CI、归档和后续 Phase 8 决策审阅。

## What I Already Know

- `scripts/asr-eval-readiness.ts` 已能输出 text 或 JSON 摘要。
- 当前 mini suite 是 26/80，默认 not ready，`--allow-not-ready` 可用于报告场景。
- 现有 draft CLI 已有 `--out` 写文件模式，可借鉴但不复用 draft promotion 逻辑。

## Requirements

- `scripts/asr-eval-readiness.ts` 支持 `--out <path>`。
- `--out` 写入的内容应与 `--json` / text 输出格式一致。
- 父目录不存在时自动创建。
- 写文件不改变 readiness 判定：未达标且没有 `--allow-not-ready` 时仍失败，不写成功报告。
- 单元测试覆盖 text/json 输出文件、父目录创建、not-ready failure 不写文件。
- 同步 roadmap/spec 文档中的 CLI 说明。

## Acceptance Criteria

- [x] `npx tsx --test tests/asrEvalReadiness.test.ts` 通过。
- [x] `npm run test:ts` 通过。
- [x] 手动运行 `--out` 可生成 JSON readiness 报告。
- [x] GitNexus staged detect_changes 为低风险或符合预期。

## Out of Scope

- 不新增真实 eval case。
- 不改 readiness 默认 80 条门槛。
- 不改 `package.json` scripts，避免混入当前已有 dirty 文件。
- 不调用云端 ASR。

## Technical Notes

- 预计修改：`scripts/asrEvalReadinessCore.ts`、`scripts/asr-eval-readiness.ts`、`tests/asrEvalReadiness.test.ts`、`ASR_PERSONALIZATION_QUALITY_LEAP.md`、`.trellis/spec/backend/tnl-normalization.md`。
