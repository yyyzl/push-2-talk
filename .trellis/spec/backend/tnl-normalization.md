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
