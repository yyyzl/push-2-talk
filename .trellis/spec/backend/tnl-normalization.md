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

## Scenario: Conservative Disfluency Cleaning Runs Before TNL Normalization

### 1. Scope / Trigger

- Trigger: any change to `src-tauri/src/tnl/disfluency.rs`, `TnlEngine::normalize`, or future TNL disfluency configuration.
- Disfluency cleaning is an early ASR text cleanup pass. It must remove obvious spoken fillers before technical tokenization, while preserving content words that merely share the same prefix.

### 2. Signatures

```rust
pub enum DisfluencyMode {
    Off,
    Conservative,
    Aggressive,
}

pub struct DisfluencyResult {
    pub text: String,
    pub changed: bool,
}

pub fn clean_disfluency(text: &str, mode: DisfluencyMode) -> DisfluencyResult;

impl TnlEngine {
    pub fn new_with_disfluency_mode(
        dictionary: Vec<String>,
        disfluency_mode: DisfluencyMode,
    ) -> Self;
}
```

### 3. Contracts

- `TnlEngine::new(dictionary)` uses `DisfluencyMode::Conservative` by default.
- Disfluency cleaning runs before Unicode normalization, tokenization, technical span detection, spoken-symbol mapping, pinyin, hyphen rewrite, and phonetic dictionary replacement.
- `DisfluencyMode::Off` must return the input unchanged.
- Conservative mode may remove only leading/isolated fillers:
  - filler chars: `嗯`, `啊`, `呃`, `唉`, `哎`, `诶`;
  - filler phrases: `这个`, `那个`, `就是说`, `怎么说呢`;
  - a filler is removable only when it is at the start after leading whitespace and followed by a separator, whitespace, or end-of-text.
- Conservative mode must not remove content words such as `这个东西` or multi-character interjections such as `嗯哼`.
- Aggressive mode additionally may collapse repeated CJK character runs of length at least 3 and remove simple start-repair fragments such as `我，那个，今天...`.
- Disfluency cleaning must not depend on dictionary content or LLM availability.
- Empty output is valid when input is only a filler; `NormalizationResult.changed` must still be true.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Mode `Off`, input `嗯，我准备好了` | Return unchanged text. |
| Conservative, input `嗯，我准备好了` | Return `我准备好了`. |
| Conservative, input `这个，我准备好了` | Return `我准备好了`. |
| Conservative, input `这个东西很重要` | Preserve text unchanged. |
| Conservative, input `嗯哼，我准备好了` | Preserve text unchanged. |
| Aggressive, input `我我我想打开设置` | Return `我想打开设置`. |
| Aggressive, input `嗯嗯嗯，我准备好了` | Return `我准备好了`. |
| Aggressive, input `我，那个，今天开会` | Return `今天开会`. |
| TNL with dictionary `Claude`, input `嗯，我最近学习了他们的那个标准产品 cloud` | Return `我最近学习了他们的那个标准产品 Claude`. |

### 5. Good/Base/Bad Cases

- Good: the normal dictation path removes obvious sentence-start fillers before technical correction.
- Base: natural text without leading fillers keeps the previous TNL behavior.
- Bad: `这个东西很重要` becomes `东西很重要`.
- Bad: disfluency cleaning runs after phonetic replacement and shifts already-collected offsets.

### 6. Tests Required

- Disfluency unit tests for Conservative removal and false-positive preservation.
- Disfluency unit tests for Off preserving text.
- Disfluency unit tests for Aggressive repeated-character and false-start cleanup.
- TNL engine integration test proving disfluency runs before phonetic dictionary replacement.
- Run the full TNL test suite, normal pipeline tests, assistant pipeline tests, `cargo check`, and ASR eval after changing this pass.

### 7. Wrong vs Correct

#### Wrong

```rust
let normalized = self.unicode_normalize(text);
let replaced = self.apply_phonetic_replacement(&normalized);
let cleaned = clean_disfluency(&replaced, DisfluencyMode::Conservative);
```

#### Correct

```rust
let cleaned = clean_disfluency(text, DisfluencyMode::Conservative).text;
let normalized = self.unicode_normalize(&cleaned);
let replaced = self.apply_phonetic_replacement(&normalized);
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
- A mixed correction pair must not be returned by pure-ASCII English phonetic lookup. Its ASCII tail is not enough evidence to rewrite an unrelated ASCII span.
- A mixed correction pair must not be returned by pure-Chinese fuzzy-pinyin lookup. Its Chinese head is not enough evidence to rewrite an unrelated Chinese span.
- If a shorter pure-ASCII sub-window matches, replace only that sub-window, preserving the surrounding Chinese context.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair `cloud code -> Claude Code`, input `我打开 cloud code` | Replace only `cloud code`, result `我打开 Claude Code`. |
| Pair `cloud code -> Claude Code`, input full window `我打开 cloud code` | Do not replace the full mixed span with `Claude Code`. |
| Pair has alias `kelaode\|code`, input `我打开 克劳德 code` | Mixed/alias window may replace `克劳德 code` with `Claude Code`. |
| Input `I use cloud storage` | No replacement from a `cloud code` pair. |
| Pair `欧喷 ai -> OpenAI`, input `enable ai mode` | Keep `ai`; mixed-language pair must not match the pure ASCII tail by English phonetic key. |
| Pair `欧喷 ai -> OpenAI`, input `我调用 欧盆 接口` | Keep `欧盆`; mixed-language pair must not match the pure Chinese head by fuzzy-pinyin key. |

### 5. Good/Base/Bad Cases

- Good: `我打开 claud code` becomes `我打开 Claude Code` by replacing the pure-ASCII sub-window.
- Base: unrelated mixed text with no alias key remains unchanged.
- Bad: `我打开 cloud code` becomes only `Claude Code` because the full mixed window used the English phonetic key of `cloud code`.

### 6. Tests Required

- Unit test for the positive pure-ASCII sub-window correction inside Chinese context.
- Unit test for mixed alias correction such as `克劳德 code -> Claude Code`.
- Regression test asserting the full mixed span is not swallowed.
- False-positive guard for common English words such as `cloud storage`.
- Regression test: mixed-language pair does not auto-apply to ASCII tail only.
- Regression test: mixed-language pair does not auto-apply to Chinese head only.

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
- Chinese content must be split into single-character syllable tokens so windows can start inside a continuous CJK run such as `我打开克劳德 code`.
- Windows may span whitespace and safe joiners such as `-`, but must not span sentence or phrase punctuation such as `。`, `，`, `.`, or `,`.
- Each `WindowKey` must include the original byte range, original window text, generated phonetic key bundle, and language flags.
- Window count must be bounded by `max_size`; `max_size = 0` returns no windows.
- Personalization and future ConvertPipeline passes should consume `SyllableLattice::windows()` instead of duplicating token-window loops.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Input `我打开 克劳德 code。` | Content tokens are `我`, `打`, `开`, `克`, `劳`, `德`, `code`; whitespace and punctuation are skipped. |
| Window text `克劳德 code` | Exposes mixed/alias keys such as `kelaode\|code`. |
| Input `我打开克劳德 code` | Windows include the inner span `克劳德 code` even though no whitespace precedes `克劳德`. |
| Input `先说 cloud。code 再继续` | No window is emitted for `cloud。code`; `cloud` and `code` remain separate windows. |
| Input `打开 cloud-code` | A safe joiner may preserve the phrase window `cloud-code`. |
| Window text `claud code` | Exposes English phonetic keys. |
| Six content tokens with `max_size = 3` | Emits a bounded sliding-window set; no window exceeds three content tokens. |

### 5. Tests Required

- Unit test for content-token extraction without whitespace or pure symbols.
- Unit test for mixed Chinese + ASCII alias keys.
- Unit test for mixed Chinese + ASCII alias keys when the Chinese product name is glued to preceding CJK text.
- Unit test that sentence punctuation blocks phrase windows and prevents cross-sentence correction.
- Unit test that safe joiners such as `-` may still form one window.
- Unit test for pure ASCII phonetic keys.
- Unit test for bounded window count.

---

## Scenario: Personalization Engine Pass Toggles Preserve Fallback Behavior

### 1. Scope / Trigger

- Trigger: any change to `PersonalizationEngine`, correction-pair candidate generation, or future ConvertPipeline pass wiring.
- The local IME-style decoder must remain decomposable: exact correction pairs represent the P1 fallback path, while phonetic, fuzzy-pinyin, mixed, and alias matching represent the P2 syllable-match path.

### 2. Signatures

```rust
pub struct PersonalizationEngineConfig {
    pub max_window_tokens: usize,
    pub apply_threshold: f32,
    pub enable_exact_text_pass: bool,
    pub enable_syllable_match_pass: bool,
}

