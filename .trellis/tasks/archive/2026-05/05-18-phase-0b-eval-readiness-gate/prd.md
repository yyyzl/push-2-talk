# Phase 0B Eval Readiness Gate

## Goal

为 Phase 8 之前的决策补一个自动化 readiness gate：统计正式 ASR eval cases 的数量，默认要求至少 80 条真实/正式样本，避免因为 26 条 mini suite 通过就误判可以启动本地 reranker。

## What I Already Know

- `ASR_PERSONALIZATION_QUALITY_LEAP.md` 明确写到 Phase 8 暂不启动，下一步应优先扩展 Phase 0B 真实样本集。
- 现有 `scripts/asr-eval-draft.ts` 已能生成 draft、approved-only promotion 到正式 cases。
- 当前正式 cases 目录是 `tests/asr_eval/cases/`，mini suite 约 26 条，不等于完整 80-120 条真实样本集。
- `npm run test:ts` 已覆盖 TypeScript runtime tests。

## Requirements

- 新增一个轻量 CLI，可检查指定 cases 目录或 suite 目录下的正式 eval case 数量。
- 默认目标最小 case 数为 80，可通过参数覆盖。
- 输出人类可读摘要，并支持 JSON 输出，便于后续 CI/脚本消费。
- 未达到门槛时默认非零退出；提供 allow/not-ready 参数用于报告生成场景。
- 不读取音频、不调用 ASR、不修改 cases，不触碰生产识别链路。

## Acceptance Criteria

- [x] 单元测试覆盖：ready、not ready、suite/cases 路径解析、invalid min。
- [x] CLI 默认检查 `tests/asr_eval` 或用户指定路径。
- [x] 当前仓库 mini suite 运行 readiness 时能明确报告 not ready。
- [x] `npm run test:ts` 通过。

## Out of Scope

- 不新增真实 eval 样本。
- 不自动 promote draft。
- 不启动 Phase 8 reranker。
- 不调用任何云端 ASR。

## Technical Notes

- 预计新增：`scripts/asrEvalReadinessCore.ts`、`scripts/asr-eval-readiness.ts`、`tests/asrEvalReadiness.test.ts`。
- 相关 spec：`.trellis/spec/backend/tnl-normalization.md` 的 ASR Eval Draft Intake/Phase 0B 场景。
