# TNL Normalization

> Executable contracts for the Technical Normalization Layer (`src-tauri/src/tnl/`).

---

## Scenario: Exact Dictionary Words Beat Phonetic Correction

### 1. Scope / Trigger

- Trigger: any change to phonetic dictionary matching, candidate recall, or TNL diagnostics in `TnlEngine` / `FuzzyMatcher`.
- TNL is latency-sensitive and sits before normal dictation insertion, assistant processing, and optional LLM polishing.
- Local phonetic correction must be conservative: it may fix ASR near-misses, but it must not override a token that is already a valid dictionary word.

### 2. Signatures

Core APIs:

```rust
impl TnlEngine {
    pub fn normalize(&self, text: &str) -> NormalizationResult;
}

impl FuzzyMatcher {
    pub fn has_exact_dictionary_match(&self, text: &str) -> bool;
}
```

Diagnostic output:

```rust
pub struct TnlCandidate {
    pub original: String,
    pub target: String,
    pub source: TnlCandidateSource,
    pub decision: TnlCandidateDecision,
}
```

### 3. Contracts

- Before applying or collecting an English phonetic match, check whether the original span already matches a dictionary entry case-insensitively.
- If the original span is an exact dictionary match, preserve the original text and do not emit a phonetic candidate for the same span.
- Exact dictionary protection applies to both paths:
  - automatic local phonetic replacement
  - medium-confidence diagnostic candidate collection
- This protection must not disable legitimate ASR correction when the original span is not a dictionary word.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Dictionary contains `Grok` and `Groq`, input span is `Grok` | Keep `Grok`; no `Grok -> Groq` local replacement; no `Grok -> Groq` diagnostic candidate. |
| Dictionary contains only `Groq`, input span is an ASR near-miss | Existing phonetic matching may still apply or produce a candidate according to score/risk thresholds. |
| Dictionary contains `Claude Code`, input span is `Cloud Code` | Existing positive correction behavior remains available. |
| Original differs only by case from a dictionary word | Treat it as an exact dictionary match for protection. |

### 5. Good/Base/Bad Cases

- Good: `就像 Grok 以及 Claude 相关的内容` remains unchanged when the dictionary includes both `Grok` and `Groq`.
- Base: unrelated text with no phonetic candidate remains unchanged and emits no diagnostics.
- Bad: `Grok` is locally rewritten to `Groq` because both words sound similar.

### 6. Tests Required

- Regression test: dictionary includes `Grok`, `Groq`, and `Claude`; input containing `Grok` must normalize to the same text.
- Assert there is no `ReplacementReason::DictionaryPhonetic` replacement from `Grok` to `Groq`.
- Assert diagnostics do not contain a `TnlCandidate { original: "Grok", target: "Groq", ... }`.
- Run the broader TNL suite after this change because `apply_phonetic_replacement` feeds `TnlEngine.normalize`.

### 7. Wrong vs Correct

#### Wrong

```rust
if let Some(fuzzy_match) = matcher.try_phonetic_match_tokens(&subset) {
    result.push_str(&fuzzy_match.word);
}
```

#### Correct

```rust
let original = &text[first_token.start..last_token.end];
if matcher.has_exact_dictionary_match(original) {
    result.push_str(original);
    // Skip phonetic replacement/candidate for this exact dictionary word.
} else if let Some(fuzzy_match) = matcher.try_phonetic_match_tokens(&subset) {
    result.push_str(&fuzzy_match.word);
}
```

---

## Scenario: Mixed-Language Windows Must Not Use English-Only Phonetic Matches

### 1. Scope / Trigger

- Trigger: any change to syllable/window candidate generation, personalization correction pairs, or English phonetic matching over spans that may include Chinese and ASCII tokens.
- Mixed-language ASR text often contains surrounding Chinese context plus an English product/tool name. Window generation must not let the English part match a pair and then replace the whole mixed span.

### 2. Signatures

Candidate-generation APIs may differ, but they must expose token language/kind before deciding which key family to query:

```rust
enum TokenKind {
    Chinese,
    Ascii,
}

pub enum MatchKind {
    EnPhonetic,
    Mixed,
    Alias,
}
```

### 3. Contracts

