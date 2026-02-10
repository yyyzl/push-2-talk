import type { Dispatch, SetStateAction } from "react";
import { AlertCircle, Sparkles } from "lucide-react";
import type { AsrConfig, AsrProvider } from "../types";
import { ASR_PROVIDERS } from "../constants";
import { ApiKeyInput, Toggle, ConfigSelect } from "../components/common";
import { useConfigSave } from "../contexts/ConfigSaveContext";

export type AsrPageProps = {
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;

  isRunning: boolean;
};

export function AsrPage({
  asrConfig,
  setAsrConfig,
  showApiKey,
  setShowApiKey,
  isRunning,
}: AsrPageProps) {
  const { saveImmediately, syncStatus } = useConfigSave();

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
              <ConfigSelect
                value={asrConfig.selection.active_provider}
                onChange={(newProvider) => {
                  setAsrConfig((prev) => ({
                    ...prev,
                    selection: { ...prev.selection, active_provider: newProvider },
                  }));
                }}
                onCommit={async (newProvider) => {
                  await saveImmediately({
                    asrConfig: {
                      ...asrConfig,
                      selection: { ...asrConfig.selection, active_provider: newProvider },
                    },
                  });
                }}
                syncStatus={syncStatus}
                disabled={isRunning}
                options={[
                  { value: "qwen" as AsrProvider, label: ASR_PROVIDERS.qwen.name },
                  { value: "doubao" as AsrProvider, label: ASR_PROVIDERS.doubao.name },
                  { value: "doubao_ime" as AsrProvider, label: ASR_PROVIDERS.doubao_ime.name },
                ]}
              />
            </div>

            {asrConfig.selection.active_provider === "qwen" && (
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">API Key</label>
                <ApiKeyInput
                  value={asrConfig.credentials.qwen_api_key}
                  onChange={(value) => {
                    setAsrConfig((prev) => ({
                      ...prev,
                      credentials: { ...prev.credentials, qwen_api_key: value },
                    }));
                  }}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                  placeholder="sk-..."
                />
              </div>
            )}

            {asrConfig.selection.active_provider === "doubao" && (
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">APP ID</label>
                  <input
                    type="text"
                    value={asrConfig.credentials.doubao_app_id}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({
                        ...prev,
                        credentials: { ...prev.credentials, doubao_app_id: value },
                      }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">Access Token</label>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={asrConfig.credentials.doubao_access_token}
                    disabled={isRunning}
                    onChange={(e) => {
                      const value = e.target.value;
                      setAsrConfig((prev) => ({
                        ...prev,
                        credentials: { ...prev.credentials, doubao_access_token: value },
                      }));
                    }}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>
              </div>
            )}

            {asrConfig.selection.active_provider === "doubao_ime" && (
              <div className="flex items-center gap-2 p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-700">
                <Sparkles size={14} className="flex-shrink-0" />
                <span>无需配置，首次使用时自动注册设备凭据。</span>
              </div>
            )}

            <div className="text-xs text-stone-400 font-semibold">
              模型：{ASR_PROVIDERS[asrConfig.selection.active_provider].model}
            </div>
          </div>
        </div>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-bold text-stone-700">备用模型</h4>
            <Toggle
              checked={asrConfig.selection.enable_fallback}
              onCheckedChange={(next) =>
                setAsrConfig((prev) => ({
                  ...prev,
                  selection: {
                    ...prev.selection,
                    enable_fallback: next,
                    fallback_provider: next ? "siliconflow" : null,
                  },
                }))
              }
              disabled={isRunning || asrConfig.selection.active_provider === 'doubao_ime'}
              size="xs"
              variant="orange"
            />
          </div>

          {asrConfig.selection.active_provider === 'doubao_ime' && (
            <div className="flex items-center gap-2 p-3 bg-stone-50 border border-stone-200 rounded-xl text-xs text-stone-500">
              <span>豆包输入法模式暂不支持备用模型配置</span>
            </div>
          )}

          {asrConfig.selection.enable_fallback && asrConfig.selection.active_provider !== 'doubao_ime' && (
            <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">服务商</label>
                <select
                  value={asrConfig.selection.fallback_provider || "siliconflow"}
                  disabled={isRunning}
                  onChange={(e) =>
                    setAsrConfig((prev) => ({
                      ...prev,
                      selection: {
                        ...prev.selection,
                        fallback_provider: e.target.value as AsrProvider,
                      },
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
                  value={asrConfig.credentials.sensevoice_api_key}
                  onChange={(val) => {
                    setAsrConfig((prev) => ({
                      ...prev,
                      credentials: { ...prev.credentials, sensevoice_api_key: val },
                    }));
                  }}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                  placeholder="sk-..."
                />
              </div>
              <div className="text-xs text-stone-400 font-semibold">
                模型：{ASR_PROVIDERS[asrConfig.selection.fallback_provider || "siliconflow"].model}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
