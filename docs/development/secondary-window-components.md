# Tauri 副窗口组件约定

## Tauri Secondary Window Components

Secondary window components (OverlayWindow, NotificationWindow, ResultPanelWindow) share a common lifecycle pattern:

### Pattern: Push + Poll → State → Render → Invoke

Hidden WebViews (`visible: false`) do not reliably receive push events. Use dual strategy:

```tsx
const [data, setData] = useState<Payload | null>(null);

// Push: event listener (works when window is already visible)
useEffect(() => {
  const unlisten = await listen<Payload>("event_name", (e) => setData(e.payload));
  return () => unlisten();
}, []);

// Poll: fallback for hidden→visible transition (self-stopping)
useEffect(() => {
  if (data) return;
  const fetch = async () => {
    const pending = await invoke<Payload | null>("get_pending_data");
    if (pending) setData(pending);
  };
  fetch();
  const interval = setInterval(fetch, 300);
  return () => clearInterval(interval);
}, [data]);

// Render based on data (null = waiting state)
if (!data) return <WaitingState />;

// Actions invoke backend commands
const handleAction = () => invoke("command_name");
```

See [Tauri IPC：隐藏窗口事件](tauri-ipc.md#mistake-5-hidden-webview-does-not-process-push-events-critical) for details.

### React StrictMode + Async `listen()` Causes Duplicate Event Processing (CRITICAL)

React 18 StrictMode in dev mode double-mounts components. When `listen()` is called inside an async `useEffect` setup function, the cleanup runs **before** the `listen()` promise resolves, leaving the old listener alive. The component then re-mounts and registers a **second** listener. Every backend event triggers both callbacks.

**Impact**: Accumulative state updates like `setItems(prev => [...prev, item])` execute twice, producing duplicate entries. Overwrite updates like `setState(value)` are unaffected because double-setting the same value is idempotent — this is why the bug only manifests in components with accumulative state (e.g., conversation turns), not in OverlayWindow or NotificationWindow.

**Wrong** — cleanup runs before `unlisten` is assigned:
```tsx
useEffect(() => {
  let unlisten: (() => void) | undefined;
  const setup = async () => {
    unlisten = await listen<T>("event", (e) => {
      setItems((prev) => [...prev, e.payload]); // RUNS TWICE per event!
    });
  };
  setup();
  return () => unlisten?.(); // unlisten is still undefined here!
}, []);
```

**Correct** — `cancelled` flag + deferred unsubscribe:
```tsx
useEffect(() => {
  let cancelled = false;
  const cleanups: (() => void)[] = [];

  const setup = async () => {
    const u = await listen<T>("event", (e) => {
      if (cancelled) return;  // Guard: ignore events from stale listener
      setItems((prev) => [...prev, e.payload]);
    });
    if (cancelled) { u(); return; }  // Already unmounted: unsubscribe immediately
    cleanups.push(u);
  };
  setup();

  return () => {
    cancelled = true;
    cleanups.forEach((fn) => fn());
  };
}, []);
```

**Rule**: Every async `listen()` in a `useEffect` must use the `cancelled` flag pattern. This is a **mandatory** pattern for all Tauri secondary window components.

**Detection**: If a single backend `app.emit()` causes two identical state updates in the frontend, check for missing `cancelled` flag in the listener setup.

### Transparent Window Dragging

`data-tauri-drag-region` HTML attribute **does not work** on transparent windows (`transparent: true` + `decorations: false`) with Windows WebView2.

**Wrong**:
```tsx
<div data-tauri-drag-region>Title Bar</div>
```

**Correct**: Use `getCurrentWindow().startDragging()` on mousedown:
```tsx
import { getCurrentWindow } from "@tauri-apps/api/window";

const startDrag = () => getCurrentWindow().startDragging().catch(() => {});

<div onMouseDown={startDrag} className="cursor-move select-none">Title Bar</div>

{/* Interactive children must stop propagation to prevent accidental drag */}
<button onMouseDown={(e) => e.stopPropagation()} onClick={handleClose}>×</button>
```

**Prerequisite**: `core:window:allow-start-dragging` must be in `capabilities/default.json`.

### Destructive IPC Call Debounce

When an IPC command uses `.take()` on a backend `Option<T>` (consuming the resource), the frontend must prevent double invocations. The `.take()` provides a natural backend guard (second call gets `None` → error), but the error message confuses users.

**Pattern**: Use an `isPasting`/`isProcessing` state guard + `try/finally`:

```tsx
const [isPasting, setIsPasting] = useState(false);

const handlePaste = useCallback(async () => {
  if (!result || isPasting) return;
  setIsPasting(true);
  try {
    await invoke("paste_latest_reply");
    setResult(null); // Clear state to prevent stale display
  } finally {
    setIsPasting(false);
  }
}, [result, isPasting]);
```

### State Cleanup After Successful Operations

When a secondary window's operation succeeds (e.g., paste), always clear the component's local state. The window is hidden but not destroyed — if it's shown again without a new event, stale data would be displayed.

**Wrong**: Only hide the window, keep React state intact.
**Correct**: `setResult(null)` after successful operation.

---

## Common Mistakes

### Mistake: Using Wrong IPC Command Name

Tauri `invoke()` calls fail silently if the command name doesn't match the backend registration. Always verify command names match `#[tauri::command] fn name()` in `lib.rs`.

See [Tauri IPC 说明](tauri-ipc.md#mistake-4-ipc-command-name-mismatch-fails-silently) for details.