pub struct PassDiagnostics {
    pub name: String,
    pub enabled: bool,
    pub elapsed_us: u64,
    pub candidate_count: usize,
    pub applied_count: usize,
}

impl PersonalizationEngine {
    pub fn with_config(store: CorrectionPairStore, config: PersonalizationEngineConfig) -> Self;
}
```

### 3. Contracts

- `PersonalizationEngine::new(store)` must preserve production defaults: exact text pass enabled, syllable-match pass enabled, `max_window_tokens = 5`, `apply_threshold = 0.88`.
- When `enable_exact_text_pass = true`, exact `original_text -> corrected_text` pairs may still apply even if syllable matching is disabled.
- When `enable_syllable_match_pass = false`, do not query English phonetic, Chinese fuzzy-pinyin, mixed, or alias keys.
- Disabled syllable matching should produce no phonetic/alias candidates for that pass, not just mark them below threshold.
- `ConversionDiagnostics.pass_summaries` must include one summary for `exact_text` and one for `syllable_match` on non-empty conversions, including disabled pass summaries with zero candidate/apply counts.
- Pass summaries are observability-only. They must not decide whether a candidate applies.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair `cloud code -> Claude Code`, input `我打开 cloud code`, syllable pass disabled | Exact text pass still rewrites to `我打开 Claude Code`. |
| Pair `cloud code -> Claude Code`, input `我打开 claud code`, syllable pass disabled | Keep `claud code`; no English phonetic candidate. |
| Pair has alias `kelaode\|code`, input `我打开 克劳德 code`, syllable pass disabled | Keep `克劳德 code`; no alias candidate. |
| Default config | Existing P1/P2 behavior remains enabled. |
| Default config, input `我打开 claud code` | `syllable_match` summary reports one candidate and one applied candidate. |

### 5. Tests Required

- Personalization engine test: disabling syllable-match pass keeps exact correction.
- Personalization engine test: disabling syllable-match pass skips English phonetic and alias candidates.
- Personalization engine test: pass summaries report enabled/disabled state, candidate count, and applied count.
- Run personalization suite and ASR eval after changing default config behavior.

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
cargo run --bin eval_asr -- --disable-syllable-match-pass --allow-quality-gate-failure
cargo run --bin eval_asr -- --apply-threshold 0.88 --max-window-tokens 5
cargo run --bin eval_asr -- --sweep-thresholds 0.70,0.88,0.99 --sweep-window-tokens 3,5 --allow-quality-gate-failure
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

struct PassSummaryCounts {
    enabled_cases: usize,
    disabled_cases: usize,
    candidate_count: usize,
    applied_count: usize,
    elapsed_us: u64,
}

fn evaluate_quality_gates(metrics: &EvalMetrics) -> QualityGateSummary;

struct EvalRunConfig {
    suite_dir: String,
    enable_exact_text_pass: bool,
    enable_syllable_match_pass: bool,
    apply_threshold: f32,
    max_window_tokens: usize,
    allow_quality_gate_failure: bool,
}

fn write_diagnostics(
    results: &[CaseResult],
    output_dir: &Path,
    run_config: &EvalRunConfig,
) -> Result<PathBuf>;
```

### 3. Contracts

