# ASR Hotword Compilation

> Executable contracts for `src-tauri/src/personalization/hotword_compiler.rs` and ASR provider hotword payloads.

---

## Scenario: User Dictionary Compiles Into Provider Hotword Packs

### 1. Scope / Trigger

- Trigger: any change to ASR provider dictionary/hotword construction, `HotwordCompiler`, dictionary metadata parsing, or provider hotword limits.
- ASR hotword compilation is upstream of real recognition requests. A payload-shape regression can break formal dictation, assistant voice input, realtime mode, fallback, and race flows.
- This phase intentionally keeps provider payload shapes compatible while centralizing pure-word extraction, source priority, de-duplication, ranking, and provider limits.

### 2. Signatures

```rust
pub const QWEN_HTTP_MAX_HOTWORDS: usize = 50;
pub const QWEN_REALTIME_MAX_HOTWORDS: usize = 50;
pub const DOUBAO_HTTP_MAX_HOTWORDS: usize = 100;
pub const DOUBAO_REALTIME_MAX_HOTWORDS: usize = 100;

pub struct AsrHotwordPack {
    pub words: Vec<AsrHotword>,
    pub max_count: usize,
}

pub struct AsrHotword {
    pub text: String,
    pub weight: Option<i32>,
    pub source: HotwordSource,
    pub aliases: Vec<String>,
}

pub fn compile_user_dictionary_asr_pack(
    dictionary_entries: &[String],
    max_count: usize,
) -> AsrHotwordPack;

pub fn render_qwen_corpus_text(pack: &AsrHotwordPack) -> String;
pub fn render_doubao_hotwords(pack: &AsrHotwordPack) -> Vec<serde_json::Value>;
```

Provider helpers:

```rust
fn build_qwen_http_corpus_text(dictionary: &[String]) -> (usize, String);
fn build_input_audio_transcription(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
fn build_corpus_context(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
fn build_realtime_context_object(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
```

### 3. Contracts

- `compile_user_dictionary_asr_pack` must accept legacy and metadata dictionary entries:
  - `"word"`
  - `"word|auto"`
  - `"word|source|category"`
- Only the pure `word` segment may enter provider payloads.
- Empty pure words are skipped.
- Duplicate words are de-duplicated case-insensitively.
- Manual user entries outrank automatic entries when duplicates conflict.
- Output order is by descending weight, then first-seen order for equal weights.
- The pack must be truncated to the provider-specific `max_count`.
- Qwen HTTP and Qwen Realtime render `input_audio_transcription.corpus.text` / system corpus text as a `、`-joined string.
- Doubao HTTP and Doubao Realtime render hotwords as the existing legacy-compatible array shape: `{"word": "<pure word>"}`.
- `AsrHotword.weight` is retained for future provider formats, but this slice must not add `weight` to Doubao outbound JSON until provider compatibility is verified separately.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Entries contain `Claude Code|auto|product` and `Claude Code|manual|product` | One `Claude Code` hotword remains, with `ManualUser` source and higher weight. |
| Entries contain `Rust|auto|tool` | Provider payload contains `Rust`, not metadata. |
| Qwen Realtime receives more than 50 compiled words | Corpus contains exactly 50 words. |
| Doubao HTTP/Realtme receives more than 100 compiled words | Hotwords array contains exactly 100 objects. |
| Dictionary is empty | Qwen omits corpus text; Doubao omits the `hotwords` field while preserving dialog context. |
| Code-symbol term contains hyphen | Alias may be retained in the pack for future formats, but aliases do not enter current provider payloads. |

### 5. Good/Base/Bad Cases

- Good: all ASR providers consume the same compiled pure-word pack and differ only in rendering format.
- Base: with a small clean dictionary, outbound payloads are equivalent to the old direct `entries_to_words` behavior.
- Bad: Qwen sends metadata strings such as `Claude Code|manual|product`.
- Bad: Doubao payload switches from `{"word": "Claude Code"}` to a weighted object before compatibility is verified.
- Bad: one provider silently ignores the compiler and reimplements source priority or limits.

### 6. Tests Required

- HotwordCompiler unit test for de-duplication and manual-over-auto priority.
- HotwordCompiler unit test for provider limit truncation.
- HotwordCompiler unit test proving Qwen corpus text omits metadata.
- HotwordCompiler unit test proving Doubao JSON keeps the legacy `{"word": ...}` shape.
- Qwen HTTP/Realtme tests proving corpus size is limited by provider constants.
- Doubao HTTP/Realtme tests proving hotwords size is limited and contains no `weight`.
- Run `cargo check` after provider wiring changes because ASR modules are production runtime paths.

### 7. Wrong vs Correct

#### Wrong

```rust
let purified_words = entries_to_words(&self.dictionary);
let hotwords: Vec<_> = purified_words
    .iter()
    .map(|word| serde_json::json!({ "word": word }))
    .collect();
```

#### Correct

```rust
let pack = compile_user_dictionary_asr_pack(&self.dictionary, DOUBAO_REALTIME_MAX_HOTWORDS);
let hotwords = render_doubao_hotwords(&pack);
```
