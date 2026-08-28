# Qwen ASR Profiles

> Executable cross-layer contracts for selecting and running Qwen Audio 3.0 or the Qwen3 legacy ASR integration.

---

## Scenario: Latest-by-default Qwen ASR With Explicit Legacy Compatibility

### 1. Scope / Trigger

- Trigger: any change to `QwenAsrProfile`, `AsrConfig.qwen_profile`, Qwen model IDs, Qwen HTTP request/response parsing, Qwen realtime WebSocket events, or the frontend Qwen profile selector.
- A profile is a paired HTTP + realtime protocol choice. It is not an arbitrary model string.
- `qwen_audio_3` is the default for new and pre-profile configurations. `qwen3_legacy` remains an explicit manual compatibility option.
- Request-level automatic retry from the latest profile to the legacy profile is forbidden. It would double requests, hide protocol failures, and make billing/diagnostics ambiguous.

### 2. Signatures

Backend configuration:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QwenAsrProfile {
    #[default]
    #[serde(rename = "qwen_audio_3", alias = "qwen_audio3")]
    QwenAudio3,
    Qwen3Legacy,
}

pub struct AsrConfig {
    pub credentials: AsrCredentials,
    pub selection: AsrSelection,
    #[serde(default)]
    pub qwen_profile: QwenAsrProfile,
    #[serde(default)]
    pub language_mode: AsrLanguageMode,
}
```

Frontend configuration:

```typescript
export type QwenAsrProfile = "qwen_audio_3" | "qwen3_legacy";

export interface AsrConfig {
  credentials: AsrCredentials;
  selection: AsrSelection;
  qwen_profile: QwenAsrProfile;
  language_mode: AsrLanguageMode;
}
```

Runtime constructors:

```rust
QwenASRClient::new_with_profile_and_correction_pairs(
    api_key,
    dictionary,
    correction_pairs,
    language_mode,
    profile,
)

QwenRealtimeClient::new_with_profile_and_correction_pairs(
    api_key,
    dictionary,
    correction_pairs,
    language_mode,
    profile,
)
```

### 3. Contracts

| Profile | HTTP contract | Realtime contract |
|---|---|---|
| `qwen_audio_3` | Model `qwen-audio-3.0-asr-flash`; DashScope multimodal-generation endpoint; `input_audio` Data URI; `format=wav`; `sample_rate=16000`; result at `output.text` | Model `qwen-audio-3.0-asr-flash-streaming`; `/api-ws/v1/inference`; `run-task` then wait for `task-started`; binary PCM frames; `finish-task`; collect final `result-generated` sentences until `task-finished` |
| `qwen3_legacy` | Model `qwen3-asr-flash`; system corpus + legacy `audio` content; `parameters.language`; result under `output.choices[0].message.content[0].text` | Model `qwen3-asr-flash-realtime`; `/api-ws/v1/realtime?model=...`; `session.update`; Base64 `input_audio_buffer.append`; `input_audio_buffer.commit`; legacy realtime events |

- Missing `qwen_profile` during Rust deserialization must resolve to `qwen_audio_3` through `#[serde(default)]`.
- `QwenAudio3` must use an explicit Serde wire name: `rename = "qwen_audio_3"`. Do not rely on `rename_all = "snake_case"` across a digit boundary because it produces `qwen_audio3`.
- Rust deserialization accepts `qwen_audio3` only as a migration alias, then serializes it back as the canonical `qwen_audio_3` value.
- Frontend normalization must map a missing or unknown runtime profile to `qwen_audio_3`, while preserving the explicit `qwen3_legacy` value.
- The frontend selector must expose exactly the two controlled profiles and persist a full `AsrConfig` snapshot through the existing config-save gateway.
- Latest HTTP and realtime requests render compiled pure hotwords as an inline `vocabulary` object with weight `4`. Auto language omits `language_hints`; Chinese mode sends `language_hints: ["zh"]`.
- Legacy requests preserve the old corpus and `language` payload shapes.
- The existing global DashScope domains remain valid for this slice. Adding workspace-specific domains or Workspace ID configuration is a separate change.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Persisted configuration has no `qwen_profile` | Load succeeds and selects `qwen_audio_3`. |
| Persisted configuration contains the previous Rust value `qwen_audio3` | Load succeeds as `QwenAudio3`; the next save writes canonical `qwen_audio_3`. |
| Frontend sends `qwen_audio_3` through `save_config` | Tauri deserialization succeeds; it must never report an unknown enum variant. |
| Frontend receives a missing/invalid runtime profile | Normalize to `qwen_audio_3` before validation or save. |
| User explicitly selects `qwen3_legacy` | Both HTTP and realtime paths use the old model and old protocol. |
| Latest HTTP response has no string `output.text` | Return a parse error that includes the response shape. |
| Latest realtime does not emit `task-started` within 10 seconds | Fail session creation with a task-start timeout. |
| Latest realtime emits `task-failed` | Return `error_code: error_message` to the session result channel. |
| Latest realtime emits `task-finished` without final text | Return `未收到转录结果`. |
| Latest realtime emits multiple final sentences | Concatenate them in event order and publish only after `task-finished`. |
| User wants to recover from a latest-model compatibility issue | User selects `qwen3_legacy`; do not silently make a second ASR request. |