- Measure only local conversion time around `PersonalizationEngine::convert`; do not include ASR provider time, audio loading, or LLM calls.
- Report `correction_pair_hit_rate`, `exact_text_hit_rate`, and `syllable_match_hit_rate`; pass hit rates are case-level rates based on whether at least one applied candidate from that pass exists in the case.
- Report `avg_latency_ms` and `p95_latency_ms` in the top-level Markdown summary.
- Report `false_replacement_rate` alongside `false_replacement_count`.
- Report candidate decision totals: `candidates_total`, `applied_candidates`, `below_threshold_candidates`, `skipped_overlap_candidates`, and `pending_candidates`.
- Report candidate match-kind totals for all candidates and applied candidates: exact text, English phonetic, Chinese fuzzy pinyin, mixed, and alias.
- Report pass summary totals for `exact_text` and `syllable_match`: enabled cases, disabled cases, candidate count, applied count, and elapsed microseconds.
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
- `--disable-syllable-match-pass` is an ablation-only eval flag. It must keep exact-text correction enabled while disabling English phonetic, Chinese fuzzy-pinyin, mixed, and alias candidate generation.
- `--disable-exact-text-pass` is an ablation-only eval flag. It must not change production defaults.
- `--allow-quality-gate-failure` may change only the process exit status for intentional ablation runs. It must still print `quality_gate_passed: false` and each `quality_gate_failure`.
- `--apply-threshold <float>` is an eval-only tuning flag. The value must be finite and within `0.0..=1.0`; it overrides `PersonalizationEngineConfig.apply_threshold` for that run only.
- `--max-window-tokens <usize>` is an eval-only tuning flag. The value must be within `1..=16`; it overrides `PersonalizationEngineConfig.max_window_tokens` for that run only.
- `--sweep-thresholds <csv>` and `--sweep-window-tokens <csv>` are eval-only tuning flags. They run the Cartesian product of threshold/window values and print a compact comparison table.
- Sweep mode must reuse the same quality-gate logic as normal eval. It fails the process when any row fails unless `--allow-quality-gate-failure` is set.
- Sweep mode must reject `--diagnostics-out` to avoid overwriting one diagnostics file with multiple configs.
- Eval tuning flags must not change production defaults exposed through `PersonalizationEngine::new(store)`.
- `--diagnostics-out <dir>` is optional. When set, eval must create `<dir>/asr_eval_diagnostics.json` after printing the report.
- Diagnostics payload must be bounded:
  - include `schema_version` (`4` after effective eval config export),
  - include top-level `eval_config` with suite path, effective apply threshold, max window tokens, pass enabled flags, and allow-failure flag,
  - include top-level `metrics` with the same aggregate rates/counts as the Markdown report,
  - include top-level `quality_gate` with `passed` and `failures`,
  - include per-case `audio_id`, provider, category, pass/fail, raw/actual/expected text, local latency, candidate count, applied count, candidates, and applied candidates,
  - include per-case `pass_summaries`,
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
| Candidate sources include exact, English phonetic, Chinese fuzzy-pinyin, mixed, and alias hits | Summary reports both total candidate count and applied count for each match kind. |
| Pass summaries include `exact_text` and `syllable_match` | Summary reports enabled/disabled case count, candidate count, applied count, and elapsed microseconds for each pass. |
| p95 local latency exceeds 30ms | Print report, mark quality gate failed, then return an error. |
| pending candidates remain after conversion | Print report, mark quality gate failed, then return an error. |
| `--disable-syllable-match-pass --allow-quality-gate-failure` is set | Print failed quality gates and return success for comparison scripts. |
| `--apply-threshold` is NaN, infinite, below 0, or above 1 | Reject before running eval. |
| `--max-window-tokens` is 0 or greater than 16 | Reject before running eval. |
| `--sweep-thresholds` contains an empty, NaN, infinite, below-0, or above-1 item | Reject before running eval. |
| `--sweep-window-tokens` contains an empty, 0, or greater-than-16 item | Reject before running eval. |
| Sweep mode uses `--diagnostics-out` | Reject before running eval. |
| `--diagnostics-out target/asr_eval_diagnostics` is set | Create `target/asr_eval_diagnostics/asr_eval_diagnostics.json` with bounded per-case payload. |
| A candidate contains very long text | Serialized diagnostics truncate it without splitting Unicode code points. |

### 5. Tests Required

- Unit test for empty latency summary.
- Unit test for nearest-rank p95 calculation.
- Unit test for aggregating candidate decision counts across cases.
- Unit test for aggregating candidate and applied match-kind counts independently.
- Unit test for aggregating pass summaries independently.
- Unit test for passing quality gates.
- Unit test for reporting all failed quality gates.
- Unit test for parsing `--diagnostics-out`.
- Unit test for parsing personalization pass toggles.
- Unit test for parsing and validating `--apply-threshold` and `--max-window-tokens`.
- Unit test for parsing and validating sweep threshold/window lists.
- Unit test that quality-gate override affects only exit success logic.
- Unit test for bounded Unicode-safe string truncation.
- Unit test for bounded diagnostics export and candidate-list capping.
- Unit test for diagnostics export schema version and `pass_summaries`.
- Unit test for diagnostics export effective eval config.
- Run `cargo run --bin eval_asr --no-default-features` after report-format changes.
- Run `cargo run --bin eval_asr --no-default-features -- --apply-threshold 0.88 --max-window-tokens 5` after eval tuning flag changes.
- Run `cargo run --bin eval_asr --no-default-features -- --sweep-thresholds 0.88,0.99 --sweep-window-tokens 3,5 --allow-quality-gate-failure` after sweep-format changes.
- Run `cargo run --bin eval_asr --no-default-features -- --disable-syllable-match-pass --allow-quality-gate-failure` after pass-toggle changes.
- Run `cargo run --bin eval_asr --no-default-features -- --diagnostics-out target/asr_eval_diagnostics` after diagnostics-format changes.

---

## Scenario: LLM Candidate Arbitration Applies Non-Overlapping Spans Only

### 1. Scope / Trigger

- Trigger: any change to `LlmPostProcessor::apply_candidate_arbitration_response`, TNL candidate IDs, or future `LlmArbiterPass` replacement application.
- LLM decisions are advisory. Local code remains responsible for safe span selection before mutating text.

### 2. Signatures

```rust
impl LlmPostProcessor {
    fn apply_candidate_arbitration_response(
        text: &str,
        diagnostics: TnlDiagnostics,
        response: &str,
        elapsed_ms: u64,
    ) -> Result<TnlCandidateArbitrationResult>;
}
```

### 3. Contracts

- Parse LLM decisions into candidate-level `AppliedLlm` / `RejectedLlm` decisions.
- After parsing all `apply` decisions, run a local non-overlap selector before editing text.
- Prefer longer accepted spans first; for equal span length, prefer higher candidate `score`; then earlier start offset.
- Any accepted candidate that overlaps an already-selected accepted candidate must be downgraded to `RejectedLlm` and record `llm_overlap_rejected` evidence.
- `TnlArbitrationSummary.applied_count` must count only candidates actually applied to text.
- `TnlArbitrationSummary.rejected_count` must include LLM rejections, missing decisions, and local overlap rejections.
- Replacement must be applied from right to left after non-overlap selection so byte offsets remain valid.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| LLM accepts `cloud code -> Claude Code` and overlapping `cloud -> Claude` | Apply only the longer phrase; reject the shorter candidate with `llm_overlap_rejected`. |
| LLM accepts two non-overlapping candidates | Apply both from right to left. |
| LLM omits a pending candidate decision | Mark that candidate `RejectedLlm` with `llm_missing_decision`. |
| More than `MAX_ARBITRATION_CANDIDATES` pending candidates exist | Extra candidates stay `SkippedLimit`; they are not applied even if response mentions them. |

### 5. Good/Base/Bad Cases

- Good: `cloud code` becomes exactly `Claude Code` when both phrase and sub-token candidates are accepted.
- Base: a single accepted candidate still updates text and diagnostics.
- Bad: applying overlapping accepted candidates produces corrupted text such as `Claudee Code`.

### 6. Tests Required

- LLM arbitration test: single accepted candidate updates text and marks `AppliedLlm`.
- LLM arbitration test: missing decision marks `RejectedLlm`.
- LLM arbitration test: overlapping accepted candidates keep only the longest/highest-ranked non-overlapping span and count the loser as rejected.
- Run the broader `llm_post_processor` tests after changing arbitration selection.

### 7. Wrong vs Correct

#### Wrong

```rust
for candidate in accepted {
    output.replace_range(candidate.start..candidate.end, &candidate.target);
}
```

#### Correct

```rust
let accepted = select_non_overlapping_accepted_candidates(diagnostics, accepted_indices);
// Helper returns accepted candidates sorted from right to left.
for candidate in accepted {
    output.replace_range(candidate.start..candidate.end, &candidate.target);
}
```

