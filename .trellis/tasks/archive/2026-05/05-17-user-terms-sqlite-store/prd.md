# Phase 5 user_terms SQLite store

## Goal

Add the first local SQLite store for Phase 5 `user_terms` so dictionary category metadata has a database landing zone. This task should prove schema creation, migration safety, and config-dictionary hydration without switching production ASR/TNL/LLM read paths away from `AppConfig.dictionary`.

## Requirements

- Add a backend `user_terms` SQLite store under the personalization layer.
- Use `rusqlite` with bundled SQLite for Windows-friendly local builds.
- Create a `user_terms` table with the roadmap fields:
  - `id`
  - `term`
  - `category`
  - `source`
  - `en_phonetic_key`
  - `zh_pinyin_fuzzy_key`
  - `created_at`
  - `updated_at`
  - `enabled`
- Create indexes for `category`, `en_phonetic_key`, `zh_pinyin_fuzzy_key`, and `term`.
- Provide a deterministic hydrator from current dictionary storage strings:
  - Parse `word`, `word|auto`, and `word|source|category`.
  - Strip metadata before storing `term`.
  - Normalize/ infer category with existing `dictionary_utils`.
  - Preserve source as `manual` or `auto`.
  - Upsert by normalized term without creating duplicates.
- Expose a default DB path under `%APPDATA%\PushToTalk\personalization\user_terms.db`.
- Keep existing runtime consumers on `AppConfig.dictionary` in this task.

## Acceptance Criteria

- [ ] Opening a new store creates the schema and indexes.
- [ ] Reopening an existing store is idempotent.
- [ ] Hydrating from legacy and category dictionary strings stores pure terms with normalized categories.
- [ ] Rehydrating the same dictionary updates existing terms instead of duplicating rows.
- [ ] Generic compact entries infer to `generic`, while non-generic terms infer expected categories.
- [ ] Targeted Rust tests pass.
- [ ] `cargo check` passes.

## Definition of Done

- TDD: add store tests before implementation, confirm they fail meaningfully, then make them pass.
- Run GitNexus impact analysis before editing symbols.
- Run `gitnexus_detect_changes` before commit.
- Update `.trellis/spec/backend/tnl-normalization.md` and `ASR_PERSONALIZATION_QUALITY_LEAP.md` if the persistence contract changes.
- Commit only this task's files; do not include unrelated version/doc config changes already present in the worktree.

## Technical Approach

- Add `src-tauri/src/personalization/user_terms_store.rs`.
- Add public exports from `src-tauri/src/personalization/mod.rs` for the store, model, and default DB path.
- Use `Connection::open(path)` plus `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`.
- Use existing `dictionary_utils::{extract_word, extract_category, normalize_or_infer_category}` for category compatibility instead of duplicating parsing rules.
- Compute phonetic index fields in a later slice; this first store writes them as `NULL` unless a later helper is added safely.
- Add `rusqlite` dependency in `src-tauri/Cargo.toml`; be careful not to stage the pre-existing version bump hunk.

## Decision (ADR-lite)

Context: Phase 5 still has no durable `user_terms` table, but the existing JSON/config dictionary is already used by production ASR/TNL/LLM paths. Replacing those paths immediately would create a broad migration and rollback risk.

Decision: Add SQLite as a sidecar store first. It can be hydrated from current dictionary strings and tested in isolation, while production runtime continues to consume the existing config dictionary.

Consequences: This does not yet deliver full SQLite-backed runtime behavior. It establishes the schema and migration surface so a later task can safely flip read/write paths or backfill from config on app startup.

## Out of Scope

- Switching ASR/TNL/LLM consumers to read `user_terms.db`.
- Migrating or deleting `AppConfig.dictionary`.
- Persisted phrase trie storage.
- Correction pair SQLite migration.
- UI changes for dictionary management.

## Research References

- [`research/sqlite-store-choice.md`](research/sqlite-store-choice.md) — local dependency and design choice notes.