- Pure ASCII windows may query English phonetic keys.
- Pure Chinese windows may query pinyin/fuzzy-pinyin keys.
- Mixed Chinese + ASCII windows must query only mixed keys and alias keys.
- A mixed window must not be replaced solely because its ASCII subset matches an English phonetic pair.
- If a shorter pure-ASCII sub-window matches, replace only that sub-window, preserving the surrounding Chinese context.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair `cloud code -> Claude Code`, input `我打开 cloud code` | Replace only `cloud code`, result `我打开 Claude Code`. |
| Pair `cloud code -> Claude Code`, input full window `我打开 cloud code` | Do not replace the full mixed span with `Claude Code`. |
| Pair has alias `kelaode\|code`, input `我打开 克劳德 code` | Mixed/alias window may replace `克劳德 code` with `Claude Code`. |
| Input `I use cloud storage` | No replacement from a `cloud code` pair. |

### 5. Good/Base/Bad Cases

- Good: `我打开 claud code` becomes `我打开 Claude Code` by replacing the pure-ASCII sub-window.
- Base: unrelated mixed text with no alias key remains unchanged.
- Bad: `我打开 cloud code` becomes only `Claude Code` because the full mixed window used the English phonetic key of `cloud code`.

### 6. Tests Required

- Unit test for the positive pure-ASCII sub-window correction inside Chinese context.
- Unit test for mixed alias correction such as `克劳德 code -> Claude Code`.
- Regression test asserting the full mixed span is not swallowed.
- False-positive guard for common English words such as `cloud storage`.

### 7. Wrong vs Correct

#### Wrong

```rust
// Wrong: this uses the English key even when the window contains Chinese.
for key in keys.en_phonetic_keys {
    lookup_by_en_phonetic(key);
}
```

#### Correct

```rust
if has_ascii && !has_chinese {
    lookup_by_en_phonetic(key);
} else if has_ascii && has_chinese {
    lookup_by_mixed_or_alias(key);
}
```

---

## Scenario: SyllableLattice Owns Local Window Key Generation

### 1. Scope / Trigger

- Trigger: any change to ASR text token windows, phonetic/alias key generation over windows, or personalization candidate generation.
- Window generation is shared infrastructure for the local IME-style second decoding path. It should not be reimplemented separately in each pass.

### 2. Signatures

Core APIs:

```rust
pub(crate) struct SyllableLattice {
    pub source_text: String,
    pub tokens: Vec<PhoneticToken>,
}

impl SyllableLattice {
    pub(crate) fn from_asr_text(text: &str) -> Self;
    pub(crate) fn windows(&self, max_size: usize) -> Vec<WindowKey>;
}

pub(crate) struct WindowKey {
    pub byte_range: Range<usize>,
    pub text: String,
    pub keys: PhoneticKeyBundle,
    pub has_chinese: bool,
    pub has_ascii: bool,
}
```

### 3. Contracts

- `SyllableLattice` is the single place that turns ASR text into content tokens and bounded token windows.
- Whitespace and pure symbol tokens are not emitted as content tokens, but window byte ranges may preserve separators between content tokens.
- Each `WindowKey` must include the original byte range, original window text, generated phonetic key bundle, and language flags.
- Window count must be bounded by `max_size`; `max_size = 0` returns no windows.
- Personalization and future ConvertPipeline passes should consume `SyllableLattice::windows()` instead of duplicating token-window loops.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Input `我打开 克劳德 code。` | Content tokens are `我打开`, `克劳德`, `code`; whitespace and punctuation are skipped. |
| Window text `克劳德 code` | Exposes mixed/alias keys such as `kelaode\|code`. |
| Window text `claud code` | Exposes English phonetic keys. |
| Six content tokens with `max_size = 3` | Emits a bounded sliding-window set; no window exceeds three content tokens. |

### 5. Tests Required

- Unit test for content-token extraction without whitespace or pure symbols.
- Unit test for mixed Chinese + ASCII alias keys.
- Unit test for pure ASCII phonetic keys.
- Unit test for bounded window count.

---

## Scenario: ASR Eval Reports Local Decode Latency

### 1. Scope / Trigger

- Trigger: any change to `src-tauri/src/bin/eval_asr.rs`, personalization conversion timing, personalization candidate diagnostics, or ASR personalization quality gates.
- The eval runner is the regression harness for the local second-decoding path. It must report accuracy, local processing latency, and candidate decision distribution so quality improvements do not hide performance regressions or silent decision drift.

### 2. Signatures

Eval entrypoint:

```rust
cargo run --bin eval_asr -- --suite tests/asr_eval/
cargo run --bin eval_asr -- --suite tests/asr_eval/ --diagnostics-out target/asr_eval_diagnostics
```

Latency summary helper:

```rust
struct LatencySummary {
    avg_ms: f64,
    p95_ms: f64,
}

fn summarize_latency_ms(values: &[f64]) -> LatencySummary;

struct CandidateDecisionCounts {
    total: usize,
    applied: usize,
    below_threshold: usize,
    skipped_overlap: usize,
    pending: usize,
}

struct MatchKindCounts {
    exact_text: usize,
    en_phonetic: usize,
    zh_pinyin_fuzzy: usize,
    mixed: usize,
    alias: usize,
}

fn evaluate_quality_gates(metrics: &EvalMetrics) -> QualityGateSummary;

fn write_diagnostics(results: &[CaseResult], output_dir: &Path) -> Result<PathBuf>;
```

### 3. Contracts

- Measure only local conversion time around `PersonalizationEngine::convert`; do not include ASR provider time, audio loading, or LLM calls.
- Report `avg_latency_ms` and `p95_latency_ms` in the top-level Markdown summary.
- Report `false_replacement_rate` alongside `false_replacement_count`.
- Report candidate decision totals: `candidates_total`, `applied_candidates`, `below_threshold_candidates`, `skipped_overlap_candidates`, and `pending_candidates`.
- Report candidate match-kind totals for all candidates and applied candidates: exact text, English phonetic, Chinese fuzzy pinyin, mixed, and alias.
- Report `quality_gate_passed`.
- Add per-case `Latency(ms)`, `Candidates`, and `Applied` to the result table for slow-case and decision inspection.
- Use nearest-rank p95 over sorted latency values. Empty input returns zeroed summary values.
- Keep latency formatting stable with millisecond precision to three decimals.
- `pending_candidates` should normally be zero after `PersonalizationEngine::convert`; a non-zero value indicates a candidate decision path was not finalized.
- Quality gates must fail when:
  - any case output mismatches expected text,
  - `correction_pair_hit_rate < 70%`,
  - `false_replacement_rate > 1%`,
  - `p95_latency_ms > 30ms`,
  - `pending_candidates > 0`.
- The report must be printed before returning a failing process status so CI/user runs can see the failing metrics.
- `--diagnostics-out <dir>` is optional. When set, eval must create `<dir>/asr_eval_diagnostics.json` after printing the report.
- Diagnostics payload must be bounded:
  - include `schema_version`,
  - include per-case `audio_id`, provider, category, pass/fail, raw/actual/expected text, local latency, candidate count, applied count, candidates, and applied candidates,
  - truncate all string fields recursively to a fixed character limit,
  - cap serialized candidate and applied-candidate lists per case,
  - do not serialize prompts, credentials, audio bytes, or unbounded user history.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Empty latency list | `avg_ms = 0`, `p95_ms = 0`. |
| Latencies `[2, 1, 4, 100]` | `avg_ms = 26.75`, `p95_ms = 100`. |
| Mini eval suite has 15-30 cases | Summary includes avg/p95 latency, decision totals, each row's local latency, candidate count, and applied count. |
| A case fails expected text comparison | Eval still prints latency report before returning failure. |
| Candidate decisions include applied, below-threshold, and skipped-overlap | Summary totals add each decision bucket independently. |
| Candidate sources include exact, English phonetic, and alias hits | Summary reports both total candidate count and applied count for each match kind. |
| p95 local latency exceeds 30ms | Print report, mark quality gate failed, then return an error. |
| pending candidates remain after conversion | Print report, mark quality gate failed, then return an error. |
| `--diagnostics-out target/asr_eval_diagnostics` is set | Create `target/asr_eval_diagnostics/asr_eval_diagnostics.json` with bounded per-case payload. |
| A candidate contains very long text | Serialized diagnostics truncate it without splitting Unicode code points. |

### 5. Tests Required

- Unit test for empty latency summary.
- Unit test for nearest-rank p95 calculation.
- Unit test for aggregating candidate decision counts across cases.
- Unit test for aggregating candidate and applied match-kind counts independently.
- Unit test for passing quality gates.
- Unit test for reporting all failed quality gates.
- Unit test for parsing `--diagnostics-out`.
- Unit test for bounded Unicode-safe string truncation.
- Unit test for bounded diagnostics export and candidate-list capping.
- Run `cargo run --bin eval_asr --no-default-features` after report-format changes.
- Run `cargo run --bin eval_asr --no-default-features -- --diagnostics-out target/asr_eval_diagnostics` after diagnostics-format changes.

---

## Scenario: Learned Single Common English Words Require Manual Confirmation

### 1. Scope / Trigger