---

## Scenario: Medium-Confidence Personalization Candidates Enter LLM Arbitration

### 1. Scope / Trigger

- Trigger: any change to runtime personalization diagnostics, `NormalPipeline` candidate arbitration wiring, or `ConversionCandidate` threshold semantics.
- The local IME-style decoder should auto-apply only high-confidence correction pairs. Medium-confidence candidates should reuse the existing bounded LLM candidate arbiter instead of being silently dropped.

### 2. Signatures

```rust
pub(crate) fn personalization_candidates_to_tnl_diagnostics(
    conversion: &ConversionResult,
) -> Option<TnlDiagnostics>;

impl NormalPipeline {
    fn merge_tnl_diagnostics(
        existing: Option<TnlDiagnostics>,
        personalization: Option<TnlDiagnostics>,
    ) -> Option<TnlDiagnostics>;
}

pub enum TnlCandidateSource {
    PersonalizationCorrectionPair,
}
```

### 3. Contracts

- Only convert personalization candidates when `ConversionResult.changed == false`; if personalization already changed the text, do not forward stale byte offsets to the LLM arbiter.
- Only `CandidateDecision::BelowApplyThreshold` candidates are eligible for conversion.
- Candidates with `score >= 0.68` become `TnlCandidateDecision::PendingLlm` and `TnlCandidateRisk::Medium`.
- Candidates with `0.55 <= score < 0.68` become `TnlCandidateDecision::RejectedLocal` and `TnlCandidateRisk::High`; they stay diagnostic-only and do not trigger LLM arbitration.
- Candidates with `score < 0.55` are omitted from `TnlDiagnostics`.
- Converted candidates must use `TnlCandidateSource::PersonalizationCorrectionPair` and preserve original byte offsets, original text, target text, and score.
- Evidence must include the correction pair id and match kind so logs can trace why the candidate exists.
- `NormalPipeline` must merge TNL diagnostics and personalization diagnostics before calling `maybe_arbitrate_candidates`.
- Existing TNL candidates must remain first in the merged list; personalization candidates are appended.
- If dictionary enhancement is disabled or no LLM processor is configured, merged pending personalization candidates follow the same skip path as TNL pending candidates.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Learned pair candidate score `0.80`, no local apply | Export one `PendingLlm` personalization candidate. |
| Learned pair candidate score `0.60`, no local apply | Export one `RejectedLocal` personalization candidate; no pending arbitration. |
| Learned pair candidate score `0.40`, no local apply | Export no TNL candidate. |
| Personalization already applied a high-confidence candidate | Do not export remaining personalization candidates for arbitration. |
| TNL diagnostics and personalization diagnostics both exist | Merge candidates into one `TnlDiagnostics` before arbitration. |

### 5. Good/Base/Bad Cases

- Good: a borderline `cloud code -> Claude Code` pair can be LLM-arbitrated instead of ignored.
- Base: high-confidence local personalization still applies without LLM.
- Bad: a personalization candidate generated before a local replacement is forwarded after the text changed, causing invalid byte offsets.

### 6. Tests Required

- Personalization helper test: medium-confidence below-threshold candidate becomes `PendingLlm` with source `PersonalizationCorrectionPair`.
- Personalization helper test: low-confidence candidate becomes `RejectedLocal`.
- Normal pipeline helper test: TNL and personalization diagnostics merge without dropping either candidate list.
- Run normal pipeline, personalization, LLM post-processor, and ASR eval checks after changing arbitration wiring.

### 7. Wrong vs Correct

#### Wrong

```rust
let (text, personalization_changed) = maybe_apply_personalization(text);
maybe_arbitrate_candidates(processor, enabled, text, tnl_diagnostics).await;
```

#### Correct

```rust
let (text, changed, personalization_diagnostics) = maybe_apply_personalization(text);
let diagnostics = merge_tnl_diagnostics(tnl_diagnostics, personalization_diagnostics);
maybe_arbitrate_candidates(processor, enabled, text, diagnostics).await;
```

---

## Scenario: LLM Arbitration Feeds Back Into Learned Correction Pairs

### 1. Scope / Trigger

- Trigger: any change to personalization `TnlCandidate` evidence, LLM candidate arbitration decisions, or correction-pair confidence feedback.
- Medium-confidence personalization candidates are learned-pair hypotheses. Once LLM arbitration makes a real apply/reject decision, that weak signal should update the learned pair instead of being lost.

### 2. Signatures

```rust
impl CorrectionPairStore {
    pub fn record_llm_arbitration_feedback_json(
        path: impl AsRef<Path>,
        pair_id: &str,
        accepted: bool,
    ) -> Result<Option<CorrectionPair>>;

    pub fn record_llm_arbitration_feedback(
        &mut self,
        pair_id: &str,
        accepted: bool,
    ) -> Option<CorrectionPair>;
}

pub fn record_llm_arbitration_feedback_pair(
    pair_id: Option<&str>,
    accepted: bool,
) -> Result<Option<CorrectionPair>>;

pub(crate) fn personalization_arbitration_feedback_from_tnl(
    diagnostics: &TnlDiagnostics,
) -> Vec<PersonalizationArbitrationFeedback>;

pub(crate) fn record_personalization_arbitration_feedback_from_tnl(
    diagnostics: &TnlDiagnostics,
) -> Result<usize>;
```

### 3. Contracts

- Only candidates with `TnlCandidateSource::PersonalizationCorrectionPair` are eligible for feedback.
- Only real LLM decisions produce feedback:
  - `TnlCandidateDecision::AppliedLlm` -> accepted feedback.
  - `TnlCandidateDecision::RejectedLlm` -> rejected feedback.
- `RejectedLlm` candidates with `llm_missing_decision` or `llm_overlap_rejected` evidence are not real LLM reject decisions; they must not update the store.
- Skipped paths (`SkippedDisabled`, `SkippedNoProcessor`, `SkippedTimeout`, `SkippedError`, `SkippedLimit`) must not update the store because no LLM judgment happened.
- The pair id must come from candidate evidence with the `pair_id:<id>` prefix; candidates without a non-empty pair id are ignored.
- Feedback updates only existing enabled non-manual learned pairs by exact `pair.id`.
- Accepted LLM feedback increments `frequency`, increases confidence by `0.05`, and refreshes lifecycle timestamps.
- Rejected LLM feedback decreases confidence by `0.05` and refreshes lifecycle timestamps.
- LLM rejection is weak feedback: it must not increment the user reject streak or disable a pair by itself.
- Manual pairs and disabled pairs must not be mutated by LLM arbitration feedback.
- Runtime feedback persistence is best-effort. If loading or saving `correction_pairs.json` fails, `NormalPipeline` logs a warning and keeps insertion/polishing behavior unchanged.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Personalization candidate `AppliedLlm` with `pair_id:learned-claude` | Increment that pair's frequency and confidence by `0.05`. |
| Personalization candidate `RejectedLlm` with `pair_id:learned-claude` | Decrease that pair's confidence by `0.05`; keep `rejected_count` unchanged. |
| Dictionary phonetic candidate `AppliedLlm` with a fake `pair_id` evidence item | Ignore it. |
| Personalization candidate `RejectedLocal` | Ignore it; no LLM decision occurred. |
| Personalization candidate `RejectedLlm` because of `llm_missing_decision` | Ignore it. |
| Personalization candidate `RejectedLlm` because of `llm_overlap_rejected` | Ignore it. |
| Candidate has no `pair_id:` evidence | Ignore it. |
| Matching pair is manual or disabled | Return `None` and preserve pair state. |
| JSON store is missing | Return `Ok(None)` and keep runtime path unchanged. |
| JSON store is invalid | Return an error to the caller; runtime logs and continues. |

