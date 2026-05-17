# Merge Sidecar User Terms Into Start App Runtime Dictionary

## Goal

Let `start_app` safely use enabled user terms from `user_terms.db` while preserving the dynamic runtime dictionary entries that the frontend already merges in, such as builtin domain entries and recent hotwords.

## What I Already Know

- `user_terms.db` is hydrated from `AppConfig.dictionary` on config load/save.
- Backend-controlled `restart_service_with_config` already prefers enabled `user_terms` entries and falls back to normalized config entries.
- The normal frontend startup path calls `start_app` with a runtime dictionary that is richer than persisted user terms:
  - user dictionary entries
  - builtin domain entries using source `domain`
  - recent hotword entries using source `recent`
- Replacing `start_app`'s input dictionary with DB rows would drop domain/recent entries and regress ASR hotwords/TNL context.
- The safe next step is to merge DB user terms into the `start_app` runtime dictionary, preferring DB metadata for duplicate user words while keeping dynamic entries.

## Requirements

- Add a helper that combines:
  - enabled `user_terms` entries from the sidecar, when readable and non-empty
  - the `dictionary` runtime entries passed into `start_app`
- Preserve non-user dynamic entries from `start_app` input, especially `domain`, `recent`, `builtin`, and `app_context` sources.
- De-duplicate by pure word case-insensitively.
- Prefer sidecar entries for duplicate words so user source/category metadata comes from SQLite.
- If the sidecar cannot be opened or has no enabled rows, use the normalized `start_app` input dictionary unchanged apart from existing category backfill.
- Keep `restart_service_with_config` behavior compatible; it can continue using the same helper with config entries.
- Do not change frontend `useAppServiceController` merge logic in this task.

## Acceptance Criteria

- [ ] `start_app` runtime dictionary can merge sidecar user entries with frontend dynamic entries.
- [ ] Duplicate dynamic/user words prefer sidecar user metadata.
- [ ] `domain` and `recent` entries absent from `user_terms.db` are retained.
- [ ] Sidecar read failure or empty DB falls back to normalized input dictionary.
- [ ] Tests cover merge, duplicate preference, dynamic preservation, and fallback.
- [ ] Relevant Rust tests and `cargo check` pass.

## Definition Of Done

- Minimal backend-only change.
- Tests added or updated around the new helper.
- Roadmap/spec updated if behavior changes.
- Work committed, task archived, and journal recorded.

## Technical Approach

Extend the existing runtime dictionary helper in `src-tauri/src/lib.rs` so it accepts generic runtime entries, not only persisted config entries. When sidecar entries exist, build a merged vector with sidecar entries first, then append normalized runtime input entries whose pure word is not already present.

Use `dictionary_utils::extract_word` for de-duplication. Use the existing normalization helper for fallback and for normalizing the frontend-provided runtime input before merging.

## Decision (ADR-lite)

**Context**: `user_terms.db` can now provide user terms, but frontend runtime dictionaries contain dynamic sources that should not be persisted into the sidecar.

**Decision**: Merge sidecar user terms into `start_app` runtime dictionaries instead of replacing the entire runtime dictionary.

**Consequences**: User term metadata starts coming from SQLite on both backend restarts and frontend-driven starts, while builtin/recent/app-context runtime signals remain outside the database. Full frontend migration can come later without a hotword regression.

## Out Of Scope

- Removing frontend runtime dictionary merge.
- Persisting builtin/recent/app-context entries into SQLite.
- Persisting phrase trie/index structures.
- Changing ASR provider payload formats.

## Technical Notes

- Relevant files: `src-tauri/src/lib.rs`, `src-tauri/src/dictionary_utils.rs`, `src-tauri/src/personalization/user_terms_store.rs`.
- Relevant specs: `.trellis/spec/backend/database-guidelines.md`, `.trellis/spec/backend/asr-hotword-compilation.md`, `.trellis/spec/backend/tnl-normalization.md`.
