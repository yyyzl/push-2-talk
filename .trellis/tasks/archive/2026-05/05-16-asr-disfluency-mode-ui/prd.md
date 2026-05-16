# 接入言语流畅化三档 UI 开关

## Goal

确认 Phase 4 口语流畅化的三档开关是否已经完成前后端闭环，并同步路线图状态，避免继续在已完成事项上重复开发。

## What I Already Know

- 后端已有 `clean_disfluency(text, mode)` 与 `DisfluencyMode::{Off, Conservative, Aggressive}`。
- `TnlConfig.disfluency_mode` 已存在于后端配置和前端 `TnlConfig` 类型。
- `PreferencesPage` 已展示 Off / Conservative / Aggressive 三段按钮，并通过 `onSetDisfluencyMode` 调用保存。
- `App.tsx` 已通过 `saveFieldPatchWithStatus({ tnlConfig: { disfluencyMode: mode } })` 走字段级 patch。
- `tests/configSaveReviewFixes.test.ts` 已有 A14 回归，检查前后端闭环关键源码契约。

## Requirements

- 不重复实现已存在 UI。
- 跑现有相关 TypeScript 回归，确认三档 UI/配置闭环仍然成立。
- 如果验证通过，更新路线图中 “UI 三档开关尚未接入” 的状态。

## Acceptance Criteria

- `npm run test:ts` 或相关目标测试通过。
- `ASR_PERSONALIZATION_QUALITY_LEAP.md` 的 Phase 4 状态与当前代码一致。

## Out of Scope

- 不新增新的流畅化规则。
- 不调整 UI 视觉设计。
- 不改后端清洗算法。
