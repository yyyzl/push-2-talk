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

pub fn compile_asr_pack_with_correction_pairs(
    dictionary_entries: &[String],
    correction_pairs: &[CorrectionPair],
    max_count: usize,
) -> AsrHotwordPack;

pub fn compile_tnl_dictionary_pack_with_pairs(
    dictionary_entries: &[String],
    correction_pairs: &[CorrectionPair],
) -> TnlDictionaryPack;

pub fn compile_llm_context_pack_with_pairs(
    dictionary_entries: &[String],
    correction_pairs: &[CorrectionPair],
    max_count: usize,
) -> LlmContextPack;

pub fn render_qwen_corpus_text(pack: &AsrHotwordPack) -> String;
pub fn render_doubao_hotwords(pack: &AsrHotwordPack) -> Vec<serde_json::Value>;
```

Provider helpers:

```rust
fn build_qwen_http_corpus_text(dictionary: &[String]) -> (usize, String);
fn build_qwen_http_corpus_text_with_pairs(
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> (usize, String);
fn build_input_audio_transcription(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
fn build_input_audio_transcription_with_pairs(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> serde_json::Value;
fn build_corpus_context(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
fn build_corpus_context_with_pairs(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> serde_json::Value;
fn build_realtime_context_object(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value;
fn build_realtime_context_object_with_pairs(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> serde_json::Value;
```

Runtime cache helpers:

```rust
fn load_asr_correction_pairs_or_empty() -> Vec<CorrectionPair>;
fn load_asr_correction_pairs_from_path_or_empty(path: &Path) -> Vec<CorrectionPair>;
fn refresh_asr_correction_pairs_runtime(state: &AppState) -> Vec<CorrectionPair>;
```

### 3. Contracts

- `compile_user_dictionary_asr_pack` must accept legacy and metadata dictionary entries:
  - `"word"`
  - `"word|auto"`
  - `"word|source|category"`
- Runtime dictionary entries may use source metadata beyond persisted user entries:
  - `"word|domain|domain_term"` for currently selected builtin domains.
  - `"word|builtin|domain_term"` for future low-priority builtin fallback terms.
  - `"word|recent|category"` for conservative frontend-derived terms from successful history records in the last 24 hours.
  - `"word|app_context|category"` for conservative backend-derived terms from the current target window UIA text.
- Only the pure `word` segment may enter provider payloads.
- Empty pure words are skipped.
- Duplicate words are de-duplicated case-insensitively.
- Manual user entries outrank automatic entries when duplicates conflict.
- Correction pair entries use `CorrectionPair.corrected_text` and rank below manual user entries but above automatic user entries.
- Runtime source priority must remain: manual user > correction pair > recent > app context > automatic user > active domain > builtin fallback.
- Frontend runtime dictionary construction must preserve source/category metadata for ASR ranking, while persisted config continues to store only user-managed dictionary entries.
- Frontend recent hotwords must be runtime-only. They may be included in `start_app` / `update_runtime_config` dictionary payloads, but must not be written to the `save_config` persisted `dictionary` field.
- Frontend runtime dictionary merge priority must be user dictionary > recent hotwords > selected builtin domain terms, with pure-word de-duplication before sending to the backend.
- Backend current-App context hotwords must be runtime-only. `start_app` may append `word|app_context|category` entries to the current recording's dictionary snapshot after reading the target window through UIA, but these entries must never be written to persisted config.
- Current-App context extraction must be conservative, bounded by input length and output count, skip URL/email-like sensitive tokens, and degrade to the original dictionary if UIA fails or returns empty text.
- Correction pairs must be included only when `enabled = true` and `corrected_text.trim()` is not empty.
- `CorrectionPair.original_text` and `alias_keys` may be retained as aliases/hints in packs, but they must not enter current provider payload text.
- If a manual dictionary word and a correction pair produce the same pure word, the manual dictionary source wins.
- Output order is by descending weight, then first-seen order for equal weights.
- The pack must be truncated to the provider-specific `max_count`.
- Qwen3 legacy HTTP and realtime render `input_audio_transcription.corpus.text` / system corpus text as a `、`-joined string.
- Qwen Audio 3.1 / 3.0 HTTP and realtime render the same compiled pure words as an inline `vocabulary` object with provider weight `4`; metadata and aliases must not enter the object keys.
- Doubao HTTP and Doubao Realtime render hotwords as the existing legacy-compatible array shape: `{"word": "<pure word>"}`.
- `AsrHotword.weight` is retained for future provider formats, but this slice must not add `weight` to Doubao outbound JSON until provider compatibility is verified separately.
- Runtime provider paths must consume an already-loaded correction-pair snapshot; they must not synchronously read `correction_pairs.json` while building request payloads or sending realtime audio chunks.
- `start_app` must refresh the ASR correction-pair cache before initializing HTTP ASR clients.
- `add_learned_word` must refresh the ASR correction-pair cache after a correction pair is accepted and persisted.
- Realtime ASR session creation must receive a cloned correction-pair snapshot from `AppState`, alongside the dictionary snapshot.
- Missing or invalid correction-pair storage must degrade to an empty pair list, log a warning for invalid loads, and never block recording.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Entries contain `Claude Code|auto|product` and `Claude Code|manual|product` | One `Claude Code` hotword remains, with `ManualUser` source and higher weight. |
| Entries contain `Rust|auto|tool` | Provider payload contains `Rust`, not metadata. |
| Entries contain `领域术语|domain|domain_term` | Provider payload contains `领域术语`, with `Domain` source ranked below automatic user words. |
| Runtime dictionary contains selected builtin domain words | They are sent as domain metadata entries, not as manual user entries. |
| Successful frontend history from the last 24h contains `Claude Code` | The next runtime dictionary refresh may include `Claude Code|recent|generic`, and provider payloads contain only `Claude Code`. |
| Frontend history contains failed or older-than-24h records | They are not converted into `recent` runtime hotwords. |
| Current target window UIA text contains `Claude Code` and `GPT-5.3-Codex` | The current recording dictionary snapshot may append `Claude Code|app_context|generic` and `GPT-5.3-Codex|app_context|code_symbol`; provider payloads contain only pure words. |
| Current target window UIA read fails, is empty, or only contains ordinary UI labels / URL / email tokens | No app-context hotwords are appended and recording still starts. |
| Correction pair `cloud code -> Claude Code`, no manual duplicate | `Claude Code` enters the combined ASR pack as `CorrectionPair`, with `cloud code` retained as an alias/hint. |
| Manual dictionary contains `Claude Code`, correction pair also corrects to `Claude Code` | The combined pack keeps the manual source. |
| Correction pair is disabled or corrected text is blank | It is skipped in ASR, TNL, and LLM packs. |
| `correction_pairs.json` is missing at service start | Runtime ASR correction-pair cache is empty and recording remains available. |
| `correction_pairs.json` is invalid at service start | Runtime ASR correction-pair cache is empty, a warning is logged, and recording remains available. |
| User accepts `winds surf -> Windsurf` through `add_learned_word` | The runtime ASR correction-pair cache refreshes so the next recording can send `Windsurf` upstream. |
| Qwen Realtime receives more than 50 compiled words | Corpus contains exactly 50 words. |
| Doubao HTTP/Realtme receives more than 100 compiled words | Hotwords array contains exactly 100 objects. |
| Dictionary is empty | Qwen3 realtime omits corpus text, Qwen Audio 3.1 / 3.0 omits `vocabulary`, and Doubao omits the `hotwords` field while preserving dialog context. |
| Code-symbol term contains hyphen | Alias may be retained in the pack for future formats, but aliases do not enter current provider payloads. |

### 5. Good/Base/Bad Cases

- Good: all ASR providers consume the same compiled pure-word pack and differ only in rendering format.
- Good: runtime ASR clients consume `compile_asr_pack_with_correction_pairs` through an `AppState` correction-pair snapshot that is refreshed on service start and accepted learning updates.
- Good: frontend derives recent hotwords from local successful history and refreshes runtime config without persisting those entries.
- Good: backend derives current-App context hotwords from the already captured target window handle and appends them only to the current recording snapshot.
- Base: with a small clean dictionary, outbound payloads are equivalent to the old direct `entries_to_words` behavior.
- Bad: Qwen sends metadata strings such as `Claude Code|manual|product`.
- Bad: `save_config` persists `Claude Code|recent|generic` into the user dictionary.
- Bad: UIA text content is logged verbatim or persisted as user dictionary data.
- Bad: provider runtime paths synchronously load `correction_pairs.json` on every audio chunk or request build without a cache strategy.
- Bad: Doubao payload switches from `{"word": "Claude Code"}` to a weighted object before compatibility is verified.
- Bad: one provider silently ignores the compiler and reimplements source priority or limits.

### 6. Tests Required

- HotwordCompiler unit test for de-duplication and manual-over-auto priority.
- HotwordCompiler unit test for provider limit truncation.
- HotwordCompiler unit test proving Qwen corpus text omits metadata.
- HotwordCompiler unit test proving Doubao JSON keeps the legacy `{"word": ...}` shape.
- HotwordCompiler unit test proving correction pairs rank below manual words and above auto words.
- HotwordCompiler unit test proving `recent` ranks below correction pairs and above `app_context` / automatic user words.
- Frontend runtime test proving recent hotwords come only from successful records within 24h and are formatted as `word|recent|category`.
- Frontend runtime-flow test proving recent hotwords participate in runtime refresh hashing and are excluded from persisted `save_config` dictionary.
- Backend app-context hotword tests proving conservative extraction, output limit, URL/email filtering, and runtime-only dictionary snapshot append.
- HotwordCompiler unit test proving disabled/blank correction pairs are skipped.
- TNL/LLM pack test proving correction pairs and correction hints are preserved for future consumers.
- Runtime cache tests proving missing correction-pair files load as an empty ASR hotword source and valid stores hydrate correction pairs.
- Provider tests proving Qwen3 HTTP/Realtime corpus and Qwen Audio 3.1 / 3.0 HTTP/Realtime vocabulary include correction-pair corrected text without original text or metadata.
- Provider tests proving Doubao HTTP/Realtme hotwords include correction-pair corrected text while preserving the legacy `{"word": ...}` shape.
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

#### Wrong

```rust
fn build_realtime_context_object(...) -> serde_json::Value {
    let pairs = CorrectionPairStore::load_json(default_correction_pairs_path()?)?;
    compile_asr_pack_with_correction_pairs(dictionary, pairs.pairs(), limit)
}
```

#### Correct

```rust
let correction_pairs = state.asr_correction_pairs.lock().unwrap().clone();
let context = build_realtime_context_object_with_pairs(
    language_mode,
    &dictionary,
    &correction_pairs,
);
```
