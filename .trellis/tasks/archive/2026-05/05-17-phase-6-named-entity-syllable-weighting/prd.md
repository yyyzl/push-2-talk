# Phase 6 Named Entity Syllable Weighting

## Goal

Complete the remaining Phase 6 loop by letting `NamedEntity` spans from TNL lightly influence the local IME-style personalization decoder. When a syllable/window candidate overlaps a conservative named-entity span, the candidate should receive a bounded score/ranking boost so known names, locations, products, and aliases are less likely to be left for cloud arbitration.

## What I Already Know

- Phase 6 already added `jieba-rs` user-term injection and POS tags `nr/ns/nt/nz` as low-priority `SpanType::NamedEntity` spans in `TechSpanDetector`.
- `PersonalizationEngine` currently builds `SyllableLattice` directly from text and does not consume `NormalizationResult.technical_spans`.
- Normal and assistant pipelines run TNL before personalization, but they currently pass only normalized text into personalization.
- Existing personalization scoring keeps `score` as the apply-threshold gate and `rank_score` for ordering; context/frequency rank boosts must not bypass the apply threshold.
- Existing safety rules must continue: risky single-word learned pairs stay non-auto-applicable, mixed windows do not use English-only keys, and strong technical spans keep precedence in TNL.

## Requirements

- Add a personalization entry point that accepts TNL technical spans alongside text while keeping existing no-span APIs backward compatible.
- Pass `NormalizationResult.technical_spans` from normal dictation and assistant paths into personalization when TNL is enabled.
- Apply a small, bounded boost only to syllable-match candidates whose candidate byte range overlaps at least one `SpanType::NamedEntity`.
- Do not boost `ExactText` candidates; exact matching already has its own high-confidence path.
- Do not let the named-entity boost revive pairs that `requires_manual_for_auto_apply()` blocks.
- Preserve existing pass toggles, window language routing, overlap selection, and diagnostics behavior.

## Acceptance Criteria

- [x] A learned alias/phonetic candidate below the default apply threshold remains below threshold without a named-entity span.
- [x] The same candidate can cross the threshold when its span overlaps a `NamedEntity` technical span.
- [x] A `NamedEntity` span outside the candidate range does not boost that candidate.
- [x] Risky single-word learned pairs remain blocked even with a `NamedEntity` span.
- [x] Normal and assistant pipelines pass TNL technical spans into personalization.
- [x] Relevant Rust tests and `cargo check` pass.

## Definition of Done

- Tests added or updated for the new scoring behavior and pipeline plumbing.
- `cargo fmt --check`, targeted Rust tests, and `cargo check` pass.
- `.trellis/spec/backend/tnl-normalization.md` and the ASR personalization plan are updated if behavior changes.
- GitNexus impact analysis is run before editing affected symbols and `detect_changes` is run before commit.

## Technical Approach

Use a conservative scoring helper inside `PersonalizationEngine`:

- `convert(text)` remains the compatibility API and delegates to a span-aware method with an empty span list.
- `convert_with_technical_spans(text, spans)` feeds named-entity overlap context into `collect_candidates`.
- Candidate scoring keeps the existing base multipliers, then applies a bounded named-entity bonus only after `auto_score` confirms the pair is eligible for auto apply.
- Pipelines preserve old behavior when TNL is disabled or no spans are available.

## Decision (ADR-lite)

Context: Phase 6 needs the NER signal to improve local second-decoding quality without adding a model or widening the cloud path.

Decision: Use TNL `NamedEntity` spans as a bounded local score signal for syllable-match candidates, not as a new global threshold or separate replacement pass.

Consequences: This can lift borderline named-entity alias/phonetic candidates while keeping common-word and non-overlapping false positives unchanged. The boost size remains a tunable constant with tests guarding the boundary.

## Out of Scope

- No ONNX/ML NER model.
- No frontend settings or UI changes.
- No global `personalization_apply_threshold` default change.
- No new ASR provider or cloud request changes.

## Technical Notes

- Likely files: `src-tauri/src/personalization/engine.rs`, `src-tauri/src/personalization/mod.rs`, `src-tauri/src/pipeline/normal.rs`, `src-tauri/src/lib.rs`.
- Relevant specs: `.trellis/spec/backend/tnl-normalization.md`, `.trellis/spec/backend/asr-hotword-compilation.md`, `.trellis/spec/backend/database-guidelines.md`.
- Code inspection notes are recorded in `research/local-code-paths.md`.
