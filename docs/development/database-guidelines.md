# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

- The first project-local database surface is the personalization `user_terms` SQLite sidecar.
- Use `rusqlite` for thin, explicit SQL wrappers.
- Neither supported platform may require a separately installed SQLite runtime; use the bundled SQLite feature.
- SQLite sidecars live under the platform config directory: `%APPDATA%\PushToTalk\personalization\` on Windows and `~/Library/Application Support/PushToTalk/personalization/` on macOS.
- A new database sidecar must be introduced behind existing JSON/config runtime paths first, then production read/write paths can be switched in a later task after tests prove migration safety.

---

## Query Patterns

- Keep store APIs narrow and domain-specific. Do not expose raw `Connection` handles outside the store module.
- Use parameterized statements with `rusqlite::params!`; never format user text into SQL strings.
- Batch hydration and migration writes should run inside a transaction.
- Existing dictionary storage strings must be parsed through `dictionary_utils` helpers before entering SQLite so metadata compatibility stays centralized.
- `user_terms` phonetic index columns must be hydrated through `personalization::phonetic_keys::build_key_bundle`; do not add a second phonetic-key implementation in the database layer.
- Runtime dictionary exports from `user_terms` must query enabled rows only and format entries through `dictionary_utils::format_entry_with_category` so source/category metadata stays compatible with TNL routing.
- When merging `user_terms` with runtime dictionary input, preserve runtime-only sources such as `domain`, `recent`, `builtin`, and `app_context`; do not run persisted-config normalization over those entries because it canonicalizes non-`auto` sources to `manual`.
- Dictionary management commands (`get_dictionary_entries`, `add_learned_word`, `delete_dictionary_entries`) must be sidecar-first after Phase 5 migration: read/upsert/disable enabled `user_terms` rows, then mirror enabled entries back to `AppConfig.dictionary` only as a compatibility snapshot.
- Ordinary config saves and loads must not overwrite initialized `user_terms` from `AppConfig.dictionary`. Only explicit dictionary payloads or the first bootstrap may hydrate the sidecar. Config loads refresh their in-memory dictionary snapshot from SQLite.

---

## Migrations

- Use idempotent `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for first-slice sidecar schemas.
- Reopening an existing database must be safe and must not drop or rewrite existing rows.
- `AppConfig.dictionary` is now a compatibility snapshot; `user_terms` is the authoritative user dictionary after bootstrap.
- Backend runtime dictionary reads fall back to normalized input when SQLite cannot be read or has never contained entries. An initialized dictionary with zero enabled rows is deliberately empty: discard stale manual/auto input entries while retaining runtime-only sources (`domain`, `recent`, `builtin`, `app_context`).
- Explicit dictionary import uses a transactional snapshot operation: upsert present terms by case-insensitive term and disable enabled rows absent from that import. Do not run it on ordinary config load.
- `get_dictionary_entries` may bootstrap from `AppConfig.dictionary` only when the database has no rows at all. Disabled rows count as initialized, so deleting the last entry must not resurrect it on the next load. `add_learned_word` and `delete_dictionary_entries` return an error if the sidecar cannot be opened.
- `save_config` may sync `AppConfig.dictionary` into `user_terms` only when the frontend sends an explicit `dictionary` field, for import/migration compatibility. Field patches, tray switches, ASR fallback repair, and ordinary settings saves must leave the sidecar untouched.
- `user_terms.en_phonetic_key` stores the first English phonetic key from `build_key_bundle`; `user_terms.zh_pinyin_fuzzy_key` stores the bundle's fuzzy pinyin key. Leave the column `NULL` when the key does not apply.
- `user_terms` key lookup APIs must query enabled rows only, return empty results for empty keys, and order manual terms before automatic terms for deterministic candidate selection.

## Correction Pair Files

- Serialize each in-process read-modify-write operation, including observation and LLM feedback, to avoid losing concurrent updates.
- Write a unique sibling temporary file, flush it, close it, then publish via `platform::replace_file`. Do not move the old destination aside before publication or share a fixed temporary filename between writers.
- A failed replacement must preserve the existing destination, including when it is unexpectedly a directory. A backup cleanup failure after successful publication is warning-only.

---

## Naming Conventions

- Tables use plural snake_case names, such as `user_terms`.
- Indexes use `idx_<table>_<column>`, such as `idx_user_term_category`.
- Timestamp fields store Unix milliseconds in integer columns and use `_at` suffixes.
- Boolean fields are stored as integer `0` / `1` in SQLite and converted at the store boundary.

---

## Common Mistakes

- Do not switch ASR/TNL/LLM production consumers to a new database in the same task that introduces the schema.
- Do not duplicate dictionary category inference in SQL store code; reuse `dictionary_utils`.
- Do not store dictionary metadata strings such as `Claude Code|manual|product` in `user_terms.term`; store only the pure term.
