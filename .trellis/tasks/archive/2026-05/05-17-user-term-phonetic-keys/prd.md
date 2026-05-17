# Phase 5 hydrate user term phonetic keys

## Goal

Populate `user_terms.en_phonetic_key` and `user_terms.zh_pinyin_fuzzy_key` during dictionary hydration so the new SQLite sidecar is closer to the Phase 5 target schema and can support later indexed lookup work.

## Requirements

- Reuse `personalization::phonetic_keys::build_key_bundle` for key generation.
- Store the first English phonetic key in `en_phonetic_key`.
- Store the fuzzy Chinese pinyin key in `zh_pinyin_fuzzy_key`.
- Leave either column `NULL` when the term has no corresponding key.
- Preserve existing dictionary hydration behavior for term/category/source/upsert/manual-source priority.
- Keep production runtime read paths unchanged.

## Acceptance Criteria

- [ ] Hydrating ASCII terms such as `Claude Code` stores a non-empty `en_phonetic_key`.
- [ ] Hydrating CJK terms such as `深度求索` stores a non-empty `zh_pinyin_fuzzy_key`.
- [ ] Hydrating generic ASCII terms without a useful phonetic key can keep key columns `NULL`.
- [ ] Rehydrating an existing term refreshes key columns without creating duplicates.
- [ ] Targeted `user_terms_store` tests pass.
- [ ] `cargo check` passes.

## Definition of Done

- Add/update tests before implementation and confirm the failing behavior.
- Run GitNexus impact analysis before modifying existing symbols.
- Run `gitnexus_detect_changes` before commit.
- Update roadmap/spec notes if key hydration contract changes.
- Commit only this task's files; do not include unrelated dirty files.

## Technical Approach

- Import `build_key_bundle` into `user_terms_store.rs`.
- Add a helper that derives `(Option<String>, Option<String>)` from a pure term.
- Use those derived values in the hydration upsert `INSERT` and `ON CONFLICT DO UPDATE`.
- Extend the existing hydration tests to assert key presence and refresh behavior.

## Out of Scope

- Mixed key or alias key columns.
- Query APIs that search by phonetic keys.
- Switching ASR/TNL/LLM production paths to SQLite.
- Persisted phrase trie storage.
