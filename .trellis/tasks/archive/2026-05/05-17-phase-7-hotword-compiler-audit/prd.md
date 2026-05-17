# Phase 7 HotwordCompiler Audit And Status Sync

## Goal

Audit the current Phase 7 HotwordCompiler implementation against `ASR_PERSONALIZATION_QUALITY_LEAP.md`, then synchronize the plan document with the real landed state. Avoid speculative rewrites because the repo already has HotwordCompiler, provider wiring, recent hotwords, app-context hotwords, and correction-pair integration.

## What I Already Know

- `src-tauri/src/personalization/hotword_compiler.rs` exists and defines ASR, TNL, and LLM context packs.
- Qwen HTTP/Reatime and Doubao HTTP/Realtime provider paths already consume `compile_asr_pack_with_correction_pairs`.
- Provider limits are encoded as `QWEN_*_MAX_HOTWORDS = 50` and `DOUBAO_*_MAX_HOTWORDS = 100`.
- Doubao rendering intentionally keeps the legacy `{"word": ...}` shape without weight, matching `.trellis/spec/backend/asr-hotword-compilation.md`.
- Recent hotwords are generated on the frontend from successful history within 24 hours and participate in runtime dictionary refresh.
- Current-App context hotwords are generated on the backend from UIA text and appended only to runtime dictionary snapshots.
- The remaining visible gap is plan-document drift: Phase 7 is still described as future work even though most of it is landed.

## Requirements

- Verify the current implementation against the Phase 7 checklist.
- Update `ASR_PERSONALIZATION_QUALITY_LEAP.md` with a dated current-status block for Phase 7.
- Clearly note intentional deviations from the original plan, especially Doubao legacy payload shape without `weight`.
- Do not modify provider payload behavior unless audit finds a concrete regression.
- Keep unrelated dirty files out of any commit.

## Acceptance Criteria

- [x] Phase 7 status in `ASR_PERSONALIZATION_QUALITY_LEAP.md` reflects existing HotwordCompiler/provider/recent/app-context/correction-pair coverage.
- [x] The status block lists remaining non-code validation items such as formal raw-ASR quality eval and optional cache optimization.
- [x] HotwordCompiler and provider-limit tests pass.
- [x] Frontend recent-hotword tests pass.
- [x] `cargo check` passes if no code behavior changes are made.

## Definition of Done

- Audit notes are persisted under `research/`.
- Relevant tests/checks are run.
- Work commit excludes pre-existing unrelated dirty files.
- Trellis task is archived and session journal is recorded.

## Technical Approach

Use local code inspection and existing tests. If the audit confirms implementation is present and tests pass, only update the plan document. If a concrete missing behavior is found, create a focused follow-up task instead of broadening this audit task.

## Decision (ADR-lite)

Context: Phase 7 was planned as a future HotwordCompiler upgrade, but later commits already implemented most of the compiler and provider integration.

Decision: Treat this task as audit/status sync, not a reimplementation.

Consequences: The plan becomes trustworthy again, and future work can focus on measurable gaps such as raw-ASR eval or cache optimization.

## Out of Scope

- No provider payload shape change.
- No Doubao `weight` field until compatibility is verified separately.
- No new ASR eval dataset or benchmark run in this task.
- No changes to unrelated version/config files already dirty in the worktree.

## Technical Notes

- Audit details: `research/phase-7-current-state.md`.
- Relevant spec: `.trellis/spec/backend/asr-hotword-compilation.md`.
