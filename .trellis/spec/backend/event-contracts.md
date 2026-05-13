# Event Contracts

> Cross-layer Tauri event payload contracts between the Rust backend and React frontend.

---

## Scenario: `transcription_complete` Optional Diagnostics

### 1. Scope / Trigger

- Trigger: any change to `transcription_complete` payloads, `PipelineResult`, backend `TranscriptionResult`, frontend `TranscriptionResult`, or persisted `HistoryRecord` fields.
- This is a cross-layer contract: Rust emits the event, `useTauriEventListeners` consumes it, and history components render/persist the derived record.
- Additive optional fields are preferred. Do not change the meaning of existing fields unless all consumers and history migration behavior are reviewed.

### 2. Signatures

Backend event payload shape:

```rust
struct TranscriptionResult {
    text: String,
    original_text: Option<String>,
    selected_text: Option<String>,
    asr_time_ms: u64,
    llm_time_ms: Option<u64>,
    total_time_ms: u64,
    mode: Option<String>,
    inserted: Option<bool>,
    tnl_diagnostics: Option<TnlDiagnostics>,
}
```

Frontend event payload shape:

```typescript
export interface TranscriptionResult {
  text: string;
  original_text: string | null;
  selected_text?: string | null;
  asr_time_ms: number;
  llm_time_ms: number | null;
  total_time_ms: number;
  mode?: string;
  inserted?: boolean;
  tnl_diagnostics?: TnlDiagnostics;
}
```

Persisted history shape:

```typescript
export interface HistoryRecord {
  originalText: string;
  polishedText: string | null;
  tnlDiagnostics?: TnlDiagnostics;
}
```

### 3. Contracts

- `text` is always the final text shown to the user and inserted/copied when applicable.
- `original_text` is optional and means "show a before/after comparison"; absence means single-column result display.
- `tnl_diagnostics` is optional. Its absence must be treated exactly like "no diagnostics".
- Diagnostics must be informational. Frontend rendering must never depend on diagnostics for correctness of the final text.
- Diagnostics may include applied, rejected, timed-out, or skipped candidates, but must not include secrets, API keys, or full prompt bodies.
- Any diagnostic field added later must be optional or have a frontend-safe default.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Backend has no TNL changes | Emit no `tnl_diagnostics` or emit an empty summary; frontend renders no diagnostics block. |
| Candidate arbitration is disabled | Do not call LLM arbitration; diagnostics may still include local applied/rejected candidates if useful. |
| LLM arbitration times out/fails | Keep final text from local high-confidence TNL only; mark pending candidates as timeout/error decisions. |
| Candidate count exceeds LLM limit | Mark extra pending candidates as skipped by limit, not as rejected by the model. |
| Old history record lacks diagnostics | History page and drawer render normally. |
| Frontend receives unknown diagnostic decision | Render a neutral fallback label instead of crashing. |

### 5. Good/Base/Bad Cases

- Good: ASR returns `Cloud Code`, dictionary contains `Claude Code`, diagnostics records `Cloud Code -> Claude Code` with `applied_llm` or `applied_local`, and history shows a compact hotword correction summary.
- Base: no candidate is found; no LLM arbitration request is sent; history looks identical to older records.
- Bad: backend sends a new required field without updating frontend types; history rendering crashes for old localStorage records.

### 6. Tests Required

- Backend unit tests for diagnostics decisions: local apply, LLM apply/reject, timeout/error, skipped-by-limit.
- Backend tests must assert no full long transcript is sent in candidate arbitration prompts; use bounded candidate context.
- Frontend tests or build/type-check must cover optional `tnl_diagnostics` on `TranscriptionResult` and `HistoryRecord`.
- Runtime regression: `transcription_complete` without diagnostics still updates transcript and writes history.
- History rendering must handle old records without diagnostics and new records with diagnostics.

### 7. Wrong vs Correct

#### Wrong

```rust
struct TranscriptionResult {
    text: String,
    tnl_diagnostics: TnlDiagnostics, // required: breaks old/no-diagnostics paths
}
```

```typescript
record.tnlDiagnostics!.candidates.map(...)
```

#### Correct

```rust
struct TranscriptionResult {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tnl_diagnostics: Option<TnlDiagnostics>,
}
```

```typescript
{record.tnlDiagnostics && (
  <TnlDiagnosticsSummary diagnostics={record.tnlDiagnostics} />
)}
```

---

## Convention: Keep Diagnostic Payloads Bounded

**What**: Event diagnostics and LLM arbitration inputs must be compact summaries, not full internal traces.

**Why**: Dictation is latency-sensitive and history is persisted in localStorage. Large payloads increase latency, storage pressure, and privacy risk.

**Example**:

```rust
// Good: candidate-local context only
let context = bounded_context(text, candidate.start, candidate.end, 64);

// Bad: send or persist the full transcript/prompt for every candidate
let context = text.to_string();
```

**Related**: `llm_post_processor` candidate arbitration and frontend history persistence.

---

## Scenario: `add_learned_word` Optional Personalization Pair

### 1. Scope / Trigger

- Trigger: any change to the `add_learned_word` Tauri command, auto vocabulary learning Toast payload, or personalization correction-pair persistence.
- This command has two compatible modes:
  - dictionary-only: manual dictionary management sends only `word` and `source`.
  - accepted learning suggestion: the learning Toast also sends `original`, `corrected`, and `category`.

