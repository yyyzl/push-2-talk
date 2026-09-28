# Tauri IPC 与跨窗口状态

## 启动状态必须早于窗口创建

Tauri 2 的顺序是：初始化插件 → 创建配置中的 WebView → 执行应用 `setup`。隐藏窗口也可能立即发送 IPC，因此在应用 `setup` 中注册 `AppState` 已经太晚：Windows 热启动实测会触发 `state() called before manage()`，让初始配置加载一直等待。

`application::runtime::plugin()` 在单实例插件之后、窗口创建之前完成配置加载和状态注册。第二实例先由单实例插件处理，避免重复迁移配置；首次 IPC 能取得完整的运行状态。应用 `setup` 只创建托盘和启动后台更新，不能再按磁盘快照覆盖可能已由前端启动命令更新的运行状态。不要用固定延时掩盖初始化竞态。

Windows 回归须在隔离测试账号，或备份并隔离现有应用配置后，运行真实打包程序：

```powershell
.\scripts\test-windows-startup.ps1 -ExecutablePath "D:\PushToTalk\push-to-talk.exe" -OutputDirectory ".\src-tauri\target\startup-smoke" -RunCount 10
```

脚本拒绝已有运行实例，逐次启动并检查服务就绪与 panic，保留日志后终止本轮创建的进程。每轮默认等待最多 6 秒，整组最多 55 秒；输出目录必须是新的。脚本不会触发录音或连接测试，应用自身的启动请求（如词库更新）仍可能发生；此检查不代替正常退出、开机自启、麦克风和跨窗口回填验收。

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