### 5. Good/Base/Bad Cases

- Good: LLM accepting `cloud code -> Claude Code` nudges the learned pair toward future local auto-apply.
- Base: LLM rejecting a candidate in one weak context slightly lowers confidence but does not disable the learned pair.
- Bad: dictionary/TNL candidates or skipped candidates accidentally mutate personalization storage.
- Bad: an invalid personalization JSON file prevents dictation insertion.

### 6. Tests Required

- Store test: LLM accept updates an enabled learned pair by id, increments frequency, and does not change `accepted_count` / `rejected_count`.
- Store test: LLM reject subtracts `0.05` without incrementing reject streak or disabling the pair.
- Store test: JSON feedback persists through `save_json`.
- Store test: manual, disabled, and missing pairs are ignored.
- Personalization helper test: feedback extraction includes only personalization `AppliedLlm` / real LLM-reject `RejectedLlm` candidates.
- Run personalization tests, normal pipeline tests, LLM post-processor tests, `cargo check`, and ASR eval after wiring feedback into `NormalPipeline`.

### 7. Wrong vs Correct

#### Wrong

```rust
for candidate in diagnostics.candidates {
    if candidate.decision == TnlCandidateDecision::RejectedLlm {
        record_rejected_correction_pair(Some(&candidate.original), Some(&candidate.target))?;
    }
}
```

#### Correct

```rust
for feedback in personalization_arbitration_feedback_from_tnl(&diagnostics) {
    record_llm_arbitration_feedback_pair(Some(&feedback.pair_id), feedback.accepted)?;
}
```

---

## Scenario: Runtime Personalization Applies Before Downstream LLM Work

### 1. Scope / Trigger

- Trigger: any change to runtime personalization application in `NormalPipeline`, `handle_assistant_mode`, shared personalization helpers, runtime diagnostic persistence, or `ConversionDiagnostics` serialization.
- Runtime personalization is the local IME-style second decoder. It must run on ASR text after TNL and before downstream LLM work in both dictation and AI assistant voice paths.
- Runtime diagnostics are for local inspection only. They must help debug the decoder without making dictation insertion or assistant LLM calls depend on file I/O success.

### 2. Signatures

```rust
pub struct PersonalizationRuntimeResult {
    pub text: String,
    pub changed: bool,
    pub conversion: ConversionResult,
    pub elapsed_us: u64,
}

pub fn apply_personalization_with_store(
    text: String,
    store: CorrectionPairStore,
) -> PersonalizationRuntimeResult;

pub fn apply_default_personalization(
    text: String,
) -> Result<Option<PersonalizationRuntimeResult>>;

pub fn write_runtime_diagnostic(
    source_text: &str,
    result: &PersonalizationRuntimeResult,
) -> Result<PathBuf>;

fn write_runtime_diagnostic_to_dir(
    output_dir: &Path,
    source_text: &str,
    result: &PersonalizationRuntimeResult,
    timestamp_ms: u128,
) -> Result<PathBuf>;

impl NormalPipeline {
    fn maybe_apply_personalization(text: String) -> (String, bool);
}

fn apply_assistant_personalization(text: String) -> (String, bool);
```

Runtime file shape:

```json
{
  "schema_version": 1,
  "stage": "personalization",
  "timestamp_ms": 0,
  "source_text": "...",
  "output_text": "...",
  "changed": true,
  "elapsed_us": 0,
  "candidate_count": 0,
  "applied_count": 0,
  "pass_summaries": [],
  "candidates": [],
  "applied": []
}
```

### 3. Contracts

