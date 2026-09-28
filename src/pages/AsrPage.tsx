import { selectedQwenModel, qwenModelOptions, withQwenModel, type QwenMode } from "../utils/qwenModels";
import type { Dispatch, SetStateAction } from "react";
import { Sparkles } from "lucide-react";
import type { AsrConfig, AsrProvider } from "../types";
import {
  ASR_PROVIDERS,
} from "../constants";
import { ApiKeyInput, ConfigToggle, ConfigSelect } from "../components/common";
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
  const { saveImmediately, isExternalSyncing } = useConfigSave();
  // 只在外部配置同步时传入状态，用户本地操作让各组件自行管理 internalStatus
  const externalOnlySyncStatus = isExternalSyncing
    ? ("syncing" as const)
    : undefined;

  const fallbackChoices: AsrProvider[] = asrConfig.selection.active_provider === "doubao_ime"
    ? ["siliconflow", "qwen", "doubao"]
    : ["qwen", "doubao"].includes(asrConfig.selection.active_provider) ? ["siliconflow"] : [];
  const fallbackProvider = asrConfig.selection.fallback_provider ?? "siliconflow";
  const supportedFallback = fallbackChoices.includes(fallbackProvider);

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <header className="space-y-2">
          <h3 className="text-lg font-semibold text-[var(--ink)]">语音识别</h3>
          <p className="text-sm text-stone-600">选择识别服务，再设置它使用的模型。</p>
          {isRunning && <p className="text-xs text-stone-600">请先停止服务，再修改识别设置。</p>}
        </header>

        <div className="space-y-4">
          <div className="space-y-5">
            <div className="space-y-2">
              <label htmlFor="asr-provider" className="text-xs font-bold text-stone-600">识别服务</label>
              <ConfigSelect
                id="asr-provider"
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
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={[
                  { value: "qwen" as AsrProvider, label: ASR_PROVIDERS.qwen.name },
                  { value: "doubao" as AsrProvider, label: ASR_PROVIDERS.doubao.name },
                  { value: "doubao_ime" as AsrProvider, label: ASR_PROVIDERS.doubao_ime.name },
                  { value: "siliconflow" as AsrProvider, label: ASR_PROVIDERS.siliconflow.name },
                ]}
              />
            </div>

            <ProviderFields asrConfig={asrConfig} setAsrConfig={setAsrConfig}
              showApiKey={showApiKey} setShowApiKey={setShowApiKey} isRunning={isRunning}
              provider={asrConfig.selection.active_provider} />

            <div className="space-y-2">
              <label htmlFor="asr-language" className="text-xs font-bold text-stone-600">识别语言</label>
              <ConfigSelect
                id="asr-language"
                value={asrConfig.language_mode}
                onChange={(mode) => {
                  setAsrConfig((prev) => ({
                    ...prev,
                    language_mode: mode,
                  }));
                }}
                onCommit={async (mode) => {
                  await saveImmediately({
                    asrConfig: {
                      ...asrConfig,
                      language_mode: mode,
                    },
                  });
                }}
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={[
                  { value: "auto", label: "自动识别（推荐）" },
                  { value: "zh", label: "中文优先" },
                ]}
              />
            </div>
          </div>
        </div>

        <section className="space-y-4 border-t border-[var(--stone)] pt-6" aria-labelledby="asr-fallback-heading">
          <div className="flex items-center justify-between gap-4">
            <div className="space-y-1">
              <h4 id="asr-fallback-heading" className="text-sm font-bold text-stone-700">备用识别</h4>
              <p className="text-xs leading-relaxed text-stone-600">主服务出错时，用另一项服务识别这段录音。需要单独配置凭据。</p>
            </div>
            <ConfigToggle
              aria-label="启用备用识别"
              checked={asrConfig.selection.enable_fallback}
              onCheckedChange={next => setAsrConfig(prev => withFallbackEnabled(prev, next))}
              onCommit={async next => { await saveImmediately({ asrConfig: withFallbackEnabled(asrConfig, next) }); }}
              disabled={isRunning || (fallbackChoices.length === 0 && !asrConfig.selection.enable_fallback)}
              syncStatus={externalOnlySyncStatus}
              size="xs" variant="orange"
            />
          </div>
          {fallbackChoices.length === 0 && <p className="text-xs text-stone-600">当前主服务暂不支持额外的备用识别。已有设置会保留。</p>}
          {asrConfig.selection.enable_fallback && (
            <div className="space-y-4">
              {supportedFallback ? (
                <>
                  {fallbackChoices.length > 1 ? <div className="space-y-2">
                    <label htmlFor="asr-fallback-provider" className="text-xs font-bold text-stone-600">备用服务</label>
                    <ConfigSelect id="asr-fallback-provider" value={fallbackProvider} disabled={isRunning}
                      options={fallbackChoices.map(value => ({ value, label: ASR_PROVIDERS[value].name }))}
                      onChange={value => setAsrConfig(prev => ({ ...prev, selection: { ...prev.selection, fallback_provider: value } }))}
                      onCommit={async value => { await saveImmediately({ asrConfig: { ...asrConfig, selection: { ...asrConfig.selection, fallback_provider: value } } }); }}
                      syncStatus={externalOnlySyncStatus} />
                  </div> : <p className="text-sm font-semibold">{ASR_PROVIDERS[fallbackProvider].name}</p>}
                  <ProviderFields asrConfig={asrConfig} setAsrConfig={setAsrConfig}
                    showApiKey={showApiKey} setShowApiKey={setShowApiKey} isRunning={isRunning}
                    provider={fallbackProvider} modes={["http"]} prefix="fallback-" />
                  <p className="text-xs leading-relaxed text-stone-600">
                    {asrConfig.selection.active_provider === "doubao_ime"
                      ? "实时识别失败后，将录音发送给所选备用服务。"
                      : "千问或豆包的录音识别会同时请求主服务和硅基流动，优先采用主服务结果；实时识别失败时也会用这套配置重试。两项服务可能分别计费。"}
                  </p>
                </>
              ) : <div className="space-y-2 text-xs leading-relaxed text-stone-600">
                <p>已保存的备用服务：{ASR_PROVIDERS[fallbackProvider].name}。当前主备组合不受支持，原配置未改动。</p>
                {fallbackChoices.length > 0 && <button type="button" disabled={isRunning}
                  className="font-semibold text-[var(--ink)] underline underline-offset-4 disabled:opacity-50"
                  onClick={() => setAsrConfig(prev => ({ ...prev, selection: { ...prev.selection, fallback_provider: fallbackChoices[0] } }))}>
                  改用{ASR_PROVIDERS[fallbackChoices[0]].name}
                </button>}
              </div>}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function withFallbackEnabled(config: AsrConfig, enabled: boolean): AsrConfig {
  return {
    ...config, selection: {
      ...config.selection, enable_fallback: enabled,
      fallback_provider: config.selection.fallback_provider ?? (enabled ? "siliconflow" : null),
    }
  };
}

type ProviderFieldsProps = AsrPageProps & {
  provider: AsrProvider;
  modes?: QwenMode[];
  prefix?: string;
};
function ProviderFields({ asrConfig, setAsrConfig, showApiKey, setShowApiKey, isRunning,
  provider, modes = ["http", "realtime"], prefix = "" }: ProviderFieldsProps) {
  const { saveImmediately, isExternalSyncing } = useConfigSave();
  const externalOnlySyncStatus = isExternalSyncing ? "syncing" as const : undefined;
  return <div className="space-y-4">
    {provider === "qwen" && (
      <div className="space-y-3">
        <div className="space-y-4">
          {modes.map((mode) => (
            <div className="space-y-2" key={mode}>
              <label htmlFor={`${prefix}qwen-model-${mode}`} className="text-xs font-bold text-stone-600">
                {mode === "http" ? "松开后识别的模型" : "边说边识别的模型"}
              </label>
              <ConfigSelect
                id={`${prefix}qwen-model-${mode}`}
                value={selectedQwenModel(asrConfig, mode)}
                onChange={(id) => setAsrConfig((prev) => withQwenModel(prev, mode, id))}
                onCommit={async (id) => {
                  await saveImmediately({ asrConfig: withQwenModel(asrConfig, mode, id) });
                }}
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={qwenModelOptions(asrConfig, mode)}
              />
              <p className="break-all font-mono text-xs text-stone-500">
                {selectedQwenModel(asrConfig, mode)}
              </p>
            </div>
          ))}
          <p className="text-xs leading-relaxed text-stone-600">
            {modes.length > 1 && "两种模式分别保存，录音时使用对应的模型。"}
            带日期的选项是固定版本。升级不会替你更换已选模型。
          </p>
        </div>

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
      </div>
    )}

    {provider === "siliconflow" && (
      <div className="space-y-2">
        <label className="text-xs font-bold text-stone-500">SiliconFlow API Key</label>
        <ApiKeyInput
          value={asrConfig.credentials.sensevoice_api_key}
          onChange={(value) => setAsrConfig((prev) => ({ ...prev, credentials: { ...prev.credentials, sensevoice_api_key: value } }))}
          show={showApiKey}
          onToggleShow={() => setShowApiKey(!showApiKey)}
          placeholder="sk-..."
        />
      </div>
    )}

    {provider === "doubao" && (
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

    {provider === "doubao_ime" && (
      <div className="flex items-center gap-2 p-3 bg-emerald-50 border border-emerald-200 rounded-xl text-xs text-emerald-700">
        <Sparkles size={14} className="flex-shrink-0" />
        <span>无需配置，首次使用时自动注册设备凭据。</span>
      </div>
    )}

    {provider !== "qwen" && (
      <div className="text-xs text-stone-400 font-semibold">
        模型：{ASR_PROVIDERS[provider].model}
      </div>
    )}

  </div>;
}
