# ConvertPipeline run stage current state

## Current code shape

`src-tauri/src/personalization/engine.rs` currently has three layers:

1. `PersonalizationEngine::convert_with_technical_spans`
   * Handles empty text early return.
   * Builds `SyllableLattice`.
   * Calls `self.collect_candidates`.
   * Sorts candidates by span length, rank score, score, and start.
   * Applies threshold and non-overlap selection.
   * Replaces selected spans in reverse byte order.
   * Updates `PassDiagnostics.applied_count`.
2. `PersonalizationEngine::collect_candidates`
   * Builds windows once.
   * Creates `ConvertContext`.
   * Delegates candidate collection to `ConvertPipeline`.
3. `ConvertPipeline`
   * Owns pass order and pass summary collection.
   * Does not yet own global selection/replacement.

## Recommended next slice

Move only global conversion orchestration into `ConvertPipeline::run`:

* Keep each `ConvertPass` collect-only.
* Keep all scoring helpers unchanged.
* Keep empty-input early return in `PersonalizationEngine`.
* Keep `PersonalizationEngine::convert` / `convert_with_technical_spans` as compatibility APIs.
* Reuse existing tests plus one direct `ConvertPipeline::run` unit test.

## Risk notes

* `replace_range` must keep reverse start-order replacement to preserve byte offsets.
* `update_pass_applied_counts` must run after `selected` is sorted back to ascending order, matching existing diagnostics.
* `rank_score` must remain sorting-only; it must not bypass `apply_threshold`.
* The moved code should remain outside individual passes so future passes cannot apply replacements before global overlap resolution.
