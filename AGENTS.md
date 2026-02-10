# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the React + TypeScript UI (main window, overlay, and notification windows).
- `index.html`, `overlay.html`, and `notification.html` are the Vite entry points for the three windows.
- `src-tauri/` contains the Rust backend, `src-tauri/tauri.conf.json`, and app icons in `src-tauri/icons/`.
- `ui/` is for auxiliary UI assets or prototypes used by the frontend.
- `scripts/` has build/release helpers; `dist/` is generated frontend output.

### Key Backend Modules (src-tauri/src/)
- `asr/` - Multi-provider ASR with HTTP and realtime modes (Qwen, Doubao, SenseVoice)
- `pipeline/` - Processing pipelines for dictation and assistant modes
- `learning/` - Auto vocabulary learning (coordinator, diff_analyzer, llm_judge, validator, store)
- `uia_text_reader.rs` - Windows UI Automation text capture
- `openai_client.rs` - Shared LLM client with connection testing
- `config.rs` - Configuration management with automatic migration

### Key Frontend Structure (src/)
- `pages/` - Page components (Dashboard, ASR, Models, LLM, Assistant, Hotkeys, Dictionary, Preferences, History, Help)
- `components/` - Reusable UI components (common/, layout/, learning/, live/, modals/, history/)
- `windows/` - Overlay and notification window components
- `hooks/` - Custom React hooks (useAppServiceController, useDictionary, useHotkeyRecording, useUpdater)
- `types/` - TypeScript type definitions
- `utils/` - Utility functions (dictionaryUtils)

## Build, Test, and Development Commands
- `npm install` installs frontend dependencies.
- `npm run dev` starts the Vite dev server for the UI.
- `npm run build` type-checks and builds the frontend bundle.
- `npm run preview` serves the built UI locally.
- `npm run tauri dev` runs the desktop app in dev mode; run as Administrator so global hotkeys work.
- `npm run tauri build` builds the NSIS installer only; output in `src-tauri/target/release/bundle/`.
- `cd src-tauri` then `cargo build`, `cargo check`, or `cargo test` for the Rust backend.
- `cd src-tauri` then `cargo run --bin test_api` to manually verify ASR API behavior.

## Coding Style & Naming Conventions
- TypeScript/React: 2-space indent, double quotes, and semicolons; components use `PascalCase`, hooks use `useX`, and UI files live in `*.tsx`.
- Rust: 4-space indent, `snake_case` for modules/functions and `CamelCase` for types; run `cargo fmt` before pushing.
- Tailwind CSS is used in JSX; keep class ordering consistent with nearby files.

## Testing Guidelines
- Development must follow TDD: write/adjust test methods first, then implement code.
- Validate test feasibility before implementation by running targeted tests and confirming they execute meaningfully.
- Implement only after test validation, then make tests pass and refactor within scope.
- Backend: run `cargo test` in `src-tauri/` for Rust tests.
- API checks: use `cargo run --bin test_api` when touching ASR integrations.
- Frontend: no dedicated JS test runner yet; smoke-test via `npm run dev` and `npm run build`.
- Final quality gate: ensure overall Cargo compilation passes in `src-tauri/` (at least `cargo check`; prefer `cargo build` for release readiness).

## Windows-Only & Architecture Notes
- This repo targets Windows 10/11 only; avoid cross-platform abstractions and `#[cfg(target_os = ...)]` branches unless required.
- All compile/build/package steps are Windows-only; always use Windows tooling/commands (PowerShell, `npm run tauri ...`, `cargo` on Windows) and avoid Linux/macOS build paths.
- Prefer Win32 APIs for hotkeys/input (GetAsyncKeyState, SendInput) and registry for auto-start.
- Global hotkeys require admin rights; preserve ghost-key detection and the 500ms watchdog when editing hotkey logic.
- Keep clipboard/focus timing safeguards (100ms delay before capture, 150ms delay before insert) in assistant/overlay flows.
- Config lives at `%APPDATA%\PushToTalk\config.json`; migration logic is in `src-tauri/src/config.rs`.
- UIA text reader uses Windows UI Automation API; maintain COM initialization guards and timeout protection.
- Learning module uses async observation tasks; respect the deduplication mechanism per window handle.

## Commit & Pull Request Guidelines
- Follow Conventional Commit-style prefixes seen in history: `feat:`, `fix:`, `perf:`, `refactor:`, `chore:`; short summaries can be Chinese or English.
- PRs should include a clear description, test steps, and screenshots for UI changes; link related issues when possible.
- Keep changes scoped and call out any Windows/admin-impacting behavior.

## Security & Configuration Tips
- Do not commit API keys or local config files.
- Auto-update uses NSIS; avoid reintroducing MSI or multi-instance installers.
- LLM provider credentials are stored in config.json; ensure proper migration when changing schema.
- For deeper architecture details, see `CLAUDE.md`.

## Recent Feature Areas
- **LLM Provider Registry**: Multi-provider management in `ModelsPage.tsx` and `config.rs`
- **Auto Vocabulary Learning**: `learning/` module with UIA text capture
- **Theme Support**: Light/dark themes via `ThemeSelector.tsx` and CSS variables
- **Connection Testing**: `test_llm_connection` command with latency measurement
