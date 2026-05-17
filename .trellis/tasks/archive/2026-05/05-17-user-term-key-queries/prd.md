# Phase 5 user term key queries

## Goal

Add query APIs on the `user_terms` SQLite sidecar for `en_phonetic_key` and `zh_pinyin_fuzzy_key`, so later runtime migration can retrieve candidate dictionary terms by the indexed keys already hydrated in the previous slice.

## Requirements

- Add `UserTermStore` methods to query enabled rows by exact `en_phonetic_key`.
- Add `UserTermStore` methods to query enabled rows by exact `zh_pinyin_fuzzy_key`.
- Empty or whitespace-only key input must return an empty list.
- Disabled rows must not be returned by key lookup.
- Results should be deterministic:
  - manual source before auto source;
  - then category/term order for stable tests and diagnostics.
- Existing hydration, reopen, and list behavior must remain unchanged.
- Keep production ASR/TNL/LLM read paths unchanged in this task.

## Acceptance Criteria

- [ ] English phonetic key lookup returns matching enabled terms.
- [ ] Chinese fuzzy pinyin key lookup returns matching enabled terms.
- [ ] Empty key lookup returns no rows.
- [ ] Disabled rows are filtered out.
- [ ] Query result order is deterministic and prioritizes manual terms.
- [ ] Targeted `user_terms_store` tests pass.
- [ ] `cargo check` passes.

## Definition of Done

- Add tests first and confirm the API is missing or behavior fails.
- Run GitNexus impact analysis before editing the existing store symbol.
- Run `gitnexus_detect_changes` before commit.
- Update database guidelines / roadmap if the query contract needs to be remembered.
- Commit only this task's files; leave unrelated dirty files alone.

## Technical Approach

- Add `find_by_en_phonetic_key(&self, key: &str) -> Result<Vec<UserTerm>>`.
- Add `find_by_zh_pinyin_fuzzy_key(&self, key: &str) -> Result<Vec<UserTerm>>`.
- Share query mapping with existing `row_to_user_term`.
- Use parameterized SQL and `enabled = 1`.
- Add a small test-only helper to disable a term if needed for filtering coverage.

## Out of Scope

- Runtime pipeline migration to `user_terms.db`.
- Search by mixed or alias keys.
- Persisted phrase trie storage.
- UI changes.