- Trigger: any change to personalization correction-pair scoring, common-English-word protection, or automatic exact/phonetic/alias application thresholds.
- Common English words such as `code`, `open`, `use`, and `server` are high-risk when learned as single-word correction pairs. Personalization may add a small ASR-risk supplement such as `cloud` without changing existing TNL fuzzy behavior.

### 2. Signatures

Shared common-word guard:

```rust
pub(crate) fn is_common_english_word(word: &str) -> bool;

impl CorrectionPair {
    pub fn requires_manual_for_auto_apply(&self) -> bool;
}
```

Personalization scoring must route all automatic scores through the guard:

```rust
fn auto_score(pair: &CorrectionPair, score: f32) -> f32;
```

### 3. Contracts

- A non-manual correction pair whose `original_text` is exactly one common ASCII word must not auto-apply, even when the pair has high confidence.
- Manual single-word pairs may still auto-apply. Manual source is the explicit user override.
- Multi-word pairs are not blocked just because one token is common. `cloud code -> Claude Code` remains valid.
- Personalization must reuse the shared TNL common-word list as its base guard. A small personalization-only supplement is allowed for ASR-specific risky words when adding them to TNL would break existing dictionary phonetic behavior.
- This guard must apply to exact text, English phonetic, mixed, and alias scoring paths.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Learned pair `cloud -> Claude`, input `I use cloud storage` | Keep `cloud`; no automatic replacement. |
| Manual pair `cloud -> Claude`, input `I use cloud storage` | Replace according to the manual pair. |
| Learned pair `cloud code -> Claude Code`, input `我打开 cloud code` | Replace the phrase. |
| TNL common-word list misses a generally risky token | Add it to the shared TNL list with a regression test. |
| A token is risky only for learned correction pairs, but valid for existing TNL dictionary correction | Add it to the personalization supplement instead of the TNL list. |

### 5. Tests Required

- Personalization engine test: learned single common word does not auto-apply.
- Personalization engine test: manual single common word still auto-applies.
- Personalization engine test: learned multi-word phrase containing a common word still auto-applies.
- TNL fuzzy test: shared common-word list includes any generally guarded token.
- If the token is personalization-only, run the broader TNL suite to prove existing dictionary phonetic behavior is unchanged.

---

## Scenario: Personalization Candidate Conflicts Use Frequency-Weighted Ranking

### 1. Scope / Trigger

- Trigger: any change to personalization candidate scoring, candidate diagnostics, overlap selection, or correction-pair frequency updates.
- Multiple correction pairs may match the same ASR span after learning, manual imports, or stale one-off pairs. Selection must prefer the pair that best reflects repeated user behavior without weakening the auto-apply safety threshold.

### 2. Signatures

Candidate diagnostics expose two separate scores:

```rust
pub struct ConversionCandidate {
    pub score: f32,
    pub rank_score: f32,
    pub applied: bool,
    pub decision: CandidateDecision,
    pub blocked_by_pair_id: Option<String>,
}

pub enum CandidateDecision {
    Pending,
    Applied,
    BelowApplyThreshold,
    SkippedOverlap,
}
```

Core selection API:

```rust
impl PersonalizationEngine {
    pub fn convert(&self, text: &str) -> ConversionResult;
}
```

### 3. Contracts

- `score` is the candidate confidence used for the automatic application threshold.
- `rank_score` is used only for ordering candidates after generation. It may include `frequency`, accepted history, or other ranking signals.
- A candidate with `score < apply_threshold` must not auto-apply even when `rank_score` is high.
- When two candidates cover the same span length and overlap, prefer higher `rank_score`; use `score` only as a tie-breaker.
- Longer candidate windows still sort before shorter windows to preserve phrase-level corrections.
- By the time `convert()` returns diagnostics, every candidate should have a final decision:
  - `Applied` for selected replacements.
  - `BelowApplyThreshold` for candidates that failed the auto-apply threshold.
  - `SkippedOverlap` for candidates blocked by an already-selected overlapping candidate.
- `applied` remains as a compatibility boolean, but new diagnostics should use `decision` for explanation.
- `SkippedOverlap` candidates must include `blocked_by_pair_id` so logs can explain why a shorter or lower-ranked window was skipped.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair A `cloud code -> Cloud Code`, confidence `0.98`, frequency `1`; Pair B `cloud code -> Claude Code`, confidence `0.90`, frequency `10` | Apply Pair B because repeated user behavior wins after both pass threshold. |
| Pair confidence is below the apply threshold but frequency is very high | Do not apply; keep original text and expose only diagnostics. |
| Two candidates have the same rank score | Prefer higher `score`, then earlier start offset. |
| A longer phrase and a shorter sub-token both match | Prefer the longer phrase before rank comparison. |
| Candidate loses due to overlap | Mark `decision = SkippedOverlap` and set `blocked_by_pair_id`. |
| Candidate loses due to threshold | Mark `decision = BelowApplyThreshold` and leave `applied = false`. |