- `apply_personalization_with_store` is the shared runtime entry point that creates `PersonalizationEngine`, runs `convert`, and returns the changed text plus conversion diagnostics and local elapsed time.
- `apply_default_personalization` loads `%APPDATA%\PushToTalk\personalization\correction_pairs.json` when present; if the file is missing it returns `Ok(None)` and the caller keeps the text unchanged.
- `write_runtime_diagnostic` is the shared diagnostic writer; dictation and AI assistant callers must use it instead of duplicating payload truncation, filename, and pruning logic.
- Normal dictation must run shared personalization after TNL and before TNL candidate LLM arbitration / final polishing when TNL is enabled.
- AI assistant voice mode must run shared personalization after TNL and before `assistant_turn_pending`, usage stats, conversation history insertion, or `AssistantProcessor::process_turn`.
- If TNL is disabled, runtime personalization remains disabled for parity with the existing dictation path.
- Runtime personalization diagnostics must be written under `%APPDATA%\PushToTalk\diagnostics\YYYY-MM-DD\`.
- File names must be unique per run, using the `personalization-<timestamp>-<uuid>.json` pattern.
- Diagnostic payloads must truncate string fields recursively and cap candidate/applied candidate lists.
- Each daily diagnostics directory must keep at most 200 `personalization-*.json` files by pruning the oldest personalization files after a successful write.
- Diagnostic persistence must be best-effort: if directory creation, serialization, or file write fails, log a warning and keep the decoded text path unchanged.
- Diagnostic pruning must not delete unrelated diagnostic files in the same directory.
- Diagnostics must not include credentials, prompts, audio bytes, or unbounded user history.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| `correction_pairs.json` is missing | Skip personalization and write no personalization diagnostic. |
| Assistant ASR text `我打开 cloud code` and store has `cloud code -> Claude Code` | The pending user instruction and LLM input use `我打开 Claude Code`. |
| Assistant personalization store load fails | Log a warning and continue with the TNL-normalized instruction. |
| TNL is disabled | Assistant and dictation skip runtime personalization. |
| Dictation or assistant personalization runs and changes text | Write a bounded diagnostic with `changed = true`, pass summaries, candidate count, and applied count. |
| Dictation or assistant personalization runs and finds no candidate | Write a bounded diagnostic with `changed = false` and zero applied candidates. |
| A transcript or candidate contains very long text | Truncate strings without splitting Unicode code points. |
| More than 20 candidates exist | Persist only the first bounded candidate entries while preserving full `candidate_count`. |
| More than 200 personalization diagnostics exist for one UTC day | After writing the new file, keep the newest 200 personalization diagnostics and remove older `personalization-*.json` files. |
| The directory also contains non-personalization diagnostic files | Leave unrelated diagnostic files untouched. |
| Diagnostic write fails | Log a warning and continue insertion/LLM processing with the personalization result. |

### 5. Good/Base/Bad Cases

- Good: `cloud code -> Claude Code` changes both dictation text and assistant voice instructions before any downstream LLM call.
- Base: unrelated text with a loaded pair store writes a no-change diagnostic that remains small.
- Bad: assistant sends `cloud code` to the LLM even though the same correction pair already fixes normal dictation.
- Bad: dictation fails or blocks because the diagnostics directory cannot be created.

### 6. Tests Required

- Unit test for `YYYY-MM-DD` diagnostic directory formatting from a fixed Unix timestamp.
- Unit test for bounded personalization diagnostic JSON: schema version, stage, elapsed time, candidate cap, pass summaries, and truncated text.
- Unit test for pruning old runtime personalization diagnostics while preserving unrelated diagnostic files.
- Unit test for shared runtime helper returning changed text, conversion diagnostics, and elapsed time.
- Unit test for assistant personalization helper correcting a known pair before conversation processing.
- Run normal pipeline tests after changing runtime personalization diagnostics.
- Run personalization tests after changing shared runtime helpers.
- Run `cargo check --no-default-features`.

### 7. Wrong vs Correct

#### Wrong

```rust
write_runtime_diagnostic(&source, &result)?;
```

#### Correct

```rust
if let Err(e) = write_runtime_diagnostic(&source, &result) {
    tracing::warn!("write failed, keep dictation path: {}", e);
}
```

---

## Scenario: CorrectionPairStore JSON Writes Are Atomic

### 1. Scope / Trigger

- Trigger: any change to `CorrectionPairStore::save_json`, accepted correction persistence, reject feedback persistence, or the JSON MVP store path.
- The JSON store is the first persistent personalization layer. It must not be corrupted by a process exit or partial write while the user accepts or rejects a learned correction.

### 2. Signatures

```rust
impl CorrectionPairStore {
    pub fn save_json(&self, path: impl AsRef<Path>) -> Result<()>;
    pub fn upsert_accepted_correction_json(...) -> Result<Option<CorrectionPair>>;
    pub fn record_rejected_correction_json(...) -> Result<Option<CorrectionPair>>;
}
```

File paths:

```text
%APPDATA%\PushToTalk\personalization\correction_pairs.json
%APPDATA%\PushToTalk\personalization\correction_pairs.json.tmp
%APPDATA%\PushToTalk\personalization\correction_pairs.json.bak
```

### 3. Contracts

- `save_json` must write JSON to a same-directory temporary file first, then rename files into place.
- If the target file exists, move it to `.bak` before moving `.tmp` into the target path.
- After a successful save, remove `.bak` and `.tmp` leftovers.
- If the final rename fails after creating `.bak`, best-effort restore `.bak` to the target path.
- `upsert_accepted_correction_json` and `record_rejected_correction_json` must persist only through `save_json`.
- Loading invalid JSON must still return an error instead of overwriting unknown/corrupt content with an empty store.
- JSON `CorrectionPair` lifecycle metadata is additive and backward-compatible: `created_at`, `updated_at`, and `last_seen_at` are optional when loading older files.
- Newly created correction pairs should set all three lifecycle timestamps. Accept/reject feedback must refresh `updated_at` and `last_seen_at` while preserving an existing `created_at`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Parent directory is missing | Create it before writing the temp file. |
| Target file exists and a stale `.bak` exists | Remove stale `.bak`, replace target atomically, and leave no `.bak` or `.tmp` on success. |
| Target file does not exist | Write temp file, rename into target, and leave no temp file on success. |
| Final rename fails after backup creation | Attempt to restore `.bak` to the target path and return the original error. |
| Existing file contains invalid JSON | Return parse error before saving; do not overwrite the file. |

### 5. Good/Base/Bad Cases

- Good: accepting `cloud code -> Claude Code` creates or replaces `correction_pairs.json` without leftover temp files.
- Base: an unchanged/identity correction does not create a JSON file.
- Bad: direct `fs::write(correction_pairs.json, ...)` truncates the store before serialization/write fully succeeds.

### 6. Tests Required

- Unit test: saving over an existing file with a stale `.bak` reloads the new pair and removes `.bak` / `.tmp`.
- Unit tests: accepted and rejected correction feedback preserve/persist lifecycle timestamps.
- Existing accepted/rejected persistence tests must continue to pass.
- Run personalization tests and `cargo check --no-default-features`.

### 7. Wrong vs Correct

#### Wrong

```rust
fs::write(path, serde_json::to_string_pretty(&pairs)?)?;
```

#### Correct

```rust
fs::write(&temp_path, content)?;
fs::rename(&temp_path, path)?;
```

---

## Scenario: Accepted Learned Pure Chinese Pairs Stay Phonetic-Compatible

### 1. Scope / Trigger

- Trigger: any change to `CorrectionPairStore::upsert_accepted_correction`, learning-accept persistence, or validation before writing learned correction pairs.
- Learned correction pairs come from observed user edits. The store must reject obviously non-ASR-style pure Chinese rewrites before they can participate in exact, fuzzy-pinyin, or alias matching.

### 2. Signatures

```rust
impl CorrectionPairStore {
    pub fn upsert_accepted_correction(
        &mut self,
        original_text: &str,
        corrected_text: &str,
        category: Option<&str>,
        surrounding_context: Option<&str>,
    ) -> Option<CorrectionPair>;
}
```

### 3. Contracts

- Empty, identity, or normalized-identity corrections still return `None`.
- When both `original_text` and `corrected_text` are pure CJK text, accepted learning must require equal character length.
- When both sides are pure CJK text, accepted learning must require compatible fuzzy-pinyin keys.
- Non-CJK, ASCII phrase, and mixed Chinese+ASCII corrections are not forced to be equal length by this guard.
- This validation applies to learned accepted corrections. Existing manual/imported pairs remain explicit user data and are not rewritten by this helper.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Learned accept `狗 -> 猫咪` | Return `None`; do not store the pair. |
| Learned accept `狗 -> 猫` | Return `None`; do not store the pair. |
| Learned accept `麻 -> 吗` | Store the pair because length and fuzzy-pinyin key are compatible. |
| Learned accept `cloud code -> Claude Code` | Store the pair; English phrase length is not constrained. |
| Learned accept `欧喷 ai -> OpenAI` | Store the pair; mixed Chinese+ASCII length is not constrained. |

### 5. Good/Base/Bad Cases

- Good: `麻 -> 吗` can be learned but remains guarded from auto-apply unless manual.
- Base: `cloud code -> Claude Code` keeps the existing learned-pair path.
- Bad: a non-phonetic pure Chinese rewrite such as `狗 -> 猫` enters the pair store and later becomes an automatic candidate.

### 6. Tests Required

- Store test: pure Chinese length mismatch returns `None` and leaves lookup empty.
- Store test: pure Chinese fuzzy-pinyin mismatch returns `None` and leaves lookup empty.
- Store test: pure Chinese equal-length fuzzy-compatible pair is accepted.
- Run the personalization suite after this change.

### 7. Wrong vs Correct

#### Wrong

```rust
let pair = CorrectionPair::new(id, original_text, corrected_text);
self.pairs.push(pair);
```

#### Correct

```rust
if !is_valid_learned_correction_pair(original_text, corrected_text) {
    return None;
}
```

---

## Scenario: Learned Single Common English Words and Single Chinese Characters Require Manual Confirmation

### 1. Scope / Trigger

- Trigger: any change to personalization correction-pair scoring, common-English-word protection, single-character Chinese protection, or automatic exact/phonetic/alias application thresholds.
- Common English words such as `code`, `open`, `use`, and `server` are high-risk when learned as single-word correction pairs. Single Chinese characters are also high-risk because they often appear inside longer words. Personalization may add a small ASR-risk supplement such as `cloud` without changing existing TNL fuzzy behavior.

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
- A non-manual correction pair whose `original_text` is exactly one CJK character must not auto-apply, even when the pair has high confidence.
- Manual single-word or single-character pairs may still auto-apply. Manual source is the explicit user override.
- Accepted-learning updates must not overwrite or reformat an existing manual correction pair for the same original text.
- Reject feedback from learning suggestions must not lower confidence, increment reject counters, or disable manual correction pairs.
- Accepted learning feedback for a previously rejected learned pair must re-enable the pair and reset the consecutive reject streak.
- Multi-word pairs are not blocked just because one token is common. `cloud code -> Claude Code` remains valid.
- Personalization must reuse the shared TNL common-word list as its base guard. A small personalization-only supplement is allowed for ASR-specific risky words when adding them to TNL would break existing dictionary phonetic behavior.
- This guard must apply to exact text, English phonetic, mixed, and alias scoring paths.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Learned pair `cloud -> Claude`, input `I use cloud storage` | Keep `cloud`; no automatic replacement. |
| Manual pair `cloud -> Claude`, input `I use cloud storage` | Replace according to the manual pair. |
| Manual pair `cloud code -> Claude Code`, later accepted learning says `cloud code -> Cloud IDE` | Preserve the manual pair and continue applying `Claude Code`. |
| Manual pair `cloud code -> Claude Code`, later accepted learning says `cloud code -> claude code` | Preserve manual casing, id, category, counters, keys, and conversion behavior. |
| Manual pair `cloud code -> Claude Code`, learning dismiss sends the same original/corrected text | Preserve confidence, reject count, enabled state, and conversion behavior. |
| Learned pair `cloud code -> Claude Code`, input `我打开 cloud code` | Replace the phrase. |
| Learned pair `麻 -> 吗`, input `麻烦打开设置` | Keep `麻烦打开设置`; no automatic replacement. |
| Manual pair `麻 -> 吗`, input `麻烦打开设置` | Replace according to the manual pair. |
| Learned pair `cloud code -> Claude Code` is disabled by repeated rejects, then accepted again | Restore exact conversion and reset `rejected_count` to 0. |
| TNL common-word list misses a generally risky token | Add it to the shared TNL list with a regression test. |
| A token is risky only for learned correction pairs, but valid for existing TNL dictionary correction | Add it to the personalization supplement instead of the TNL list. |

### 5. Tests Required

- Personalization engine test: learned single common word does not auto-apply.
- Personalization engine test: manual single common word still auto-applies.
- Personalization engine/storage test: learned single Chinese character does not auto-apply.
- Personalization engine test: manual single Chinese character still auto-applies.
- Personalization storage/engine test: accepted learning does not overwrite or reformat an existing manual pair.
- Personalization storage/engine test: reject feedback does not weaken an existing manual pair.
- Personalization storage/engine test: accepted learning clears prior reject streak for a learned pair.
- Personalization engine test: learned multi-word phrase containing a common word still auto-applies.
- TNL fuzzy test: shared common-word list includes any generally guarded token.
- If the token is personalization-only, run the broader TNL suite to prove existing dictionary phonetic behavior is unchanged.

---

## Scenario: Re-Observed Learned Correction Pairs Get Lightweight Positive Feedback

### 1. Scope / Trigger

- Trigger: any change to `CorrectionPairStore::record_observed_correction`, learning observation routing, correction-pair feedback persistence, or repeated learning suggestion emission.
- Re-observation means the learning observer has again seen the same `original -> corrected` edit for an already saved learned pair.

### 2. Signatures

```rust
impl CorrectionPairStore {
    pub fn record_observed_correction_json(...) -> Result<Option<CorrectionPair>>;
    pub fn record_observed_correction(
        &mut self,
        original_text: &str,
        corrected_text: &str,
    ) -> Option<CorrectionPair>;
}

