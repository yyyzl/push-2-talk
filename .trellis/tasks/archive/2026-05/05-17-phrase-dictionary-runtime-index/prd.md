# Phase 5 phrase dictionary runtime index

## Goal

Add a small runtime index for `category=phrase` dictionary rules inside TNL so the phrase prepass no longer scans every phrase rule at every text offset. This keeps the current personalization behavior while preparing the code path for later persisted phrase/user-term indexes.

## Requirements

- Build an in-memory phrase rule index when `TnlEngine` is constructed.
- Route only `phrase` category entries into the phrase prepass; all other category routing behavior stays unchanged.
- Preserve existing phrase behavior:
  - ASCII whitespace-separated phrases match case-insensitively.
  - CJK phrase entries may collapse whitespace inserted between characters.
  - Phrase matching must not cross punctuation such as comma or Chinese comma.
  - Longest match at the same start position wins.
- Avoid database/schema changes in this task.
- Avoid changing frontend, ASR provider payloads, or persisted dictionary storage.

## Acceptance Criteria

- [ ] Existing phrase prepass tests still pass.
- [ ] A longer phrase sharing the same first segment wins over a shorter phrase.
- [ ] Indexed lookup handles CJK phrase entries with shared leading characters.
- [ ] Non-phrase dictionary categories still skip phrase prepass.
- [ ] `cargo fmt` and targeted Rust tests pass.
- [ ] `cargo check` passes for the backend.

## Definition of Done

- Tests are added or updated before implementation and then made green.
- TNL spec/roadmap notes are updated if the runtime contract changes or becomes more precise.
- GitNexus impact analysis is run before editing TNL symbols, and `gitnexus_detect_changes` is run before commit.
- Only files related to this task are staged and committed.

## Technical Approach

Introduce a private `PhraseDictionaryIndex` near `PhraseDictionaryRule` in `src-tauri/src/tnl/engine.rs`. The index will own the existing phrase rules plus first-segment buckets:

- `Segmented` rules are bucketed by their lowercase first phrase segment.
- `CjkCharacters` rules are bucketed by their first CJK character.
- `apply_phrase_dictionary_rewrite` iterates text starts as before, but asks the index for candidate rule indexes that can plausibly match the current start instead of scanning all rules.
- Exact matching remains delegated to `try_match_phrase_rule`, so boundary, punctuation, whitespace, and replacement behavior stay centralized.

## Decision (ADR-lite)

Context: The roadmap mentions a future persisted phrase trie/index, but the current code still does a linear scan over every phrase rule for every input start. Jumping directly to SQLite or a persisted trie would mix storage migration with runtime matching behavior.

Decision: Implement a private runtime first-segment index now. Keep the public TNL behavior and storage format unchanged.

Consequences: This is a conservative performance/structure step, not the final Phase 5 storage solution. Future SQLite `user_terms` and persisted phrase indexes can replace or hydrate the same runtime structure later.

## Out of Scope

- SQLite `user_terms` table or migrations.
- Persisted phrase trie/index files.
- `en_phonetic_key` / `zh_pinyin_fuzzy_key` index columns.
- Frontend dictionary UX changes.
- ASR provider hotword payload changes.

## Technical Notes

- Main file: `src-tauri/src/tnl/engine.rs`.
- Relevant spec: `.trellis/spec/backend/tnl-normalization.md`.
- Roadmap section: `ASR_PERSONALIZATION_QUALITY_LEAP.md`, Phase 5 dictionary category and phrase index notes.
- Existing tests already cover ASCII phrase casing, CJK inserted whitespace, punctuation boundaries, and non-phrase skip behavior.
