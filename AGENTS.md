# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the React + TypeScript UI (main window, overlay, and notification windows).
- `index.html`, `overlay.html`, and `notification.html` are the Vite entry points for the three windows.
- `src-tauri/` contains the Rust backend, `src-tauri/tauri.conf.json`, and app icons in `src-tauri/icons/`.
- `ui/` is for auxiliary UI assets or prototypes used by the frontend.
- `scripts/` has build/release helpers; `dist/` is generated frontend output.
- `tests/` contains TypeScript runtime regression tests (`tsx --test`).

### Key Backend Modules (src-tauri/src/)
- `asr/` - Multi-provider ASR with HTTP and realtime modes (Qwen, Doubao, Doubao IME, SenseVoice)
- `pipeline/` - Processing pipelines for dictation and assistant modes
- `tnl/` - Technical Normalization Layer between ASR and LLM (pinyin/phonetic matching, letter merge, hyphen rewrite)
- `learning/` - Auto vocabulary learning (coordinator, diff_analyzer, llm_judge, validator, store)
- `builtin_dictionary_updater.rs` - Remote builtin hotwords fetch + atomic cache persistence + runtime refresh events
- `platform/` - Native capability factory, Windows and macOS implementations
- `platform/windows/uia_text_reader.rs` - Windows UI Automation text capture
- `openai_client.rs` - Shared LLM client with connection testing
- `config.rs` - Configuration management with automatic migration

### Key Frontend Structure (src/)
- `pages/` - Page components (Dashboard, ASR, Models, LLM, Assistant, Hotkeys, Dictionary, Preferences, History, Help)
- `components/` - Reusable UI components (common/, layout/, learning/, live/, modals/, history/, notice/)
- `windows/` - Overlay and notification window components
- `hooks/` - Custom React hooks (useAppServiceController, useDictionary, useHotkeyRecording, useUpdater, useTauriEventListeners)
- `types/` - TypeScript type definitions
- `utils/` - Utility functions (dictionaryUtils)

## Build, Test, and Development Commands
- `npm install` installs frontend dependencies.
- `npm run dev` starts the Vite dev server for the UI.
- `npm run build` type-checks and builds the frontend bundle.
- `npm run preview` serves the built UI locally.
- `npm run test:ts` runs TypeScript runtime tests in `tests/*.test.ts`.
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
- Frontend: run `npm run test:ts`; additionally smoke-test via `npm run dev` and `npm run build`.
- Final quality gate: ensure overall Cargo compilation passes in `src-tauri/` (at least `cargo check`; prefer `cargo build` for release readiness).

## Platform Architecture Notes
- Windows 10/11 remains supported; macOS is an experimental native adapter. Follow `PLATFORM_ARCHITECTURE.md`. Shared business logic calls `platform` capabilities; keep native APIs and conditional compilation inside that boundary.
- Build and test each OS with its native toolchain. Windows regression validation must run on Windows; macOS validation must run on macOS. Do not claim one platform was tested from a build on the other.
- Preserve Windows-native hotkeys/input (GetAsyncKeyState, SendInput), UIA and audio session behavior in `platform/windows/`. macOS may use different native APIs.
- Follow the existing administrator workflow for Windows hotkeys; preserve ghost-key detection and the 500ms watchdog. macOS uses explicit system privacy permissions.
- Keep clipboard/focus timing safeguards (100ms delay before capture, 150ms delay before insert) in assistant/overlay flows.
- Config lives at `%APPDATA%\PushToTalk\config.json`; migration logic is in `src-tauri/src/config.rs`.
- UIA text reader uses Windows UI Automation API; maintain COM initialization guards and timeout protection.
- Learning module uses async observation tasks; respect deduplication per opaque `InputTarget`.

## Commit & Pull Request Guidelines
- Follow Conventional Commit-style prefixes seen in history: `feat:`, `fix:`, `perf:`, `refactor:`, `chore:`; short summaries can be Chinese or English.
- PRs should include a clear description, test steps, and screenshots for UI changes; link related issues when possible.
- Keep changes scoped and call out any Windows/admin-impacting behavior.

## Security & Configuration Tips
- Do not commit API keys or local config files.
- Windows auto-update uses NSIS; avoid reintroducing MSI or multi-instance installers. macOS packaging uses its platform config; signing, notarization and release updates require separate validation.
- LLM provider credentials are stored in config.json; ensure proper migration when changing schema.
- For deeper architecture details, see `CLAUDE.md`.

## Recent Feature Areas
- **LLM Provider Registry**: Multi-provider management in `ModelsPage.tsx` and `config.rs`
- **Auto Vocabulary Learning**: `learning/` module with UIA text capture
- **TNL Normalization Layer**: `tnl/` module for deterministic ASR normalization (pinyin/phonetic/hyphen/letter-merge)
- **Doubao IME First-Class ASR**: default provider, automatic credential bootstrap, and startup fallback behavior
- **Builtin Dictionary Runtime Refresh**: `builtin_dictionary_updater.rs` + `builtin_dictionary_updated` event + dynamic frontend domain snapshot
- **Tray Quick Switches**: runtime toggles for post-processing/dictionary enhancement and ASR provider switching from tray menu
- **Update Notes Aggregation**: cross-version release notes merge in updater modal (`releaseNotes.ts`)
- **Global Notice Capsule**: floating notification host (`GlobalNoticeHost.tsx`, `NoticeCapsule.tsx`)
- **Doubao Realtime Tuning**: bidirectional streaming path and parameter tuning in `asr/realtime/doubao.rs`
- **Polishing Failure Feedback**: runtime `polishing_failed` hint path from normal pipeline to frontend
- **Connection Testing**: `test_llm_provider` command with latency measurement

## 开发文档

项目的事件契约、ASR、TNL、数据库与窗口约定见 [开发文档索引](docs/development/README.md)，按当前修改内容查阅。