pub fn record_observed_correction_pair(
    original_text: Option<&str>,
    corrected_text: Option<&str>,
) -> Result<Option<CorrectionPair>>;
```

### 3. Contracts

- Re-observation must update only an existing enabled non-manual pair whose normalized `original_text` and `corrected_text` match the observed edit.
- Re-observation must not create a new pair.
- Re-observation must increment `frequency`, increase confidence by `0.05` up to `1.0`, and refresh `updated_at` / `last_seen_at`.
- Re-observation must not increment `accepted_count`; it is weaker than explicit user acceptance.
- Re-observation must not mutate manual pairs.
- Re-observation must not re-enable disabled learned pairs; only explicit acceptance can clear a disabled/reject streak.
- In the learning observer, if the target word already exists in the dictionary and the same correction pair is re-observed, record feedback and skip emitting a duplicate learning Toast.
- If feedback persistence fails, log a warning and continue with the normal suggestion flow.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Existing learned pair `cloud code -> Claude Code`, observed again | Increment `frequency`, add `0.05` confidence, persist timestamps. |
| No matching pair exists | Return `None`; do not create JSON content. |
| Matching pair is manual | Return `None`; preserve manual counters and confidence. |
| Matching learned pair is disabled | Return `None`; do not re-enable it. |
| JSON file is missing | Return `Ok(None)`. |
| JSON file is invalid | Return an error; caller logs and may still emit a suggestion. |

### 5. Good/Base/Bad Cases

- Good: a repeated `cloud code -> Claude Code` edit strengthens the existing learned pair and does not show another Toast.
- Base: first-time useful correction still emits the existing learning suggestion.
- Bad: observing a previously rejected/disabled pair silently re-enables it without explicit user acceptance.

### 6. Tests Required

- Store test: re-observation strengthens an enabled learned pair without changing `accepted_count`.
- Store test: JSON re-observation persists `frequency`, confidence, and lifecycle timestamps.
- Store test: re-observation ignores manual and disabled pairs.
- Learning routing tests should continue to prove first-time existing-dictionary corrections still emit when no pair is saved yet.

### 7. Wrong vs Correct

#### Wrong

```rust
// Wrong: creates or re-enables pairs from passive observation.
store.upsert_accepted_correction(original, corrected, category, context);
```

#### Correct

```rust
if let Ok(Some(pair)) = record_observed_correction_pair(Some(original), Some(corrected)) {
    tracing::info!("observed existing pair {}", pair.id);
}
```

---

## Scenario: User Reverts Learned Correction Pairs As Strong Negative Feedback

### 1. Scope / Trigger

- Trigger: any change to `CorrectionPairStore::record_reverted_correction`, learning observation routing, or feedback deltas for learned correction pairs.
- Reversion means the observer sees the reverse edit of an existing learned pair: the inserted/corrected target is changed back to the pair's original ASR text.

### 2. Signatures

```rust
impl CorrectionPairStore {
    pub fn record_reverted_correction_json(...) -> Result<Option<CorrectionPair>>;
    pub fn record_reverted_correction(
        &mut self,
        original_text: &str,
        corrected_text: &str,
    ) -> Option<CorrectionPair>;
}

