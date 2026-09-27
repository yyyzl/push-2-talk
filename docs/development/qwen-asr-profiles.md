# Qwen ASR Profiles

> Executable cross-layer contracts for selecting and running Qwen Audio 3.1, Qwen Audio 3.0 or Qwen3 legacy ASR.

---

## Scenario: Latest-by-default Qwen ASR With Explicit Legacy Compatibility

### 1. Scope / Trigger

- Trigger: any change to `QwenAsrProfile`, `AsrConfig.qwen_profile`, Qwen model IDs, Qwen HTTP request/response parsing, Qwen realtime WebSocket events, or the frontend Qwen profile selector.
- A profile is a paired HTTP + realtime protocol choice. It is not an arbitrary model string.
- `qwen_audio_3_1` is the default for new and pre-profile configurations. Explicit `qwen_audio_3` (3.0) and `qwen3_legacy` selections must survive upgrades.
- Request-level automatic retry from the latest profile to the legacy profile is forbidden. It would double requests, hide protocol failures, and make billing/diagnostics ambiguous.

### 2. Signatures

Backend configuration:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QwenAsrProfile {
    #[default]
    #[serde(rename = "qwen_audio_3_1")]
    QwenAudio3_1,
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
export type QwenAsrProfile = "qwen_audio_3_1" | "qwen_audio_3" | "qwen3_legacy";

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
| `qwen_audio_3_1` | Model `qwen-audio-3.1-asr-flash`; same Audio 3.x HTTP protocol as 3.0 | Model `qwen-audio-3.1-asr-flash-streaming`; same Audio 3.x binary PCM protocol as 3.0 |
| `qwen_audio_3` | Model `qwen-audio-3.0-asr-flash`; DashScope multimodal-generation endpoint; `input_audio` Data URI; `format=wav`; `sample_rate=16000`; result at `output.text` | Model `qwen-audio-3.0-asr-flash-streaming`; `/api-ws/v1/inference`; `run-task` then wait for `task-started`; binary PCM frames; `finish-task`; collect final `result-generated` sentences until `task-finished` |
| `qwen3_legacy` | Model `qwen3-asr-flash`; system corpus + legacy `audio` content; `parameters.language`; result under `output.choices[0].message.content[0].text` | Model `qwen3-asr-flash-realtime`; `/api-ws/v1/realtime?model=...`; `session.update`; Base64 `input_audio_buffer.append`; `input_audio_buffer.commit`; legacy realtime events |

- Missing `qwen_profile` during Rust deserialization must resolve to `qwen_audio_3_1` through `#[serde(default)]`.
- `QwenAudio3` must use an explicit Serde wire name: `rename = "qwen_audio_3"`. Do not rely on `rename_all = "snake_case"` across a digit boundary because it produces `qwen_audio3`.
- Rust deserialization accepts `qwen_audio3` only as a migration alias, then serializes it back as the canonical `qwen_audio_3` value.
- Frontend `normalizeQwenAsrProfile(unknown)` must preserve all three valid values, map `qwen_audio3` to 3.0, and use `qwen_audio_3_1` only for missing/invalid input. Runtime config and localStorage migration must use the same function.
- The frontend selector must derive exactly three options from `QWEN_ASR_PROFILES` and persist a full `AsrConfig` snapshot through the existing config-save gateway.
- Backend `QwenAsrProfile::http_model()` and `realtime_model()` own the exact model IDs. Both Audio 3.x variants share transport branches; Qwen3 alone uses the legacy protocol.
- Latest HTTP and realtime requests render compiled pure hotwords as an inline `vocabulary` object with weight `4`. Auto language omits `language_hints`; Chinese mode sends `language_hints: ["zh"]`.
- Legacy requests preserve the old corpus and `language` payload shapes.
- The existing global DashScope domains remain valid for this slice. Adding workspace-specific domains or Workspace ID configuration is a separate change.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Persisted configuration has no `qwen_profile` | Load succeeds and selects `qwen_audio_3_1`. |
| Persisted configuration selects `qwen_audio_3` | Load, save and restart keep Audio 3.0 rather than changing it to the default. |
| Frontend sends `qwen_audio_3_1` through `save_config` | Explicit Serde rename accepts it and subsequent serialization preserves it. |
| Persisted configuration contains the previous Rust value `qwen_audio3` | Load succeeds as `QwenAudio3`; the next save writes canonical `qwen_audio_3`. |
| Frontend sends `qwen_audio_3` through `save_config` | Tauri deserialization succeeds; it must never report an unknown enum variant. |
| Frontend receives a missing/invalid runtime profile | Normalize to `qwen_audio_3_1` before validation or save; inherited object keys such as `toString` are invalid. |
| User explicitly selects `qwen3_legacy` | Both HTTP and realtime paths use the old model and old protocol. |
| Audio 3.x HTTP response has no string `output.text` | Accept documented `output.output.sentence.text`; if both are absent return a parse error. |
| Latest realtime does not emit `task-started` within 10 seconds | Fail session creation with a task-start timeout. |
| Latest realtime emits `task-failed` | Return `error_code: error_message` to the session result channel. |
| Latest realtime emits `task-finished` without final text | Return `未收到转录结果`. |
| Latest realtime emits multiple final sentences | Concatenate them in event order and publish only after `task-finished`. |
| User wants to recover from a latest-model compatibility issue | User selects `qwen3_legacy`; do not silently make a second ASR request. |

### 5. Good/Base/Bad Cases

- Good: a pre-profile configuration loads into 3.1; an explicit 3.0 choice remains 3.0 after normalization and save.
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

- Rust config tests: missing profile defaults to `QwenAudio3_1`; all three values round-trip through `AsrConfig`; `qwen_audio3` loads as a 3.0 migration alias and serializes canonically.
- Qwen HTTP tests: latest model/input/format/sample-rate/vocabulary contract, latest Chinese language hint, latest response parsing, and unchanged legacy request/response parsing.
- Qwen realtime tests: latest `run-task`, vocabulary, language hint, `finish-task`, final-sentence parsing, and task-failure parsing.
- Frontend runtime test: all three profile constants map to exact HTTP/realtime model IDs; missing profile normalizes to latest; every explicit choice survives normalization, including localStorage migration.
- Frontend build/type-check: every `AsrConfig` construction includes or safely normalizes `qwen_profile`.
- Overall backend compilation: `cargo check --all-targets` must pass because the profile crosses config, startup, HTTP, and realtime paths.

#### 真实 API 验证入口

`test_api` 复用正式的 `QwenASRClient` / `QwenRealtimeClient`，通过 `--qwen-profile` 和 `--mode http|realtime` 指定测试组合，默认 3.1 + HTTP。

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin test_api -- --asr qwen --qwen-profile qwen_audio_3_1 --mode realtime --file sample.wav
```

- 凭据只从进程环境 `DASHSCOPE_API_KEY` 读取；禁止写入命令行参数或测试报告。
- HTTP 调用 `transcribe_from_memory` 单次请求，不自动重试。完整验证限制 45 秒。
- 实时输入必须为 16 kHz 单声道 PCM16 WAV；每 100 ms 发送 1600 个采样，结束后提交并等待最终文本。
- JSON 输出包含实际 `model`、`elapsed_ms`、`text`。实时耗时含音频发送时间，不能直接与 HTTP 耗时作速度排名。
- 云端调用需要用户授权；合成音频只做兼容性冒烟，不证明真人、方言、噪声等场景的准确率。

### 7. Wrong vs Correct

#### Wrong

```rust
const MODEL: &str = "qwen-audio-3.0-asr-flash-streaming";
// Still sends session.update and Base64 input_audio_buffer.append events.
```

#### Correct

```rust
match profile {
    QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
        // /api-ws/v1/inference -> run-task -> binary PCM -> finish-task
    }
    QwenAsrProfile::Qwen3Legacy => {
        // /api-ws/v1/realtime -> session.update -> Base64 append -> commit
    }
}
```

#### Wrong — binary normalization after adding a third option

```typescript
profile === "qwen3_legacy" ? "qwen3_legacy" : DEFAULT_QWEN_ASR_PROFILE;
// This silently upgrades an explicit 3.0 selection to 3.1.
```

#### Correct

```typescript
qwen_profile: normalizeQwenAsrProfile(profile);
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
