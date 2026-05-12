# Cross-Layer Thinking Guide

> **Purpose**: Think through data flow across layers before implementing.

---

## The Problem

**Most bugs happen at layer boundaries**, not within layers.

Common cross-layer bugs:
- API returns format A, frontend expects format B
- Database stores X, service transforms to Y, but loses data
- Multiple layers implement the same logic differently

---

## Before Implementing Cross-Layer Features

### Step 1: Map the Data Flow

Draw out how data moves:

```
Source → Transform → Store → Retrieve → Transform → Display
```

For each arrow, ask:
- What format is the data in?
- What could go wrong?
- Who is responsible for validation?

### Step 2: Identify Boundaries

| Boundary | Common Issues |
|----------|---------------|
| API ↔ Service | Type mismatches, missing fields |
| Service ↔ Database | Format conversions, null handling |
| Backend ↔ Frontend | Serialization, date formats |
| Component ↔ Component | Props shape changes |

### Step 3: Define Contracts

For each boundary:
- What is the exact input format?
- What is the exact output format?
- What errors can occur?

---

## Common Cross-Layer Mistakes

### Mistake 1: Implicit Format Assumptions

**Bad**: Assuming date format without checking

**Good**: Explicit format conversion at boundaries

### Mistake 2: Scattered Validation

**Bad**: Validating the same thing in multiple layers

**Good**: Validate once at the entry point

### Mistake 3: Leaky Abstractions

**Bad**: Component knows about database schema

**Good**: Each layer only knows its neighbors

---

## Tauri IPC Boundary Contracts

### Mistake 4: IPC Command Name Mismatch Fails Silently

Tauri `invoke("command_name")` fails silently when the command does not exist in the backend `.invoke_handler()` registration. A `.catch(console.error)` only logs to devtools — the UI falls back to defaults with no user-visible error.

**Real example**: `invoke("get_config")` was called but the backend only registered `load_config`. Result: theme always defaulted to "light" with zero visible errors.

**Rule**: When adding a new `invoke()` call in frontend, immediately grep the backend for the exact command name:

```bash
grep -n "fn <command_name>" src-tauri/src/lib.rs
grep -n "<command_name>" src-tauri/src/lib.rs  # also check .invoke_handler() registration
```

### Mistake 5: Hidden WebView Does NOT Process Push Events (CRITICAL)

When a Tauri window is created with `visible: false`, its WebView **does not reliably process IPC events** (`listen()`, `emit()`). Events emitted before or immediately after `panel.show()` may be silently lost — even with delays. This was confirmed on Windows + WebView2.

**Symptoms**: Frontend event listener is registered, backend `app.emit()` succeeds, but the callback never fires. No errors anywhere.

**Wrong**: Rely solely on push events (`listen()`) for hidden→visible windows.

**Correct**: Use **Push + Poll dual strategy**:

```tsx
// Push: event listener (works when window is already visible)
useEffect(() => {
  const unlisten = await listen<Payload>("event_name", (e) => setData(e.payload));
  return () => unlisten();
}, []);

// Poll: fallback for hidden→visible transition
useEffect(() => {
  if (data) return; // Already have data, stop polling
  const fetch = async () => {
    const pending = await invoke<Payload | null>("get_pending_data");
    if (pending) setData(pending);
  };
  fetch(); // Immediate first attempt
  const interval = setInterval(fetch, 300);
  return () => clearInterval(interval);
}, [data]);
```

**Why poll works**: `invoke()` goes through a different IPC channel than `listen()`/`emit()` and is reliable even when the WebView was recently hidden.

### Mistake 6: New Windows MUST Be Added to Capabilities (CRITICAL — Silent Total Failure)

In Tauri 2.0, `src-tauri/capabilities/default.json` has a `"windows"` array that controls which windows receive IPC permissions. **If a new window label is not in this array, ALL frontend IPC operations silently fail** — `invoke()`, `listen()`, `emit()`, `startDragging()` return no data and throw no errors.

**Real example**: `result_panel` was defined in `tauri.conf.json` and the window appeared, but was missing from `capabilities/default.json`. Result: `invoke()` returned null, `listen()` never fired, `startDragging()` did nothing. All failures were silent.

**Rule**: When adding a new window to `tauri.conf.json`, **immediately** add its label to `capabilities/default.json`:

```jsonc
// src-tauri/capabilities/default.json
{
  "windows": ["main", "overlay", "notification", "result_panel"],  // ← add here
  "permissions": [...]
}
```

**Checklist for new Tauri windows**:
1. Add window config to `tauri.conf.json`
2. Add window label to `capabilities/default.json` `"windows"` array
3. Add any needed permissions (e.g., `core:window:allow-start-dragging`)
4. Add HTML entry to project root
5. Add entry to `vite.config.ts` `rollupOptions.input`

### Pending State Lifecycle Across Layers

When `AppState` holds an `Option<PendingResult>` that both backend and frontend interact with, three lifecycle edges must be handled:

| Edge | What Happens | Required Action |
|------|-------------|-----------------|
| **Overwrite** | New result arrives while old result is still pending | Emit completion event for old result before replacing (ensures history recording) |
| **Dismiss** | User closes the result panel without acting | Emit completion event with `inserted: false` (ensures history recording) |
| **App Stop** | Service stopped while result is pending | Clear pending state + hide window (prevents stale UI on restart) |

**Wrong**: Only handle the happy path (user clicks paste).

**Correct**: Treat `PendingResult` as a resource with acquire/release semantics — every path that consumes or discards it must emit the appropriate completion event.

---

## Checklist for Cross-Layer Features

Before implementation:
- [ ] Mapped the complete data flow
- [ ] Identified all layer boundaries
- [ ] Defined format at each boundary
- [ ] Decided where validation happens
- [ ] Verified IPC command names match between frontend `invoke()` and backend `#[tauri::command]`
- [ ] Checked pending/temporary state has cleanup in all exit paths (success, error, dismiss, app stop)
- [ ] **New window?** Added label to `capabilities/default.json` `"windows"` array
- [ ] **New window API?** Added required permission (e.g., `core:window:allow-start-dragging`)

After implementation:
- [ ] Tested with edge cases (null, empty, invalid)
- [ ] Verified error handling at each boundary
- [ ] Checked data survives round-trip
- [ ] Confirmed IPC command registration in `.invoke_handler()`
- [ ] **Hidden→visible window?** Verified data delivery with poll fallback (not just push events)

---

## When to Create Flow Documentation

Create detailed flow docs when:
- Feature spans 3+ layers
- Multiple teams are involved
- Data format is complex
- Feature has caused bugs before
