# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ⚠️ Windows 专属设计理念（重要）

**本项目是专门为 Windows 用户设计的桌面应用，不考虑跨平台兼容性。**

### 核心原则

1. **Windows Only** — 所有设计决策优先考虑 Windows 平台的最佳实践
2. **直接使用 Win32 API** — 不需要跨平台抽象层，可直接调用 Windows 原生 API
3. **Windows 特有功能优先** — 充分利用 Windows 独有的系统特性（如 `GetAsyncKeyState`、注册表、NSIS 安装器等）
4. **简化实现** — 不需要为 macOS/Linux 兼容性增加额外的条件编译或抽象

### 设计决策指南

| 场景 | 正确做法 | 错误做法 |
|------|---------|---------|
| 全局热键检测 | 直接用 Win32 `GetAsyncKeyState` | 用跨平台库的通用抽象 |
| 配置文件路径 | 直接用 `%APPDATA%` | 用 `dirs` crate 的跨平台路径 |
| 安装器 | 只构建 NSIS (.exe) | 同时构建 MSI/DMG/DEB |
| 系统托盘 | 使用 Windows 原生托盘 API | 考虑 macOS 菜单栏兼容 |
| 键盘模拟 | 直接用 Win32 `SendInput` | 用跨平台键盘模拟库 |
| 音频静音 | 直接用 Windows Audio Session API | 考虑 PulseAudio/CoreAudio |

### 已采用的 Windows 专属方案

- **Ghost Key Detection**: 使用 `GetAsyncKeyState` Win32 API 检测按键真实状态
- **Auto-Start**: 直接操作 Windows 注册表 `HKEY_CURRENT_USER\...\Run`
- **Installer**: 仅使用 NSIS 安装器，已移除 MSI 支持
- **Admin Rights**: 要求管理员权限运行（全局键盘钩子需要）

### 后续开发注意事项

- **不要**为了"以防万一"添加 `#[cfg(target_os = "...")]` 条件编译
- **不要**选择"跨平台兼容"的库而放弃更好的 Windows 原生方案
- **不要**在代码注释中提及"未来可能支持其他平台"
- **可以**直接使用 `windows` crate 调用 Win32 API
- **可以**假设用户环境是 Windows 10/11

### 版本兼容性原则

**本项目是单机桌面应用，无需考虑版本向前/向后兼容。**

- 用户通过自动更新升级后，直接运行新版本代码
- 不存在"旧版客户端访问新版服务器"等分布式兼容场景
- **放心修改**：数据结构、API 格式、配置字段等可直接调整，无需保留旧版兼容代码
- **例外情况**：仅在配置文件迁移时需要考虑（如 config.rs 中的自动迁移逻辑，确保旧配置能正常加载）

---

## Project Overview

PushToTalk is a desktop application built with Tauri 2.0 that enables voice-to-text input and AI-powered assistance via global keyboard shortcuts. The architecture follows a clear separation between:
- **React frontend** (TypeScript + Tailwind CSS) for UI
- **Rust backend** (Tauri) for system-level operations

### Core Workflows

**1. Dictation Mode (按住/松手录音)**
- Press Ctrl+Win (or custom hotkey) → Records audio → Release key → Transcribes via ASR API → Optional LLM post-processing → Auto-inserts text into active window
- Supports two recording modes:
  - **Press Mode**: Hold key to record, release to stop (traditional)
  - **Toggle Mode**: Press F2 to start, press F2 again to finish (prevents accidental stops)

**2. AI Assistant Mode (智能指令模式)**
- Press Alt+Space (or custom hotkey) → Captures selected text → Records voice command → Processes with context-aware LLM → Replaces/inserts text
- Two operating modes:
  - **Q&A Mode**: No text selected, ask questions and get answers
  - **Text Processing Mode**: Text selected, give commands to transform selected text

### Key Features

- **Dual Recording Modes**:
  - Press mode (hold to record)
  - Toggle/Release mode (press once to start, press again to finish, F2 default)
