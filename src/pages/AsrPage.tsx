import type { Dispatch, SetStateAction } from "react";
import { AlertCircle, Sparkles } from "lucide-react";
import type { AsrConfig, AsrProvider, OmniAsrConfig } from "../types";
import { ASR_PROVIDERS, DEFAULT_OMNI_ASR_CONFIG, OMNI_ENDPOINT_PRESETS } from "../constants";
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
  const { saveImmediately, isExternalSyncing } = useConfigSave();
  // 只在外部配置同步时传入状态，用户本地操作让各组件自行管理 internalStatus
  const externalOnlySyncStatus = isExternalSyncing
    ? ("syncing" as const)
    : undefined;

  const isOmni = asrConfig.selection.active_provider === 'omni';
  const omniConfig: OmniAsrConfig = asrConfig.omni ?? DEFAULT_OMNI_ASR_CONFIG;

  const updateOmniConfig = (patch: Partial<OmniAsrConfig>) => {
    setAsrConfig((prev) => ({
      ...prev,
      omni: { ...(prev.omni ?? DEFAULT_OMNI_ASR_CONFIG), ...patch },
    }));
  };

  // 切换预设：缓存当前 endpoint 的完整 profile，恢复目标 endpoint 的 profile
  const switchPreset = (newEndpoint: string) => {
    setAsrConfig((prev) => {
      const cur = prev.omni ?? DEFAULT_OMNI_ASR_CONFIG;
      const profiles = { ...cur.endpoint_profiles };
      // 保存当前 profile
      profiles[cur.endpoint] = {
        api_key: cur.api_key, model: cur.model,
        enable_thinking: cur.enable_thinking, thinking_supported: cur.thinking_supported,
        custom_rules: cur.custom_rules, skip_tnl: cur.skip_tnl,
        skip_post_processing: cur.skip_post_processing, include_builtin_dictionary: cur.include_builtin_dictionary,
      };
      // 恢复目标 profile（优先缓存，其次预设默认值）
      const cached = profiles[newEndpoint];
      const presetDefaults = OMNI_ENDPOINT_PRESETS.find(p => p.value === newEndpoint)?.defaults;
      const restored = cached ?? presetDefaults ?? { ...DEFAULT_OMNI_ASR_CONFIG, api_key: '', model: '' };
      return {
        ...prev,
        omni: {
          ...cur, endpoint: newEndpoint, endpoint_profiles: profiles,
          api_key: restored.api_key, model: restored.model,
          enable_thinking: restored.enable_thinking, thinking_supported: restored.thinking_supported,
          custom_rules: restored.custom_rules, skip_tnl: restored.skip_tnl,
          skip_post_processing: restored.skip_post_processing, include_builtin_dictionary: restored.include_builtin_dictionary,
        },
      };
    });
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <span>识别引擎</span>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--steel)]" />
          <span>ASR 用于语音转文字：千问 / 豆包 + 硅基备用；Omni 精准模式通过多模态 LLM 直接转录。</span>
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
                syncStatus={externalOnlySyncStatus}
                disabled={isRunning}
                options={[
                  { value: "qwen" as AsrProvider, label: ASR_PROVIDERS.qwen.name },
                  { value: "doubao" as AsrProvider, label: ASR_PROVIDERS.doubao.name },
                  { value: "doubao_ime" as AsrProvider, label: ASR_PROVIDERS.doubao_ime.name },
                  { value: "omni" as AsrProvider, label: ASR_PROVIDERS.omni.name },
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

            {isOmni && (
              <div className="space-y-3">
                {/* 服务商预设 */}
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">服务商</label>
                  <div className="flex gap-2">
                    {OMNI_ENDPOINT_PRESETS.map((preset) => (
                      <button
                        key={preset.label}
                        type="button"
                        disabled={isRunning}
                        onClick={() => switchPreset(preset.value)}
                        className={`px-3 py-1.5 rounded-lg text-xs font-bold border transition-colors disabled:opacity-60 ${
                          omniConfig.endpoint === preset.value
                            ? 'bg-violet-100 border-violet-300 text-violet-700'
                            : 'bg-white border-stone-200 text-stone-500 hover:border-stone-300'
                        }`}
                      >
                        {preset.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* API Key */}
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">API Key</label>
                  <ApiKeyInput
                    value={omniConfig.api_key}
                    onChange={(val) => updateOmniConfig({ api_key: val })}
                    show={showApiKey}
                    onToggleShow={() => setShowApiKey(!showApiKey)}
                    disabled={isRunning}
                    placeholder="sk-..."
                  />
                </div>

                {/* 模型名称输入框 */}
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">模型名称</label>
                  <input
                    type="text"
                    value={omniConfig.model}
                    disabled={isRunning}
                    onChange={(e) => updateOmniConfig({ model: e.target.value })}
                    placeholder="LongCat-Flash-Omni-2603"
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                  />
                </div>

                {/* 开关组 */}
                <div className="space-y-2 p-3 bg-stone-50 border border-stone-200 rounded-xl">
                  {omniConfig.thinking_supported && (
                    <div className="flex items-center justify-between">
                      <div>
                        <span className="text-xs font-bold text-stone-600">深度思考模式</span>
                        <p className="text-[10px] text-stone-400 mt-0.5">启用 thinking 推理链（会增加延迟和 token 消耗）</p>
                      </div>
                      <Toggle
                        checked={omniConfig.enable_thinking}
                        onCheckedChange={(v) => updateOmniConfig({ enable_thinking: v })}
                        disabled={isRunning}
                        size="xs"
                        variant="blue"
                      />
                    </div>
                  )}
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-bold text-stone-600">包含内置词库</span>
                    <Toggle
                      checked={omniConfig.include_builtin_dictionary}
                      onCheckedChange={(v) => updateOmniConfig({ include_builtin_dictionary: v })}
                      disabled={isRunning}
                      size="xs"
                      variant="blue"
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-bold text-stone-600">跳过 TNL 规范化</span>
                    <Toggle
                      checked={omniConfig.skip_tnl}
                      onCheckedChange={(v) => updateOmniConfig({ skip_tnl: v })}
                      disabled={isRunning}
                      size="xs"
                      variant="blue"
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-bold text-stone-600">跳过 LLM 后处理</span>
                    <Toggle
                      checked={omniConfig.skip_post_processing}
                      onCheckedChange={(v) => updateOmniConfig({ skip_post_processing: v })}
                      disabled={isRunning}
                      size="xs"
                      variant="blue"
                    />
                  </div>
                </div>

                {/* 自定义转录规则文本框 */}
                <div className="space-y-2">
                  <label className="text-xs font-bold text-stone-500">自定义转录规则（可选）</label>
                  <textarea
                    value={omniConfig.custom_rules}
                    disabled={isRunning}
                    onChange={(e) => updateOmniConfig({ custom_rules: e.target.value })}
                    placeholder={"- \"PushToTalk\" 是一个产品名，必须连写驼峰\n- 说到\"克劳德\"时写成 \"Claude\"\n- 金额用阿拉伯数字+单位（如 35万）"}
                    rows={4}
                    className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60 resize-y"
                  />
                </div>
              </div>
            )}

            <div className="text-xs text-stone-400 font-semibold">
              模型：{isOmni ? omniConfig.model : ASR_PROVIDERS[asrConfig.selection.active_provider].model}
            </div>

            {/* 识别语言（Omni 不需要此选项，由 prompt 控制） */}
            {!isOmni && (<div className="space-y-2">
              <label className="text-xs font-bold text-stone-500">识别语言</label>
              <ConfigSelect
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
            </div>)}
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
              disabled={isRunning || asrConfig.selection.active_provider === 'doubao_ime' || isOmni}
              size="xs"
              variant="orange"
            />
          </div>

          {asrConfig.selection.active_provider === 'doubao_ime' && (
            <div className="flex items-center gap-2 p-3 bg-stone-50 border border-stone-200 rounded-xl text-xs text-stone-500">
              <span>豆包输入法模式暂不支持备用模型配置</span>
            </div>
          )}

          {isOmni && (
            <div className="flex items-center gap-2 p-3 bg-stone-50 border border-stone-200 rounded-xl text-xs text-stone-500">
              <span>Omni 精准模式不支持备用模型配置</span>
            </div>
          )}

          {asrConfig.selection.enable_fallback && asrConfig.selection.active_provider !== 'doubao_ime' && !isOmni && (
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
