// Isolated browser acceptance fixture: production AsrPage, fabricated credentials.
// Open /tests/ui/asr-model-selection.html with npm run dev. Never touches Tauri config.
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { AsrPage } from "../../src/pages/AsrPage";
import { ConfigSaveContext } from "../../src/contexts/ConfigSaveContext";
import type { AsrConfig } from "../../src/types";
import "../../src/index.css";

const legacy = {
  credentials: { qwen_api_key: "fixture-key", sensevoice_api_key: "", doubao_app_id: "", doubao_access_token: "", doubao_ime_device_id: "", doubao_ime_token: "", doubao_ime_cdid: "" },
  selection: { active_provider: "qwen", enable_fallback: false, fallback_provider: null },
  language_mode: "auto",
} as AsrConfig;
const storageKey = "ptt-asr-acceptance-fixture";
function Fixture() {
  const [config, setConfig] = useState<AsrConfig>(() => JSON.parse(localStorage.getItem(storageKey) || JSON.stringify(legacy)));
  const [saved, setSaved] = useState(config);
  const [fail, setFail] = useState(false);
  const [running, setRunning] = useState(false);
  const [showKey, setShowKey] = useState(false);
  return <main className="min-h-screen bg-[var(--paper)] p-8">
    <div className="mx-auto mb-5 flex max-w-3xl flex-wrap gap-4 text-sm">
      <button onClick={() => { localStorage.removeItem(storageKey); setConfig(legacy); setSaved(legacy); }}>载入旧配置</button>
      <label><input type="checkbox" checked={fail} onChange={e => setFail(e.target.checked)} /> 模拟保存失败</label>
      <label><input type="checkbox" checked={running} onChange={e => setRunning(e.target.checked)} /> 服务运行中</label>
      <button onClick={() => setConfig(prev => ({ ...prev, qwen_models: { ...prev.qwen_models, http: "future-saved-model" } }))}>载入未知模型</button>
    </div>
    <ConfigSaveContext.Provider value={{ syncStatus: "idle", isSaving: false, isExternalSyncing: false, syncWindowSource: null, saveImmediately: async overrides => {
      await new Promise(resolve => setTimeout(resolve, 50));
      if (fail) throw new Error("fixture save failure");
      const next = overrides?.asrConfig || config;
      localStorage.setItem(storageKey, JSON.stringify(next));
      setSaved(next);
    } }}>
      <AsrPage asrConfig={config} setAsrConfig={setConfig} showApiKey={showKey} setShowApiKey={setShowKey} isRunning={running} />
    </ConfigSaveContext.Provider>
    <pre aria-label="已保存的验收配置" className="mx-auto mt-6 max-w-3xl overflow-auto text-xs">{JSON.stringify(saved, null, 2)}</pre>
  </main>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