- **AI Assistant Mode**: Context-aware text processing with voice commands (Alt+Space default)
- **Custom Hotkey Binding**: 73 keys supported (modifiers, letters, numbers, F1-F12, arrows, etc.)
- **Multi-ASR Support**: Alibaba Qwen (realtime/HTTP), Doubao (realtime/HTTP), SiliconFlow SenseVoice
- **ASR Fallback Strategy**: Automatic fallback from Doubao/Qwen to SenseVoice with parallel racing
- **LLM Post-Processing**: Optional text polishing, translation, or formatting via OpenAI-compatible APIs
- **Visual Feedback**: Overlay window shows recording status with real-time waveform visualization
- **Audio Feedback**: Start/stop beep sounds via rodio player
- **Transcription History**: Automatic history tracking with search and copy functionality
- **System Tray**: Minimize to tray on close, auto-start on boot support
- **Multi-Configuration**: Save and switch between different LLM prompt presets
- **Auto-Update**: Tauri Plugin Updater v2 with 6 mirror endpoints for reliable updates
- **Personal Dictionary**: Custom hotword list for improved recognition of professional terms
- **VAD (Voice Activity Detection)**: Smart silence detection with hangover mechanism to prevent word clipping
- **AGC (Automatic Gain Control)**: Auto volume adjustment for better recognition of soft speech
- **Mute Other Apps**: Optional feature to mute other applications during recording
- **Multi-Monitor Support**: Overlay window automatically adapts to multi-display environments
- **Auto Vocabulary Learning**: Monitors user corrections to ASR errors and automatically learns proper nouns and terms
- **UIA Text Reading**: Uses Windows UI Automation API for non-intrusive text capture (replaces clipboard method)
- **LLM Provider Registry**: Multi-provider management with connection testing and latency display
- **Theme Support**: Light/dark theme switching

## Development Commands

### Development
```bash
npm install                    # Install frontend dependencies
npm run tauri dev             # Run dev server (requires admin rights on Windows)
```

⚠️ **Critical**: Must run with administrator privileges on Windows for global keyboard hook (`rdev`) to function.

### Building
```bash
npm run tauri build           # Build production bundles (NSIS installer only)
```

Output location: `src-tauri/target/release/bundle/`

**Note**: MSI installer support removed to prevent multiple instances during auto-update.

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

1. **hotkey_service.rs** - Custom dual-hotkey listener using `rdev`
   - Supports **73 keys**: modifiers, letters, numbers, F1-F12, arrows, navigation keys
   - **Dual hotkey system**: Independent dictation and assistant mode bindings
   - **Ghost key detection**: Windows Win32 API (`GetAsyncKeyState`) prevents stuck states from rdev event loss
   - **500ms watchdog timer**: Automatic state recovery for reliability
   - Thread-safe state management with `Arc<Mutex<bool>>`
   - Callback-based: `on_start()` and `on_stop()` closures passed to `start()`
   - **Platform requirement**: Windows admin rights mandatory

2. **audio_recorder.rs** - Real-time audio capture (non-streaming mode)
   - Uses `cpal` for cross-platform audio I/O
   - Handles F32/I16/U16 sample format conversion automatically
   - Audio stream lifecycle: Must keep stream alive in memory during recording
   - Outputs WAV files via `hound` to system temp directory

3. **streaming_recorder.rs** - Real-time streaming audio capture
   - For WebSocket-based realtime ASR (Qwen/Doubao)
   - Emits audio chunks via callback for low-latency transmission
   - Includes audio visualization data (RMS levels) for overlay window
   - **VAD integration**: Silence detection with 3-chunk hangover (0.6s) to prevent word clipping
   - **AGC integration**: Automatic gain control for consistent audio levels

