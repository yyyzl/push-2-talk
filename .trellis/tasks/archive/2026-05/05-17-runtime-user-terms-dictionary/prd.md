# Load Runtime Dictionary From User Terms Sidecar

## Goal

Introduce a safe read path from the `user_terms.db` sidecar into backend-controlled runtime dictionary restarts, while preserving the existing frontend runtime dictionary merge that includes user terms, builtin domain terms, recent hotwords, and app-context hotwords.

## What I Already Know

- `user_terms.db` is now hydrated from `AppConfig.dictionary` during persisted config load/save.
- `UserTermStore` supports enabled-only phonetic key queries but does not yet expose enabled terms as dictionary storage/runtime entries.
- The frontend `useAppServiceController` builds a runtime dictionary by merging user entries, builtin domain entries, and recent hotword entries before calling `start_app`.
- `start_app` should not be blindly switched to read only `user_terms.db`, because that would drop builtin/recent runtime entries supplied by the frontend.
- Backend-controlled `restart_service_with_config` currently converts `config.dictionary` to pure words via `entries_to_words`, losing category metadata for TNL when restarting from tray/provider flows.

## Requirements

- Add a `UserTermStore` API to list enabled terms as dictionary entries preserving source/category metadata.
- Disabled sidecar rows must be excluded from this API.
- Entry formatting must reuse `dictionary_utils::format_entry_with_category`.
- Add a warning-only helper that loads enabled runtime entries from the default sidecar and falls back to normalized `AppConfig.dictionary` if the sidecar read fails or is empty.
- Use that helper in backend-controlled `restart_service_with_config`.
- Do not change `start_app`'s `dictionary` parameter behavior; frontend runtime merge remains authoritative there.
- Do not change ASR/TNL hot paths beyond the backend restart dictionary source.

## Acceptance Criteria

- [ ] Enabled user terms can be exported from SQLite as dictionary entries with metadata.
- [ ] Disabled rows are not exported.
- [ ] `restart_service_with_config` uses sidecar entries when available and falls back to config entries on sidecar failure/empty result.
- [ ] Existing frontend runtime dictionary merge remains untouched.
- [ ] Targeted Rust tests pass.
- [ ] `cargo check` passes.

## Definition Of Done

- Tests added for store export and fallback helper behavior.
- Relevant docs/roadmap updated if behavior changes.
- GitNexus impact checked before editing shared symbols and detect_changes run before commit.
- Commit only this task's files.

## Technical Approach

Add `UserTermStore::list_enabled_dictionary_entries()` in `src-tauri/src/personalization/user_terms_store.rs`, backed by a deterministic SQL query over `enabled = 1`. Map each row with `format_entry_with_category(term, source, Some(category))`.

In `src-tauri/src/lib.rs`, add a path-injectable helper for tests, then call the default-path helper from `restart_service_with_config`. Keep failures warning-only and fallback to the normalized config dictionary.

## Decision (ADR-lite)

**Context**: The sidecar is ready to read, but the frontend runtime dictionary includes dynamic sources not present in `user_terms.db`.

**Decision**: Start sidecar reads in backend-controlled restart flows only, preserving frontend-owned runtime dictionary merging for `start_app`.

**Consequences**: TNL restarts can preserve user term metadata from SQLite, but builtin/recent runtime terms continue to flow through the existing frontend merge. Full runtime migration remains a later task that must explicitly merge DB user terms with dynamic runtime terms.

## Out Of Scope

- Replacing `start_app` dictionary input.
- Moving builtin domain terms or recent hotwords into SQLite.
- Persisting phrase trie/index structures.
- Changing frontend runtime dictionary merge behavior.

## Technical Notes

- Relevant files: `src-tauri/src/personalization/user_terms_store.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/dictionary_utils.rs`.
- Relevant specs: `.trellis/spec/backend/index.md`, `.trellis/spec/backend/database-guidelines.md`.
