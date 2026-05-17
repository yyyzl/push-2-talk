# Sample intake current state

## Existing eval schema

`src-tauri/src/bin/eval_asr.rs` expects each eval case to include:

* `audio_id`
* `audio_wav_path`
* `provider`
* `raw_asr_text`
* `expected_text`
* `user_final_text`
* `category`
* `notes`

The current suite has 26 fixed text cases under `tests/asr_eval/cases/`.

## History source

Frontend `HistoryRecord` includes `originalText`, optional `polishedText`, `mode`, `success`, and optional TNL diagnostics. It is stored in browser localStorage through `src/utils/history.ts`, so a repo-local importer should accept an exported JSON file rather than try to read WebView storage directly.

Useful draft mapping:

* `raw_asr_text` = `originalText`
* `expected_text` = `polishedText ?? originalText`
* `user_final_text` = same as `expected_text`
* `provider` = `history-draft`
* `category` = `real_history_draft`

Filter to successful normal-mode records by default so assistant/polishing workflows do not pollute ASR eval samples.

## Runtime diagnostics source

Runtime personalization diagnostics contain bounded `source_text`, `output_text`, `changed`, `candidate_count`, `applied_count`, `candidates`, and `applied`. They are useful for identifying local second-decoding activity, but not proof of the user's intended final text.

Useful draft mapping:

* `raw_asr_text` = `source_text`
* `expected_text` = `output_text`
* `user_final_text` = `output_text`
* `provider` = `runtime-diagnostic-draft`
* `category` = `runtime_personalization_draft`

Filter to changed/applied records by default. Notes must say the sample needs manual confirmation before promotion to formal cases.

## Recommended slice

Implement a TypeScript module plus CLI under `scripts/` because the project already uses `tsx` for test/runtime scripts and this avoids changing the Rust eval runner. Keep the output as draft files under `tests/asr_eval/drafts/`; formal promotion can be a later task after the user reviews real content.
