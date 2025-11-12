# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PushToTalk is a desktop application built with Tauri 2.0 that enables voice-to-text input via global keyboard shortcuts. The architecture follows a clear separation between:
- **React frontend** (TypeScript + Tailwind CSS) for UI
- **Rust backend** (Tauri) for system-level operations

The application flow: User presses Ctrl+Win → Records audio → Releases key → Transcribes via Alibaba Qwen ASR API → Auto-inserts text into active window.

## Development Commands

### Development
```bash
npm install                    # Install frontend dependencies
npm run tauri dev             # Run dev server (requires admin rights on Windows)
```

⚠️ **Critical**: Must run with administrator privileges on Windows for global keyboard hook (`rdev`) to function.

### Building
```bash
npm run tauri build           # Build production bundles (MSI + NSIS installers)
```

Output location: `src-tauri/target/release/bundle/`

### Testing API Integration
```bash
cd src-tauri
cargo run --bin test_api      # Standalone tool to test Qwen ASR API
```

See `测试工具使用说明.md` for detailed usage.

### Rust-only Development
```bash
cd src-tauri
cargo build                   # Build Rust backend only
cargo check                   # Fast compile check
```

## Architecture & Key Patterns

### Backend Modules (src-tauri/src/)

The Rust backend is organized into independent modules that communicate through the main lib.rs orchestrator:

1. **hotkey_service.rs** - Global keyboard listener using `rdev`
   - Monitors Ctrl+Win key combination
   - Thread-safe state management with `Arc<Mutex<bool>>`
   - Callback-based: `on_start()` and `on_stop()` closures passed to `start()`
   - **Platform requirement**: Windows admin rights mandatory

2. **audio_recorder.rs** - Real-time audio capture
   - Uses `cpal` for cross-platform audio I/O
   - Handles F32/I16/U16 sample format conversion automatically
   - Audio stream lifecycle: Must keep stream alive in memory during recording
   - Outputs WAV files via `hound` to system temp directory

3. **qwen_asr.rs** - Speech-to-text API client
   - Integrates Alibaba DashScope qwen3-asr-flash model
   - Base64 encodes audio before upload
   - **Important**: Automatically strips trailing punctuation from transcription
   - Uses multimodal-generation endpoint (not the old ASR endpoint)
   - **Timeout & Retry**: 6s request timeout with automatic retry (max 2 retries)
   - Error handling with detailed logging for debugging

4. **text_inserter.rs** - Clipboard-based text injection
   - Strategy: Save clipboard → Copy text → Simulate Ctrl+V → Restore clipboard
   - Uses `arboard` (clipboard) + `enigo` (keyboard simulation)

5. **config.rs** - Persistent configuration
   - Stores DashScope API key in `%APPDATA%\PushToTalk\config.json`
   - Uses `dirs` crate for cross-platform app data directory

### Frontend Architecture (src/)

Single-page React app with direct Tauri IPC communication:

- **State Management**: React hooks (no external state library needed)
- **Tauri Communication**:
  - `invoke()` for commands: `save_config`, `load_config`, `start_app`, `stop_app`
  - `listen()` for events: `recording_started`, `recording_stopped`, `transcribing`, `transcription_complete`, `error`

### Critical Event Flow

```
User presses Ctrl+Win
  → hotkey_service detects via rdev callback
  → Calls on_start() closure
  → Emits "recording_started" event to frontend
  → audio_recorder.start_recording() captures stream

User releases key
  → hotkey_service detects release
  → Calls on_stop() closure
  → audio_recorder.stop_recording() saves WAV
  → Emits "transcribing" event
  → qwen_asr.transcribe() uploads to API
  → text_inserter.insert_text() injects result
  → Emits "transcription_complete" with text
  → Deletes temp audio file
```

### Tauri IPC Commands (lib.rs)

All backend functions exposed via `#[tauri::command]`:

- `save_config(api_key: String)` - Persist API key to disk
- `load_config()` - Load saved configuration
- `start_app(api_key: String)` - Initialize all services and start hotkey listener
- `stop_app()` - Cleanup and stop services

The `AppState` struct manages shared mutable state across all services using `Arc<Mutex<>>`.

## Important Implementation Details

### Audio Recording Lifecycle
The audio stream from `cpal` is NOT Send-safe. The current solution spawns a dedicated thread that owns the stream and polls `is_recording` flag. Alternative approaches (storing stream in struct) will fail compilation.

### Global Hotkey Detection
`rdev` requires system-level permissions. On Windows, this means:
- Must launch with administrator privileges
- Alternative: Use `tauri-plugin-global-shortcut` (not implemented in MVP)

### API Response Format
Qwen ASR response structure:
```json
{
  "output": {
    "choices": [{
      "message": {
        "content": [{"text": "transcribed text"}]
      }
    }]
  }
}
```

Parse via: `result["output"]["choices"][0]["message"]["content"][0]["text"]`

### Binary Configuration
The project has two binaries defined in Cargo.toml:
- `push-to-talk` (main app) - default-run
- `test_api` (standalone API tester)

Run specific binary: `cargo run --bin test_api`

## Common Issues & Solutions

### "Audio file is empty" error
- Cause: Audio stream dropped too early
- Current fix: Thread-based stream ownership in audio_recorder.rs

### "No keyboard events detected"
- Cause: Missing administrator privileges
- Solution: Right-click → Run as Administrator

### Compilation error with single quotes in char array
- Cause: Rust requires escaping single quotes in char literals
- Fix: Use `'\''` instead of `'''`

### "Transcription timeout" or API hangs
- Cause: API request taking too long or network issues
- Solution: Automatic 6s timeout with 2 retry attempts
- Implementation: Uses `reqwest::Client` with timeout configuration

## Configuration

Config file location: `%APPDATA%\PushToTalk\config.json`

Get DashScope API key: https://dashscope.console.aliyun.com/