### 5. Good/Base/Bad Cases

- Good: old configuration loads into the latest profile, the UI shows the latest option, and a save persists `qwen_profile: "qwen_audio_3"`.
- Good: the frontend value survives a JSON/Tauri round trip, and the temporary `qwen_audio3` value migrates to the same enum variant.
- Good: one profile selection controls both HTTP and realtime model/protocol pairs.
- Good: realtime audio is sent only after `task-started` and final text is emitted only after `task-finished`.
- Base: an explicit legacy selection behaves exactly like the pre-profile integration.
- Bad: changing only the model ID while leaving the Qwen3 request or WebSocket event shape unchanged.
- Bad: allowing free-form model IDs in configuration.
- Bad: falling back to Qwen3 automatically after a latest-profile request fails.
- Bad: publishing the first final sentence before the realtime task has finished.
- Bad: assuming Serde `snake_case` inserts an underscore before a trailing digit and therefore omitting an explicit wire rename.

### 6. Tests Required

- Rust config tests: missing profile defaults to `QwenAudio3`; `qwen_audio_3` round-trips through `AsrConfig`; `qwen_audio3` loads as a migration alias and serializes canonically; explicit legacy profile serializes as `qwen3_legacy`.
- Qwen HTTP tests: latest model/input/format/sample-rate/vocabulary contract, latest Chinese language hint, latest response parsing, and unchanged legacy request/response parsing.
- Qwen realtime tests: latest `run-task`, vocabulary, language hint, `finish-task`, final-sentence parsing, and task-failure parsing.
- Frontend runtime test: profile constants map to the exact HTTP/realtime model IDs; missing profile normalizes to latest; explicit legacy survives normalization.
- Frontend build/type-check: every `AsrConfig` construction includes or safely normalizes `qwen_profile`.
- Overall backend compilation: `cargo check --all-targets` must pass because the profile crosses config, startup, HTTP, and realtime paths.

### 7. Wrong vs Correct

#### Wrong

```rust
const MODEL: &str = "qwen-audio-3.0-asr-flash-streaming";
// Still sends session.update and Base64 input_audio_buffer.append events.
```

#### Correct

```rust
match profile {
    QwenAsrProfile::QwenAudio3 => {
        // /api-ws/v1/inference -> run-task -> binary PCM -> finish-task
    }
    QwenAsrProfile::Qwen3Legacy => {
        // /api-ws/v1/realtime -> session.update -> Base64 append -> commit
    }
}
```

#### Wrong — implicit digit-boundary naming

```rust
#[serde(rename_all = "snake_case")]
pub enum QwenAsrProfile {
    QwenAudio3, // Serializes as qwen_audio3, which breaks the frontend contract.
}
```

#### Correct — explicit canonical wire value

```rust
#[serde(rename_all = "snake_case")]
pub enum QwenAsrProfile {
    #[serde(rename = "qwen_audio_3", alias = "qwen_audio3")]
    QwenAudio3,
}
```