4. **asr/** - Multi-provider ASR module (refactored architecture)
   - **asr/http/qwen.rs** - Qwen HTTP mode (multimodal-generation endpoint)
     - Supports personal dictionary via `vocabulary` parameter
   - **asr/http/doubao.rs** - Doubao HTTP mode
     - Supports personal dictionary via `additions` parameter
   - **asr/http/sensevoice.rs** - SiliconFlow SenseVoice fallback
   - **asr/realtime/qwen.rs** - Qwen WebSocket realtime mode
     - Supports personal dictionary via `vocabulary` in transcription config
   - **asr/realtime/doubao.rs** - Doubao WebSocket realtime mode
     - Supports personal dictionary via `additions` parameter
   - **asr/race_strategy.rs** - Parallel ASR request racing with automatic fallback
     - Primary ASR (Qwen/Doubao) retries up to 2 times with 500ms delay
     - SenseVoice runs in parallel background thread
     - Smart fallback: checks background result before each retry
     - Returns first success or combined error if both fail
   - **Timeout & Retry**: 6s request timeout with automatic retry (max 2 retries)
   - Base64 encodes audio before upload in HTTP mode

5. **llm_post_processor.rs** - Optional LLM text refinement (for dictation mode)
   - Sends transcribed text to OpenAI-compatible API with custom system prompts
   - Supports multiple presets: text polishing, translation, email formatting, etc.
   - Users can define custom scenarios via UI
   - HTTP connection pool optimized (increased pool limits)

6. **assistant_processor.rs** - AI Assistant mode processor
   - **Dual system prompts**:
     - `qa_system_prompt`: For Q&A mode (no selected text)
     - `text_processing_system_prompt`: For text processing mode (with selected text)
   - Independent LLM endpoint configuration (separate from dictation mode)
   - Context-aware processing based on clipboard content

7. **clipboard_manager.rs** - Context capture for AI Assistant
   - Captures selected text via clipboard with **3 retry attempts** (exponential backoff)
   - **100ms delay** after hotkey release before Ctrl+C (prevents modifier key conflicts)
   - RAII `ClipboardGuard` ensures automatic restoration even on panic
   - Uses `arboard` for clipboard operations + `win32_input` (Win32 SendInput API) for keyboard simulation

8. **text_inserter.rs** - Clipboard-based text injection
   - Strategy: Save clipboard → Copy text → Simulate Ctrl+V → Restore clipboard
   - Uses `arboard` (clipboard) + `win32_input` (Win32 SendInput API)
   - **Focus management**: 150ms delay before text insertion to restore window focus (for toggle mode)

9. **audio_utils.rs** - Audio processing utilities
   - **VAD (Voice Activity Detection)**: RMS-based silence detection with 0.003 threshold
   - **AGC (Automatic Gain Control)**: Automatic volume normalization
     - Target RMS: 0.10, Max gain: 5.0×, Min gain: 0.1×
     - Smooth transitions: fast attack (0.5), slow release (0.1)
     - Noise floor: 0.003 (below this, gain stays at 1.0)
   - RMS calculation for waveform visualization (×1.5 amplification for normal, ×1.8 for locked)
   - Audio format conversion helpers
   - Smooth transitions: 70%/30% on rise, 40%/60% on fall

10. **beep_player.rs** - Audio feedback system
    - Plays start/stop beep sounds using `rodio` crate
    - Provides tactile feedback for recording state changes

11. **config.rs** - Persistent configuration
    - Stores all API keys and settings in `%APPDATA%\PushToTalk\config.json`
    - **Automatic migration** from old single-hotkey to dual-hotkey system
    - **Backward compatibility** for SmartCommandConfig → AssistantConfig
    - Supports multiple LLM prompt presets with custom names
    - Manages minimize-to-tray and auto-start preferences
    - **Personal dictionary**: Custom hotword list (`dictionary: Vec<String>`)
    - **Mute other apps**: `enable_mute_other_apps` option
    - Uses `dirs` crate for cross-platform app data directory

12. **pipeline/** - Processing pipeline framework
    - **pipeline/normal.rs** - Dictation mode pipeline: ASR → Optional LLM → Insert
    - **pipeline/assistant.rs** - AI Assistant pipeline: Capture → ASR → Context LLM → Replace
    - Pipeline result structure tracks timing metrics (ASR time, LLM time, total time)
    - Clean separation of concerns between transcription and text processing
    - **Learning integration**: Triggers vocabulary learning observation after text insertion

13. **learning/** - Auto vocabulary learning module
    - **learning/coordinator.rs** - Learning workflow orchestrator
      - Triggers observation after ASR text insertion
      - Manages observation tasks per window (deduplication)
      - Configurable observation duration (default 5 seconds)
    - **learning/diff_analyzer.rs** - Text difference analyzer
      - Word-level diff detection between ASR output and user corrections
      - Context extraction for LLM judgment
      - Handles CJK and ASCII text boundaries
    - **learning/llm_judge.rs** - LLM-based vocabulary judgment
      - Determines if corrections are proper nouns, terms, or frequent words
      - Uses configurable LLM provider from shared config
    - **learning/validator.rs** - ASR text presence validator
      - Verifies ASR text still exists in target window
      - Uses UIA text reader for non-intrusive validation
    - **learning/store.rs** - Dictionary entry storage
      - Manages auto-learned vocabulary entries
      - Tracks word frequency and last used time
    - **Event emission**: `learning_suggestion` event for frontend toast notifications

14. **uia_text_reader.rs** - Windows UI Automation text reader
    - Non-intrusive text capture from focused windows
    - Uses `IUIAutomationTextPattern` and `IUIAutomationValuePattern`
    - **COM initialization**: RAII `ComGuard` for proper cleanup
    - **Timeout protection**: 2-second UIA call timeout
    - **Concurrency control**: Max 2 concurrent UIA workers
    - **Blacklist mechanism**: Temporarily skips problematic windows (3 failures → 30s blacklist)
    - Replaces clipboard-based text capture for learning validation

15. **openai_client.rs** - Shared OpenAI-compatible API client
    - Unified HTTP client for all LLM calls
    - Connection testing with latency measurement
    - Supports provider registry configuration

### Frontend Architecture (src/)

Multi-page React app with Tauri IPC communication:

- **Main Window (App.tsx)**: Configuration UI with sidebar navigation
  - **Pages**:
    - `DashboardPage.tsx` - Overview and quick actions
    - `AsrPage.tsx` - ASR provider selection and configuration
    - `ModelsPage.tsx` - LLM provider registry management (NEW)
      - Add/edit/delete LLM providers
      - Connection testing with latency display
      - Default provider selection
    - `LlmPage.tsx` - Text polishing configuration
    - `AssistantPage.tsx` - AI Assistant mode settings
    - `HotkeysPage.tsx` - Dual hotkey configuration
    - `DictionaryPage.tsx` - Personal dictionary management
      - Manual and auto-learned entries
      - Source badges (manual/auto)
    - `PreferencesPage.tsx` - System preferences (theme, tray, auto-start)
    - `HistoryPage.tsx` - Transcription history with search
    - `HelpPage.tsx` - Help and support links
  - **Components**:
    - `Sidebar.tsx` - Navigation sidebar with icons
    - `TopStatusBar.tsx` - Service status and quick controls
    - `LlmConnectionConfig.tsx` - Reusable LLM provider selector
    - `ThemeSelector.tsx` - Light/dark theme toggle
    - `VocabularyLearningToast.tsx` - Learning suggestion notifications

- **Overlay Window (src/windows/OverlayWindow.tsx)**: Floating recording status indicator
  - **Three visual states**:
    1. **Recording (Normal)**: 9-bar waveform, red gradient pill
    2. **Recording (Locked/Toggle)**: 5-bar mini waveform + dual control buttons, blue pill (#3B82F6)
       - Cancel button (X icon) on left
       - Finish button (checkmark) on right
    3. **Transcribing**: Dot matrix + rotating sun icon
  - Real-time audio waveform visualization (min 4px, max 24px normal / 20px locked)
  - Auto-hides when idle, appears during recording
  - Always-on-top, click-through window
  - **Safety mechanisms**:
    - 15-second transcription timeout with forced hide
    - 60-second locked recording timeout with auto-cancel
    - Duplicate listener prevention
    - Submit button debouncing

- **Notification Window (src/windows/NotificationWindow.tsx)**: Learning suggestion toast
  - Displays vocabulary learning suggestions
  - Accept/reject buttons for user feedback
  - Shows diff context (original → corrected)
  - Auto-dismiss after timeout

- **State Management**: React hooks (useState, useEffect) for local state
- **Tauri Communication**:
  - `invoke()` for commands: `save_config`, `load_config`, `start_app`, `stop_app`, `get_history`, `get_auto_start_enabled`, `set_auto_start`, etc.
  - `listen()` for events: `recording_started`, `recording_stopped`, `recording_locked`, `transcribing`, `transcription_complete`, `error`, `audio_level`, `overlay_update`, `learning_suggestion`

### Critical Event Flow

#### Dictation Mode (Press Mode - Traditional)
```
User presses Ctrl+Win
  → hotkey_service detects via rdev callback
  → Calls on_start() closure
  → Emits "recording_started" event to frontend
  → Emits "overlay_update" with state: "listening"
  → streaming_recorder.start_recording() / audio_recorder.start_recording()
  → Periodic "audio_level" events for waveform visualization
  → Beep player plays start sound

User releases key
  → hotkey_service detects release
  → Calls on_stop() closure
  → Emits "recording_stopped" event
  → Emits "overlay_update" with state: "processing"
  → Beep player plays stop sound
  → streaming_recorder.stop_recording() / audio_recorder.stop_recording()
  → Emits "transcribing" event
  → Pipeline processing begins
```

#### Dictation Mode (Toggle Mode - Release Lock)
```
User presses F2 (first time)
  → hotkey_service detects press
  → Sets is_recording_locked to true
  → Calls on_start() closure
  → Emits "recording_locked" event to frontend
  → Emits "overlay_update" with state: "locked"
  → streaming_recorder.start_recording()
  → 60-second safety timer starts
  → Beep player plays start sound

User presses F2 (second time) or timeout
  → hotkey_service detects press or timer expires
  → Race condition protection with is_processing_stop atomic flag
  → Sets is_recording_locked to false
  → Calls on_stop() closure
  → Emits "recording_stopped" event
  → Hides overlay window
  → 150ms delay to restore window focus
  → Beep player plays stop sound
  → Pipeline processing begins
```

#### AI Assistant Mode
```
User selects text and presses Alt+Space
  → hotkey_service detects assistant hotkey
  → Waits 100ms after key release
  → clipboard_manager.capture_selected_text()
    → Simulates Ctrl+C
    → Retries up to 3 times with exponential backoff
    → Stores in ClipboardGuard (RAII)
  → Starts audio recording
  → Emits "recording_started" event

User releases Alt+Space (or presses again if toggle mode)
  → Stops recording
  → ASR transcription
  → assistant_processor.process()
    → If clipboard has content: text_processing_system_prompt
    → If clipboard empty: qa_system_prompt
    → Sends to LLM with context
  → text_inserter.insert_text() replaces/inserts result
  → ClipboardGuard restores original clipboard
```

#### Pipeline Processing Flow
```
Realtime Mode (WebSocket):
  → Audio chunks sent during recording via WebSocket
  → Partial results received and accumulated
  → Final transcription on stream close

HTTP Mode:
  → Complete WAV file uploaded after recording stops
  → Single transcription result returned

ASR Fallback (race_strategy.rs):
  → Primary ASR (Qwen/Doubao) starts
  → SenseVoice starts in parallel background thread
  → Primary retries up to 2 times (500ms delay)
  → Before each retry, checks if SenseVoice succeeded
  → Returns first success or waits for both results
  → If both fail, returns combined error

Post-Processing (Normal Pipeline):
  → (Optional) llm_post_processor.process() refines text
  → text_inserter.insert_text() injects result
  → Emits "transcription_complete" with final text
  → Emits "overlay_update" with state: "success"
  → Saves to history
  → Deletes temp audio file (if applicable)

AI Assistant Pipeline:
  → assistant_processor.process() with context
  → text_inserter.insert_text() replaces selection or inserts at cursor
  → Emits "transcription_complete"
  → Saves to history

Learning Observation Flow (after text insertion):
  → Pipeline triggers start_learning_observation()
  → Waits observation_duration_secs (default 5s)
  → UIA text reader captures current window text
  → Validates ASR text still present
  → diff_analyzer detects user corrections
  → LLM judge evaluates if correction is vocabulary-worthy
  → Emits "learning_suggestion" event to frontend
  → User accepts/rejects via notification toast
  → Accepted words added to dictionary with "auto" source
```

### Tauri IPC Commands (lib.rs)

All backend functions exposed via `#[tauri::command]`:

- `save_config(config: AppConfig)` - Persist full configuration to disk with automatic migration
- `load_config()` - Load saved configuration
- `start_app(window: Window, config: AppConfig)` - Initialize all services and start hotkey listener
- `stop_app()` - Cleanup and stop services
- `get_history()` - Retrieve transcription history
- `clear_history()` - Delete all history records
- `get_auto_start_enabled()` - Check if auto-start is enabled
- `set_auto_start(enable: bool)` - Toggle auto-start on boot (requires admin)
- `test_llm_connection(endpoint, api_key, model)` - Test LLM provider connection with latency measurement (NEW)
- `accept_learning_suggestion(word)` - Accept vocabulary learning suggestion (NEW)
- `reject_learning_suggestion(id)` - Reject vocabulary learning suggestion (NEW)

The `AppState` struct manages shared mutable state across all services using `Arc<Mutex<>>` and atomic flags.

### System Tray Integration

- **Minimize to Tray**: Configurable option to minimize instead of close
- **Auto-Start**: Windows registry-based auto-start on boot (requires admin)
- **Tray Menu**: Show/Hide window, Quit application

### Auto-Update System

- **Tauri Plugin Updater v2**: `tauri-plugin-updater` with passive install mode
- **6 Mirror Endpoints**: gh-proxy.org, hk.gh-proxy.org, cdn.jsdelivr.net, github.com, cdn.gh-proxy.org, edgeone.gh-proxy.org
- **Ed25519 Signature**: Public key verification for security
- **Artifact Creation**: `createUpdaterArtifacts: true` in tauri.conf.json
- **Update Flow**: Check → Download → Verify → Install → Restart
- **Current Version**: 1.5.2

## Important Implementation Details

### Audio Recording Lifecycle
The audio stream from `cpal` is NOT Send-safe. The current solution spawns a dedicated thread that owns the stream and polls `is_recording` flag. Alternative approaches (storing stream in struct) will fail compilation.

### Global Hotkey Detection
`rdev` requires system-level permissions. On Windows, this means:
- Must launch with administrator privileges
- Alternative: Use `tauri-plugin-global-shortcut` (not implemented)
- **Ghost key detection** via Win32 `GetAsyncKeyState` API prevents stuck states

### Custom Hotkey System
73 keys supported across categories:
- **Modifiers**: ControlLeft/Right, ShiftLeft/Right, AltLeft/Right, MetaLeft/Right
- **Letters**: A-Z (26 keys)
- **Numbers**: 0-9 (10 keys)
- **Function**: F1-F12 (12 keys)
- **Navigation**: Arrow keys, Home, End, PageUp, PageDown
- **Special**: Space, Tab, Escape, Return, Backspace, Delete, Insert, CapsLock

Configuration stored as `Vec<HotkeyKey>` with support for multi-key combinations.

### Toggle Mode (Release Lock) Implementation
- **Atomic lock flag**: `is_recording_locked: Arc<AtomicBool>`
- **Race condition protection**: `is_processing_stop` atomic flag
- **60-second safety timer**: Auto-cancel long recordings
- **Focus management**: 150ms delay before text insertion to restore window focus
- **Overlay window hide**: Ensures target window is focused before Ctrl+V

### AI Assistant Context Capture
- **100ms delay** after hotkey release before Ctrl+C simulation
- **3 retry attempts** with exponential backoff (1s, 2s, 4s)
- **ClipboardGuard**: RAII pattern ensures restoration even on panic
- **Dual system prompts**: Different prompts for Q&A vs text processing

### API Response Format

**Qwen ASR response structure:**
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

**Doubao ASR WebSocket message:**
- Event-driven: `"speech_start"`, `"partial_result"`, `"final_result"`, `"speech_end"`
- Partial results contain incremental text that gets accumulated
- Final result emitted on stream closure

**SenseVoice HTTP response:**
```json
{
  "data": {
    "text": "transcribed text"
  }
}
```

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
- Solution: Automatic 6s timeout with 2 retry attempts (500ms delay between)
- Fallback: SenseVoice runs in parallel and will take over if primary fails

### "HTTP connection pool exhausted"
- Cause: Default reqwest pool size too small for concurrent requests
- Solution: Configure custom HTTP client with increased pool limits (see llm_post_processor.rs)
- Implementation: `.pool_max_idle_per_host()` and `.pool_idle_timeout()`

### Overlay window not showing
- Cause: Window creation race condition or Tauri event timing
- Solution: Ensure overlay window is created in `tauri.conf.json` with `"visible": false` initially
- Trigger visibility via IPC events after main window ready

### Auto-start not working
- Cause: Windows registry requires admin rights to modify
- Solution: Must run installer with admin privileges
- Registry path: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`

### Toggle mode text not inserting
- Cause: Target window loses focus when overlay is visible
- Solution: Hide overlay 150ms before text insertion to restore focus

### Ghost key stuck state
- Cause: rdev occasionally misses key release events
- Solution: Win32 `GetAsyncKeyState` verification + 500ms watchdog timer

### Multiple instances after update
- Cause: MSI installers bypass Tauri's single-instance mechanism
- Solution: Removed MSI support, use NSIS installer only

## Configuration

Config file location: `%APPDATA%\PushToTalk\config.json`

### Configuration Structure

```json
{
  "asr_config": {
    "credentials": {
      "qwen_api_key": "sk-...",
      "sensevoice_api_key": "sk-...",
      "doubao_app_id": "...",
      "doubao_access_token": "..."
    },
    "selection": {
      "active_provider": "qwen",
      "enable_fallback": true,
      "fallback_provider": "siliconflow"
    }
  },
  "use_realtime_asr": true,
  "dual_hotkey_config": {
    "dictation": {
      "keys": ["control_left", "meta_left"],
      "release_mode_keys": ["f2"]
    },
    "assistant": {
      "keys": ["alt_left", "space"]
    }
  },
  "enable_llm_post_process": true,
  "llm_config": {
    "shared": {
      "providers": [
        {
          "id": "zhipu",
          "name": "智谱AI",
          "endpoint": "https://open.bigmodel.cn/api/paas/v4/chat/completions",
          "api_key": "sk-...",
          "default_model": "glm-4-flash"
        }
      ],
      "default_provider_id": "zhipu",
      "polishing_provider_id": "zhipu",
      "assistant_provider_id": "zhipu",
      "learning_provider_id": "zhipu"
    },
    "feature_override": {
      "use_shared": true
    },
    "presets": [
      {"id": "1", "name": "文本润色", "system_prompt": "..."},
      {"id": "2", "name": "中译英", "system_prompt": "..."}
    ],
    "active_preset_id": "1"
  },
  "assistant_config": {
    "enabled": true,
    "llm": {"use_shared": true},
    "qa_system_prompt": "You are a helpful assistant...",
    "text_processing_system_prompt": "You are an expert text editor..."
  },
  "learning_config": {
    "enabled": true,
    "observation_duration_secs": 5,
    "feature_override": {"use_shared": true}
  },
  "dictionary": ["专业术语1", "人名|auto", "地名"],
  "enable_mute_other_apps": false,
  "close_action": "minimize",
  "theme": "light"
}
```

### API Key Sources

- **DashScope (Qwen)**: https://bailian.console.aliyun.com/?tab=model#/api-key
- **Doubao (ByteDance)**:
  - Recording file recognition: https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition
  - Streaming recognition: https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition
- **SiliconFlow**: https://cloud.siliconflow.cn/me/account/ak
- **ZhipuAI (GLM-4-Flash)**: https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414

## Key Architecture Decisions

1. **Pipeline Pattern**: Separates concerns between transcription, LLM processing, and text insertion
2. **Dual Processor Design**: AssistantProcessor separate from LlmPostProcessor (independent API configs)
3. **RAII Guards**: ClipboardGuard ensures restoration even on panic
4. **Atomic State Management**: Lock-free concurrency for recording state
5. **Event-Driven UI**: Tauri events for real-time frontend updates
6. **Race Strategy**: Parallel ASR racing maximizes reliability without sacrificing speed
7. **Ghost Key Detection**: Win32 API verification prevents stuck states from rdev event loss
8. **Focus Management**: Overlay window hiding + delay ensures text insertion succeeds
9. **Dual Hotkey System**: Independent configuration for dictation and assistant modes
10. **Automatic Migration**: Backward compatibility for configuration schema changes
11. **LLM Provider Registry**: Centralized provider management with per-feature binding
12. **UIA Text Reading**: Non-intrusive text capture via Windows UI Automation API
13. **Auto Vocabulary Learning**: Intelligent correction monitoring with LLM-based judgment
