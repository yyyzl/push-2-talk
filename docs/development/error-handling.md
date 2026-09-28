# 异步资源、并发与错误处理

## Resource Lifecycle: RAII Guards in Async Pipelines

### ClipboardGuard Must Be Released Immediately in Async Flows

When a pipeline captures a system resource (e.g., `ClipboardGuard` holds the original clipboard content), the guard must be dropped **immediately** after capture — not held until the pipeline completes.

**Why**: Async pipelines (AI assistant with slow LLM models) may take 30+ seconds. Holding `ClipboardGuard` during this time prevents the user from using their clipboard normally.

**Wrong**:
```rust
let (guard, text) = clipboard_manager::get_selected_text()?;
// guard lives until pipeline completes (30+ seconds)
let result = pipeline.process(guard, text).await;
```

**Correct**:
```rust
let selected_text = match clipboard_manager::get_selected_text() {
    Ok((guard, text)) => {
        drop(guard); // Immediately restore clipboard
        text
    }
    Err(e) => None,
};
// selected_text lives in memory, clipboard is free
let result = pipeline.process(selected_text).await;
```

**Rule**: Any RAII guard that holds a shared system resource (clipboard, file lock, etc.) must be dropped at the earliest safe point. Store the extracted data in memory instead of passing the guard through the pipeline.

---

## Concurrency Guards: AtomicBool for Async Pipelines

### Mistake: `store(true)` Creates a Race Window Between Check and Set

When using `AtomicBool` as a processing guard (e.g., `is_assistant_processing`), placing the **check** in one function (hotkey callback) and the **set** in another (async pipeline) creates a time window where concurrent triggers both pass the check.

**Real example**: `is_assistant_processing` was checked in `on_start` (hotkey pressed) but set to `true` inside the async pipeline — 100ms+ later (after clipboard capture delay). The `rdev` keyboard hook fires ghost double-events, and both triggers passed the check before either set the flag.

**Wrong** — check and set are in different scopes with a time gap:
```rust
// on_start callback (synchronous)
if is_processing.load(Ordering::SeqCst) { return; } // CHECK here

// ... 100ms+ later, inside async pipeline ...
is_processing.store(true, Ordering::SeqCst); // SET here — too late!
```

**Correct** — atomic check-and-set with `compare_exchange`:
```rust
// At the pipeline entry point (first line of the critical section)
if is_processing
    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
    .is_err()
{
    tracing::warn!("Already processing, ignoring duplicate trigger");
    return;
}
// Only ONE concurrent caller reaches here
```

**Rule**: When an `AtomicBool` guards a critical section that starts asynchronously (spawned task, async pipeline), use `compare_exchange` at the **entry point of the critical section**, not `store(true)` after a delay. The `on_start` check remains as a first-line defense, but `compare_exchange` is the authoritative gate.

**Cleanup**: Ensure `store(false, Ordering::SeqCst)` is called in **all** exit paths (success, error, early return).

---

## Common Mistakes

### Mistake: Forgetting Completion Events on Discard Paths

When `AppState` holds a pending result that drives downstream effects (history recording via `transcription_complete` event), every path that disposes of the result must emit the completion event — not just the success path.

**Paths to cover**:
- Success (paste) → `inserted: true`
- Fallback (window closed, copy to clipboard) → `inserted: false`
- Dismiss (user closes panel) → `inserted: false`
- Overwrite (new result replaces old) → `inserted: false` for old result
- App stop → clear without event (no history needed for interrupted sessions)

See [Tauri IPC 说明](tauri-ipc.md#pending-state-lifecycle-across-layers) for the full lifecycle matrix.
