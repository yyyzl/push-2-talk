# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

- The first project-local database surface is the personalization `user_terms` SQLite sidecar.
- Use `rusqlite` for thin, explicit SQL wrappers.
- The Windows app must not require a separately installed SQLite runtime; use the bundled SQLite feature when adding the dependency.
- SQLite sidecars live under `%APPDATA%\PushToTalk\personalization\`.
- A new database sidecar must be introduced behind existing JSON/config runtime paths first, then production read/write paths can be switched in a later task after tests prove migration safety.

---

## Query Patterns

- Keep store APIs narrow and domain-specific. Do not expose raw `Connection` handles outside the store module.
- Use parameterized statements with `rusqlite::params!`; never format user text into SQL strings.
- Batch hydration and migration writes should run inside a transaction.
- Existing dictionary storage strings must be parsed through `dictionary_utils` helpers before entering SQLite so metadata compatibility stays centralized.

---

## Migrations

- Use idempotent `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for first-slice sidecar schemas.
- Reopening an existing database must be safe and must not drop or rewrite existing rows.
- Keep legacy `AppConfig.dictionary` intact until a dedicated migration task switches production consumers.
- For `user_terms`, hydrate from current config strings as a repeatable operation and upsert by case-insensitive term.

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
