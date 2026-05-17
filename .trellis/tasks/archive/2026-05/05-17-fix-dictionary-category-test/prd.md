# Fix Stale Dictionary Category Metadata Test

## Goal

Restore the TypeScript runtime regression suite by updating a stale dictionary category metadata assertion to match the current backend helper used by `add_learned_word`.

## What I Already Know

- `npm run test:ts` currently reports 119/120 passing.
- The failing test is `tests/dictionaryCategoryMetadata.test.ts`.
- The stale assertion expects `src-tauri/src/lib.rs` to contain `upsert_entry_with_category`.
- Current backend behavior intentionally calls `upsert_entry_with_inferred_category`, which preserves valid existing metadata and infers missing or invalid categories before delegating to the lower-level category upsert helper.
- This is a test expectation drift, not a product behavior change.

## Requirements

- Update the stale test assertion to verify the current backend category save path.
- Keep the existing intent of the test: front-end category edits must flow through `add_learned_word` with `category.as_deref()`.
- Do not change production runtime behavior.
- Do not modify unrelated dirty files.

## Acceptance Criteria

- [ ] `npm run test:ts` passes.
- [ ] The changed test still proves backend category persistence uses the category-aware path.
- [ ] No Rust production code changes are needed.

## Definition Of Done

- Minimal test-only patch.
- Relevant test command passes.
- Task committed, archived, and journaled.

## Out Of Scope

- Refactoring dictionary category helpers.
- Changing frontend dictionary UI.
- Changing backend learning or ASR behavior.