### 5. Tests Required

- Personalization engine test: repeated same-span pair beats one-off higher-confidence candidate.
- Personalization engine test: high frequency does not bypass the apply threshold.
- Personalization engine diagnostics test: skipped-overlap candidate records `blocked_by_pair_id`.
- Personalization engine diagnostics test: below-threshold candidate records `BelowApplyThreshold`.
- Run the personalization suite and ASR eval after ranking changes.

---

## Scenario: Corrected Text May Seed Cross-Language Product Aliases

### 1. Scope / Trigger

- Trigger: any change to `personalization/phonetic_keys.rs`, correction-pair key generation, or alias lookup behavior.
- Learned pairs must cover the common case where the user fixes an English product name once, but later ASR outputs a Chinese transliteration plus an English tail.

### 2. Signatures

Core key-generation API:

```rust
pub fn build_key_bundle(text: &str) -> PhoneticKeyBundle;

pub struct PhoneticKeyBundle {
    pub alias_keys: Vec<String>,
    pub mixed_keys: Vec<String>,
}
```

Correction-pair key hydration:

```rust
impl CorrectionPair {
    pub fn ensure_keys(&mut self);
}
```

### 3. Contracts

- `ensure_keys()` must derive lookup keys from both `original_text` and `corrected_text`.
- For corrected ASCII product names with an explicitly seeded pinyin alias, add cross-language alias keys. Example: `Claude Code` should add `kelaode|code` and `kelaode|KT`.
- Seeded aliases must be a tiny conservative table, not an automatic transliteration generator for every English word.
- Alias keys generated from corrected text are allowed because the correction pair is user-accepted, manual, imported, or otherwise already present in the personalization store.
- Existing Chinese+ASCII input key generation must continue producing aliases such as `kelaode|code`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Accepted pair `cloud code -> Claude Code` | Persist alias keys including `kelaode|code` and `kelaode|KT`. |
| Later ASR text is `我打开 克劳德 code` | Match the learned pair and output `我打开 Claude Code`. |
| Corrected text has no seeded product alias | Do not invent cross-language aliases. |
| Seed table is expanded | Add unit tests for generated aliases and run personalization eval. |

### 5. Tests Required

- Phonetic key test: `build_key_bundle("Claude Code")` contains `kelaode|code` and `kelaode|KT`.
- Store/engine test: accepted `cloud code -> Claude Code` reloads and corrects `克劳德 code`.

---

## Scenario: English Plural Near-Misses Share Phrase Phonetic Keys

### 1. Scope / Trigger

- Trigger: any change to `personalization/phonetic_keys.rs` English key generation, ASR eval cases for product/tool names, or correction-pair lookup by English phonetic key.
- ASR often pluralizes one token in a technical phrase (`types script`, `types scripts`) even though the user's correction pair is singular (`type script -> TypeScript`). The local second-decoding path should cover these small suffix variants without storing duplicate correction pairs.

### 2. Signatures

```rust
fn build_en_phonetic_keys(words: &[String]) -> Vec<String>;
fn singularize_ascii_word(word: &str) -> String;
```

### 3. Contracts

- Generate normal Double Metaphone keys first, then add a conservative singularized variant when it differs.
- Singularization may strip a trailing `s` for words longer than three ASCII characters.
- Singularization may convert `ies -> y` when the remaining stem has at least two characters.
- Do not strip `ss`, and do not singularize words with length `<= 3`.
- Keep keys deduplicated and stable in insertion order.
- This is a candidate-generation helper only; it must not bypass the personalization apply threshold or common-word guard.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair `type script -> TypeScript`, input `types script` | Input key set shares a key with the stored pair and can auto-apply if score passes threshold. |
| Pair `type script -> TypeScript`, input `types scripts` | Both plural tokens can be singularized and matched. |
| Word ends with `ss` | Do not strip the suffix. |
| Word length is `<= 3` | Do not singularize. |
| Input `Please type carefully` | No `TypeScript` replacement because the phrase key does not match. |

### 5. Tests Required

- Phonetic key unit test: `type script`, `types script`, and `types scripts` share at least one English phonetic key.
- ASR eval cases for `types script` and `types scripts`.
- False-positive guard for common `type` usage.
