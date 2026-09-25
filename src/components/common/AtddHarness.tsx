import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function AtddHarness() {
  const [running, setRunning] = useState(false);
  const [scenario, setScenario] = useState("dictation");
  const [result, setResult] = useState("仅限本地 ATDD：5 秒后打开独立 TextEdit 文档，初始化成功后真实录音 18 秒。助手结果需通过结果面板插入。");
  return <aside aria-label="ATDD 测试驱动" className="fixed bottom-4 right-4 z-[200] max-w-md rounded-xl border border-amber-400 bg-amber-50 p-4 text-sm text-slate-900 shadow-lg">
    <label className="mb-2 block">验收场景
      <select disabled={running} value={scenario} onChange={(event) => setScenario(event.target.value)} className="ml-2 rounded border bg-white p-1">
        <option value="dictation">真实听写</option>
        <option value="assistant_question">助手问答（无选区）</option>
        <option value="assistant_selection">助手处理选中文本</option>
      </select>
    </label>
    <p role="status" className="max-h-40 overflow-y-auto whitespace-pre-wrap">{result}</p>
    <button disabled={running} className="mt-2 rounded bg-slate-900 px-3 py-2 text-white disabled:opacity-50" onClick={async () => {
      setRunning(true);
      setResult("验收运行中：准备独立文档与选区，初始化成功后录音 18 秒，再等待真实处理结果。");
      try { setResult(await invoke<string>("run", { scenario })); }
      catch (error) { setResult(String(error)); }
      finally { setRunning(false); }
    }}>执行真实录音验收</button>
    <button disabled={!running} className="ml-2 rounded border border-slate-500 px-3 py-2 disabled:opacity-50" onClick={async () => {
      try { setResult(await invoke<string>("atdd_cancel")); }
      catch (error) { setResult(String(error)); }
    }}>取消本次录音</button>
    <div className="mt-2 flex gap-2">
      <button disabled={running} className="rounded border border-slate-500 px-2 py-1 disabled:opacity-50" onClick={async () => {
        try { setResult(await invoke<string>("paste_latest_reply")); }
        catch (error) { setResult(String(error)); }
      }}>粘贴助手最新回复</button>
      <button disabled={running} className="rounded border border-slate-500 px-2 py-1 disabled:opacity-50" onClick={async () => {
        try { await invoke("dismiss_conversation"); setResult("已结束助手会话"); }
        catch (error) { setResult(String(error)); }
      }}>结束助手会话</button>
    </div>
  </aside>;
}
