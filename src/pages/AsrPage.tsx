import type { Dispatch, SetStateAction } from "react";
import { AlertCircle } from "lucide-react";
import type { AsrCache, AsrConfig, AsrProvider } from "../types";
import { ASR_PROVIDERS } from "../constants";
import { ApiKeyInput, Toggle } from "../components/common";

export type AsrPageProps = {
  asrCache: AsrCache;
  setAsrCache: Dispatch<SetStateAction<AsrCache>>;
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;

  isRunning: boolean;
};

export function AsrPage({
  asrCache,
  setAsrCache,
  asrConfig,
  setAsrConfig,
  showApiKey,
  setShowApiKey,
  isRunning,
}: AsrPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <span>识别引擎</span>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--steel)]" />
          <span>ASR 用于语音转文字：千问 / 豆包 + 硅基备用。</span>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">主模型</h4>
          <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
            <div className="space-y-2">
              <label className="text-xs font-bold text-stone-500">服务商</label>
              <select
                value={asrConfig.primary.provider}
                disabled={isRunning}
                onChange={(e) => {
                  const newProvider = e.target.value as AsrProvider;
                  setAsrConfig((prev) => ({
                    ...prev,
                    primary:
                      newProvider === "qwen"
                        ? { provider: "qwen", api_key: asrCache.qwen.api_key }
                        : {
                            provider: "doubao",
                            api_key: "",
                            app_id: asrCache.doubao.app_id,
                            access_token: asrCache.doubao.access_token,
                          },
                  }));
                  setAsrCache((prev) => ({ ...prev, active_provider: newProvider }));
                }}
                className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
              >
                <option value="qwen">{ASR_PROVIDERS.qwen.name}</option>
                <option value="doubao">{ASR_PROVIDERS.doubao.name}</option>
              </select>
            </div>

            {asrConfig.primary.provider === "qwen" ? (
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">API Key</label>
                <ApiKeyInput
                  value={asrConfig.primary.api_key}
                  onChange={(value) => {
                    setAsrConfig((prev) => ({ ...prev, primary: { ...prev.primary, api_key: value } }));
                    setAsrCache((prev) => ({ ...prev, qwen: { api_key: value } }));
                  }}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                  placeholder="sk-..."
                />
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">APP ID</label>
                  <input
                    type="text"
                    value={asrConfig.primary.app_id || ""}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({ ...prev, primary: { ...prev.primary, app_id: value } }));
                      setAsrCache((prev) => ({ ...prev, doubao: { ...prev.doubao, app_id: value } }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">Access Token</label>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={asrConfig.primary.access_token || ""}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({ ...prev, primary: { ...prev.primary, access_token: value } }));
                      setAsrCache((prev) => ({ ...prev, doubao: { ...prev.doubao, access_token: value } }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
              </div>
            )}

            <div className="text-xs text-stone-400 font-semibold">
              模型：{ASR_PROVIDERS[asrConfig.primary.provider].model}
            </div>
          </div>
        </div>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-bold text-stone-700">备用模型</h4>
            <Toggle
              checked={asrConfig.enable_fallback}
              onCheckedChange={(next) =>
                setAsrConfig((prev) => ({
                  ...prev,
                  enable_fallback: next,
                  fallback:
                    next && (!prev.fallback?.api_key)
                      ? { provider: "siliconflow", api_key: asrCache.siliconflow.api_key }
                      : prev.fallback,
                }))
              }
              disabled={isRunning}
              size="xs"
              variant="indigo"
            />
          </div>

          {asrConfig.enable_fallback && (
            <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">服务商</label>
                <select
                  value={asrConfig.fallback?.provider || "siliconflow"}
                  disabled={isRunning}
                  onChange={(e) =>
                    setAsrConfig((prev) => ({
                      ...prev,
                      fallback: { provider: e.target.value as AsrProvider, api_key: prev.fallback?.api_key || "" },
                    }))
                  }
                  className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                >
                  <option value="siliconflow">{ASR_PROVIDERS.siliconflow.name}</option>
                </select>
              </div>
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">API Key</label>
                <ApiKeyInput
                  value={asrConfig.fallback?.api_key || ""}
                  onChange={(val) => {
                    setAsrConfig((prev) => ({
                      ...prev,
                      fallback: { provider: prev.fallback?.provider || "siliconflow", api_key: val },
                    }));
                    setAsrCache((prev) => ({ ...prev, siliconflow: { api_key: val } }));
                  }}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                  placeholder="sk-..."
                />
              </div>
              <div className="text-xs text-stone-400 font-semibold">
                模型：{ASR_PROVIDERS[asrConfig.fallback?.provider || "siliconflow"].model}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
