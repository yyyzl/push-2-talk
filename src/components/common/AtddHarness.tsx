import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function AtddHarness() {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState("仅限本地 ATDD：5 秒后打开独立 TextEdit 验收文档，确认系统焦点后录音 18 秒。");
  return <aside aria-label="ATDD 测试驱动" className="fixed bottom-4 right-4 z-[200] max-w-md rounded-xl border border-amber-400 bg-amber-50 p-4 text-sm text-slate-900 shadow-lg">
    <p role="status">{result}</p>
    <button disabled={running} className="mt-2 rounded bg-slate-900 px-3 py-2 text-white disabled:opacity-50" onClick={async () => {
      setRunning(true);
      setResult("验收运行中：正在准备独立 TextEdit 文档，录音 18 秒后自动结束。");
      try { setResult(await invoke<string>("run")); }
      catch (error) { setResult(String(error)); }
      finally { setRunning(false); }
    }}>执行真实录音验收</button>
    <button disabled={!running} className="ml-2 rounded border border-slate-500 px-3 py-2 disabled:opacity-50" onClick={async () => {
      try { setResult(await invoke<string>("cancel_locked_recording")); }
      catch (error) { setResult(String(error)); }
    }}>取消本次录音</button>
  </aside>;
}
