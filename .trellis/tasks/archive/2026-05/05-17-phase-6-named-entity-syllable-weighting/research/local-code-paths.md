# Local Code Path Research

## Question

How should Phase 6 named-entity spans reach the local `SyllableMatchPass` behavior?

## Findings

- `TnlEngine::normalize` returns `NormalizationResult { text, diagnostics, technical_spans, ... }`.
- `TechSpanDetector` now emits `SpanType::NamedEntity` for user terms and conservative jieba POS tags.
- `NormalPipeline::process` currently discards `tnl_result.technical_spans` and calls `apply_default_personalization_with_config(text, config)`.
- Assistant flow in `src-tauri/src/lib.rs` also discards `tnl_result.technical_spans` before calling `apply_assistant_personalization`.
- `PersonalizationEngine::convert` builds a `SyllableLattice` and calls `collect_candidates`; it has no span context today.
- Candidate `score` controls the apply threshold. `rank_score` only orders candidates and must not bypass the threshold.
- `auto_score` already blocks learned risky single-word pairs by returning `0.0`; the named-entity bonus should be applied after that guard so blocked pairs stay blocked.

## Recommended Implementation

- Add a span-aware compatibility layer rather than replacing existing APIs:
  - `PersonalizationEngine::convert(text)` delegates to `convert_with_technical_spans(text, &[])`.
  - Runtime helper APIs add span-aware variants used by normal and assistant pipelines.
- Use byte-range overlap with `SpanType::NamedEntity` to derive a small score context for syllable-match candidates.
- Keep exact-text candidates unchanged because exact matching is the fallback path, not the syllable/alias risk area.
- Add tests around threshold crossing, non-overlap, and blocked risky learned pairs.
