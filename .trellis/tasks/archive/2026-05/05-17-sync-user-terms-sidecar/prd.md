# Sync Dictionary To User Terms Sidecar

## Goal

Make the new SQLite `user_terms` sidecar participate in the real production lifecycle by hydrating it from the current `AppConfig.dictionary` whenever persisted configuration is loaded or saved, without switching ASR/TNL runtime consumers away from the existing config dictionary yet.

## What I Already Know

- Phase 5 has introduced `UserTermStore` with schema, indexes, phonetic key hydration, and key lookup APIs.
- Current runtime paths still use `AppConfig.dictionary` and `learning::store::entries_to_words(...)` for ASR/TNL hotword behavior.
- `AppConfig::load()` already backfills dictionary categories and returns a `migrated` flag.
- `save_config`, `patch_config_fields`, learning add/delete flows, tray toggles, and startup load paths converge through `load_persisted_config()` / `save_persisted_config_without_emit()` in `src-tauri/src/lib.rs`.
- The database guideline says new sidecars must be introduced behind existing JSON/config paths first, then production consumers can be switched later.

## Requirements

- Add a narrow production sync helper that opens the default `user_terms.db` sidecar and hydrates it from normalized `AppConfig.dictionary`.
- Invoke that helper after persisted config loads and after persisted config saves, so startup, UI saves, learning additions, and dictionary deletions keep the sidecar fresh.
- Do not fail config load/save if sidecar sync fails; log a warning and keep the existing config-based runtime path working.
- Preserve the existing `AppConfig.dictionary` storage and ASR/TNL runtime dictionary behavior.
- Keep the sidecar hydration idempotent and reuse `UserTermStore::hydrate_dictionary_entries`.

## Acceptance Criteria

- [ ] Loading config attempts to hydrate `user_terms.db` from the loaded dictionary.
- [ ] Saving config attempts to hydrate `user_terms.db` from the saved dictionary.
- [ ] Sidecar sync errors are warning-only and do not block config load/save.
- [ ] Tests cover successful sync and sync failure handling through injectable paths or helpers.
- [ ] Existing `user_terms_store` tests still pass.
- [ ] `cargo check` passes.

## Definition Of Done

- Tests added or updated for the new lifecycle hook.
- Relevant Rust formatting and compilation checks pass.
- `ASR_PERSONALIZATION_QUALITY_LEAP.md` and/or backend database spec is updated if behavior changes.
- Commit only this task's files; leave unrelated dirty files untouched.

## Technical Approach

Introduce a small wrapper near persisted config helpers in `lib.rs`, for example `sync_user_terms_sidecar_from_dictionary(...)`, with an internal path-injectable variant for tests. The helper should call `UserTermStore::open(path)` then `hydrate_dictionary_entries(dictionary)`, returning `Result<usize, String>` or `anyhow::Result<usize>` internally. Production call sites catch errors and log warnings.

This is intentionally a sidecar hydration step, not a runtime read migration. A later task can switch TNL phrase/phonetic lookup to `user_terms.db` after this database is being continuously populated in real user sessions.

## Decision (ADR-lite)

**Context**: The SQLite store exists but is not yet populated by real app flows.

**Decision**: Hydrate the sidecar opportunistically at config load/save boundaries, warning-only on failure.

**Consequences**: The database becomes ready for future runtime migration while config remains the source of truth. There is a small extra disk/SQLite operation on config lifecycle paths, but hot ASR recognition paths remain unchanged.

## Out Of Scope

- Switching TNL/ASR runtime dictionary lookup to SQLite.
- Persisting phrase trie/index structures.
- Adding UI for directly reading or editing `user_terms.db`.
- Deleting or replacing `AppConfig.dictionary`.

## Technical Notes

- Relevant code: `src-tauri/src/personalization/user_terms_store.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/config.rs`.
- Relevant specs: `.trellis/spec/backend/index.md`, `.trellis/spec/backend/database-guidelines.md`.
- Relevant roadmap: `ASR_PERSONALIZATION_QUALITY_LEAP.md` Phase 5 current status.
