# Draft promotion current state

## Existing intake

`scripts/asrEvalDraftCore.ts` currently emits draft cases from:

* exported frontend history JSON,
* runtime personalization diagnostics JSON files or directories.

Draft cases already contain the formal eval fields plus `notes` explaining manual confirmation is required. They do not yet have a machine-readable review marker.

## Promotion shape

Formal eval cases should keep:

* `audio_id`
* `audio_wav_path`
* `provider`
* `raw_asr_text`
* `expected_text`
* `user_final_text`
* `category`
* `notes`
* `diagnostics`

Promotion should drop draft-only fields such as `review_status` and `review_notes`.

## Safe approval marker

Use `review_status: "approved"` as the only promotion marker. Generated drafts should default to `review_status: "needs_review"`. This avoids treating ordinary notes text as approval and keeps manual review auditable in the draft file.

## CLI recommendation

Extend the existing `scripts/asr-eval-draft.ts` rather than adding a second script:

```powershell
npx tsx scripts/asr-eval-draft.ts --promote tests/asr_eval/drafts/reviewed.json --out tests/asr_eval/cases/phase0b-real.json
```

The script should reject mixing `--promote` with `--history` or `--diagnostics`, and fail if no approved cases are found.