pub fn record_reverted_correction_pair(
    original_text: Option<&str>,
    corrected_text: Option<&str>,
) -> Result<Option<CorrectionPair>>;
```

### 3. Contracts

- Reversion must match an enabled non-manual pair in reverse: observed `original_text` equals pair `corrected_text`, and observed `corrected_text` equals pair `original_text` after surface normalization.
- Reversion must not create a new pair.
- Reversion must decrement confidence by `0.30`, increment `rejected_count`, refresh `updated_at` / `last_seen_at`, and disable the pair after the existing reject streak threshold.
- Reversion must not mutate manual pairs.
- Reversion must not re-enable disabled learned pairs.
- The learning observer should record a successful reversion before calling the LLM judge and should not emit a learning Toast for that diff.
- If reversion persistence fails, log a warning and continue with the normal learning flow.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Pair `cloud code -> Claude Code`, observed `Claude Code -> cloud code` | Decrease confidence by `0.30`, increment `rejected_count`, persist timestamps. |
| Matching pair is manual | Return `None`; preserve manual counters and confidence. |
| Matching learned pair is disabled | Return `None`; do not re-enable it. |
| No reverse matching pair exists | Return `None`; continue normal learning flow. |
| JSON file is missing | Return `Ok(None)`. |
| JSON file is invalid | Return an error; caller logs and may still continue normal learning flow. |

### 5. Good/Base/Bad Cases

- Good: a user changes `Claude Code` back to `cloud code`, so the learned `cloud code -> Claude Code` pair is strongly penalized.
- Base: a normal first-time edit still reaches the LLM learning judge.
- Bad: a manual correction pair is weakened by passive observation.

### 6. Tests Required

- Store test: reverse edit penalizes an enabled learned pair by `0.30`.
- Store test: JSON reverse edit persists confidence and reject counters.
- Store test: reverse edit ignores manual and disabled pairs.
- Learning routing tests should continue to pass so first-time corrections still emit suggestions.

### 7. Wrong vs Correct

#### Wrong

```rust
// Wrong: treats a user reverting our correction as a new positive correction.
store.upsert_accepted_correction("Claude Code", "cloud code", category, context);
```

#### Correct

```rust
if let Ok(Some(pair)) = record_reverted_correction_pair(Some("Claude Code"), Some("cloud code")) {
    tracing::info!("reverted learned pair {}", pair.id);
}
```

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
- `rank_score` is used only for ordering candidates after generation. It may include `frequency`, accepted history, bounded surrounding-context overlap, or other ranking signals.
- Surrounding-context overlap may add only a small bounded bonus to `rank_score`; it must ignore the matched span's own terms so every same-span candidate does not get the same artificial boost.
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
| Two same-span pairs have equal confidence/frequency, and only one pair's `surrounding_context` overlaps the current text outside the matched span | Prefer the context-matching pair. |
| Pair confidence is below the apply threshold but frequency is very high | Do not apply; keep original text and expose only diagnostics. |
| Pair confidence is below the apply threshold but surrounding context overlaps strongly | Do not apply; keep original text and expose only diagnostics. |
| Two candidates have the same rank score | Prefer higher `score`, then earlier start offset. |
| A longer phrase and a shorter sub-token both match | Prefer the longer phrase before rank comparison. |
| Candidate loses due to overlap | Mark `decision = SkippedOverlap` and set `blocked_by_pair_id`. |
| Candidate loses due to threshold | Mark `decision = BelowApplyThreshold` and leave `applied = false`. |

### 5. Tests Required

- Personalization engine test: repeated same-span pair beats one-off higher-confidence candidate.
- Personalization engine test: high frequency does not bypass the apply threshold.
- Personalization engine test: matching surrounding context boosts same-span candidate rank.
- Personalization engine test: context rank boost does not bypass the apply threshold.
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
- For corrected ASCII product names with an explicitly seeded pinyin alias, add cross-language alias keys. Example: `Claude Code` should add `kelaode|code` and `kelaode|KT`; `OpenAI` should add `oupenai` and `oupen|ai`.
- When an accepted learned pair updates an existing original text to a different corrected text, refresh the pair id and derived lookup keys for the new target.
- Pair updates must remove stale aliases generated from the previous corrected text so old cross-language aliases cannot point to the new target accidentally.
- Seeded aliases must be a tiny conservative table, not an automatic transliteration generator for every English word.
- Alias keys generated from corrected text are allowed because the correction pair is user-accepted, manual, imported, or otherwise already present in the personalization store.
- Existing Chinese+ASCII input key generation must continue producing aliases such as `kelaode|code`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Accepted pair `cloud code -> Claude Code` | Persist alias keys including `kelaode|code` and `kelaode|KT`. |
| Later ASR text is `我打开 克劳德 code` | Match the learned pair and output `我打开 Claude Code`. |
| Accepted pair `open ai -> OpenAI` | Persist alias keys including `oupenai` and `oupen\|ai`; later mixed ASR text `欧盆 ai` may match the same pair without storing a duplicate mixed pair. |
| Pair is later updated from `cloud code -> Claude Code` to `cloud code -> Cloud IDE` | Persist the new pair id, keep exact `cloud code` correction, and remove stale `kelaode|code` / `kelaode|KT` aliases. |
| Corrected text has no seeded product alias | Do not invent cross-language aliases. |
| Seed table is expanded | Add unit tests for generated aliases and run personalization eval. |

### 5. Tests Required

- Phonetic key test: `build_key_bundle("Claude Code")` contains `kelaode|code` and `kelaode|KT`.
- Phonetic key test: `build_key_bundle("OpenAI")` contains `oupenai` and `oupen|ai`.
- Store/engine test: accepted `cloud code -> Claude Code` reloads and corrects `克劳德 code`.
- Store/engine test: accepted `open ai -> OpenAI` reloads and corrects `欧盆 ai` through corrected-text aliases.
- Store/engine test: updating an accepted pair removes stale generated aliases and updates the pair id.

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
