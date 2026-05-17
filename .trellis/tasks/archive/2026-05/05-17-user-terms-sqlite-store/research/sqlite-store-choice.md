# SQLite store choice

## Local findings

- The project currently stores user dictionary entries in `AppConfig.dictionary: Vec<String>`.
- `ASR_PERSONALIZATION_QUALITY_LEAP.md` explicitly says Phase 5 should not assume `user_terms` already exists and should wait until SQLite lands before migrating dictionary metadata.
- No SQLite crate is currently wired into `src-tauri/Cargo.toml`.
- `CorrectionPairStore` already uses an atomic JSON sidecar under `%APPDATA%\PushToTalk\personalization\correction_pairs.json`; the same personalization directory is a natural home for a future SQLite file.

## Dependency check

- `cargo search rusqlite --limit 1` returned `rusqlite = "0.39.0"`.
- `cargo info rusqlite` confirms it is an ergonomic SQLite wrapper with docs at `https://docs.rs/rusqlite/` and repository `https://github.com/rusqlite/rusqlite`.

## Decision

Use `rusqlite = { version = "0.39.0", features = ["bundled"] }` for the first SQLite slice.

## Why

- The project is Windows-only, and the bundled feature avoids relying on system SQLite installation.
- A thin store module can be tested with temporary database files before production runtime paths switch from JSON/config to SQLite.
- Keeping this first slice as a sidecar store reduces migration risk: existing ASR/TNL/LLM consumers continue to use `AppConfig.dictionary` until a later task explicitly flips read paths.
