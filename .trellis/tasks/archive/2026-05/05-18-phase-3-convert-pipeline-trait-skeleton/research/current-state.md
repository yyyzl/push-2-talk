# ConvertPipeline trait skeleton current state

## Code paths inspected

* `src-tauri/src/personalization/engine.rs`
  * `PersonalizationEngine::convert_with_technical_spans` currently builds a `SyllableLattice`, calls `collect_candidates`, sorts/ranks candidates, selects non-overlapping applied candidates, mutates output text, then updates pass applied counts.
  * `collect_candidates` already has two conceptual passes:
    * `exact_text`: `store.lookup_by_text(window_text)`
    * `syllable_match`: `lookup_by_en_phonetic`, `lookup_by_zh_pinyin_fuzzy`, `lookup_by_mixed`, `lookup_by_alias_key`
  * `PassDiagnostics` already records pass name, enabled flag, elapsed time, candidate count, and applied count.
* `src-tauri/src/personalization/mod.rs`
  * Runtime helpers construct `PersonalizationEngine` and call `convert_with_technical_spans`; external API can remain unchanged.
* `.trellis/spec/backend/tnl-normalization.md`
  * Existing contracts require pass toggles to preserve fallback behavior.
  * Existing contracts require future ConvertPipeline passes to consume `SyllableLattice::windows()` instead of duplicating token-window loops.
  * NamedEntity score boost must remain bounded and local to syllable-match candidates.

## Recommended slice

Create a private skeleton in `engine.rs` first:

* `ConvertContext<'a>` carries `lattice`, `store`, `config`, and `technical_spans`.
* `ConvertPass` exposes `name`, `enabled`, and `collect_candidates`.
* `ExactTextPass` implements exact lookup only.
* `SyllableMatchPass` implements phonetic / fuzzy / mixed / alias lookup only.
* `ConvertPipeline::default()` owns the stable pass order and returns `(Vec<ConversionCandidate>, Vec<PassDiagnostics>)`.

## Risk notes

* Candidate collection order changes can accidentally change dedupe winners for the same pair/range. Keep `ExactTextPass` before `SyllableMatchPass` so exact candidates remain preferred when both exist.
* Pass summaries are consumed by eval diagnostics, so names and enabled/disabled semantics must not change.
* Do not move selection/overlap logic into passes in this slice; keeping it in `PersonalizationEngine::convert_with_technical_spans` preserves current behavior.