### 2. Signatures

Backend command shape:

```rust
async fn add_learned_word(
    app_handle: AppHandle,
    word: String,
    source: String,
    original: Option<String>,
    corrected: Option<String>,
    category: Option<String>,
) -> Result<(), String>
```

Frontend accepted-learning payload:

```typescript
await invoke("add_learned_word", {
  word: suggestion.word,
  source: "auto",
  original: suggestion.original,
  corrected: suggestion.corrected,
  category: suggestion.category,
});
```

### 3. Contracts

- Missing optional fields must preserve legacy dictionary-only behavior.
- If `original` and `corrected` are both present, non-empty, and not the same after surface normalization, the backend must persist a local personalization correction pair.
- A user-accepted correction pair is considered confirmed enough to be used by the local personalization decoder on the next dictation.
- Correction pairs are local app data under the PushToTalk config directory. Do not emit them through frontend events unless a UI explicitly needs them.
- Manual dictionary edits must not require or synthesize `original` / `corrected`.
- Accepted learning suggestions must not overwrite or reformat an existing manual correction pair for the same `original`, even when the corrected target differs only by case or spacing.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Manual dictionary add sends only `word` and `source` | Add/update dictionary entry; no personalization pair is written. |
| Learning Toast accepts `cloud code -> Claude Code` | Add dictionary entry and write a `learned` correction pair. |
| Optional fields are empty or normalize to the same text | Add dictionary entry; skip correction-pair write. |
| Correction-pair JSON is missing | Create it atomically through the store save path. |
| Correction-pair JSON is invalid | Return an error for accepted-learning persistence instead of silently overwriting unknown data. |
| A manual pair already maps `cloud code -> Claude Code`, and learning accepts `cloud code -> Cloud IDE` | Keep the manual pair unchanged; still allow the dictionary add/update path to proceed. |
| A manual pair already maps `cloud code -> Claude Code`, and learning accepts `cloud code -> claude code` | Keep the manual pair's casing, id, category, counters, and keys unchanged. |

### 5. Tests Required

- Backend storage test: accepted correction persists, reloads, and retains category/source/confidence.
- Backend conversion test: a freshly accepted mixed-language pair is confident enough to apply after reload.
- Backend storage/conversion test: accepted learning does not overwrite or reformat an existing manual correction pair.
- Frontend build/type-check must cover the extended Toast `invoke` payload.

---

## Scenario: `dismiss_learning_suggestion` Optional Personalization Reject

### 1. Scope / Trigger

- Trigger: any change to the `dismiss_learning_suggestion` Tauri command, learning Toast dismiss behavior, or personalization correction-pair negative feedback.
- Explicit user dismissals and passive timeout dismissals have different semantics.

### 2. Signatures

Backend command shape:

```rust
async fn dismiss_learning_suggestion(
    id: String,
    original: Option<String>,
    corrected: Option<String>,
) -> Result<(), String>
```

Frontend explicit-dismiss payload:

```typescript
await invoke("dismiss_learning_suggestion", {
  id: suggestion.id,
  original: suggestion.original,
  corrected: suggestion.corrected,
});
```

Frontend timeout payload:

```typescript
await invoke("dismiss_learning_suggestion", { id: suggestion.id });
```

### 3. Contracts

- Missing optional fields must preserve notification-only behavior.
- An explicit dismiss from the button or Escape key may record negative feedback for an existing correction pair.
- Passive timeout must not record negative feedback because the user may not have seen the suggestion.
- Reject feedback must not create a new correction pair. It only updates an existing pair that matches both normalized `original` and normalized `corrected`.
- Rejected learned pairs must affect future matching. Non-manual exact matches should respect confidence so a rejected learned pair can fall below the automatic-apply threshold.
- A later accepted learning suggestion for the same learned pair must re-enable it and clear the consecutive reject streak.
- Manual pairs remain high-priority and must not be weakened, rejected, or disabled by learning-suggestion dismiss feedback.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|---|---|
| Explicit dismiss sends original/corrected and pair exists | Increment `rejected_count`, lower confidence, persist store. |
| Explicit dismiss sends original/corrected but pair is missing | No new pair is created; command succeeds. |
| Timeout sends only id | No reject is recorded; command succeeds. |
| Rejected learned pair falls below threshold | It no longer auto-applies, including exact-text matches. |
| Pair reaches consecutive reject threshold | Pair is disabled and excluded from lookup. |
| A disabled learned pair is later accepted again | Re-enable the pair, restore high confidence, and reset `rejected_count` to 0. |
| Matching pair is `manual` | Do not increment `rejected_count`, lower confidence, or disable the pair. |

### 5. Tests Required

- Backend storage test: reject lowers confidence and does not create a missing pair.
- Backend conversion test: a rejected learned exact-text pair no longer auto-applies.
- Backend storage/conversion test: accepting a previously rejected learned pair clears the reject streak and restores conversion.
- Backend storage/conversion test: reject feedback does not weaken or disable a manual pair.
- Frontend build/type-check must cover the explicit-dismiss payload while preserving timeout payload compatibility.
