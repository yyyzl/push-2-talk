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

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **push2talk-rust** (2789 symbols, 6025 relationships, 239 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/push2talk-rust/context` | Codebase overview, check index freshness |
| `gitnexus://repo/push2talk-rust/clusters` | All functional areas |
| `gitnexus://repo/push2talk-rust/processes` | All execution flows |
| `gitnexus://repo/push2talk-rust/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->
