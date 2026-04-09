import type { Dispatch, SetStateAction } from "react";
import { AlertCircle } from "lucide-react";
import type { AsrConfig, OmniAsrConfig, OmniSharedConfig } from "../types";
import {
  ASR_PROVIDERS,
  DEFAULT_OMNI_ASR_CONFIG,
  DEFAULT_OMNI_SHARED_CONFIG,
  OMNI_CUSTOM_PRESET_KEY,
  OMNI_ENDPOINT_PRESETS,
} from "../constants";
import { ApiKeyInput, Toggle } from "../components/common";

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
  const omniConfig: OmniAsrConfig = asrConfig.omni ?? DEFAULT_OMNI_ASR_CONFIG;
  const omniSharedConfig: OmniSharedConfig =
    { ...DEFAULT_OMNI_SHARED_CONFIG, ...(asrConfig.omni_shared_config ?? {}) };
  const activeOmniPresetKey = omniConfig.active_profile_key || DEFAULT_OMNI_ASR_CONFIG.active_profile_key;

  const updateOmniConfig = (patch: Partial<OmniAsrConfig>) => {
    setAsrConfig((prev) => ({
      ...prev,
      omni: { ...(prev.omni ?? DEFAULT_OMNI_ASR_CONFIG), ...patch },
    }));
  };

  const updateOmniSharedConfig = (patch: Partial<OmniSharedConfig>) => {
    setAsrConfig((prev) => ({
      ...prev,
      omni_shared_config: {
        ...(prev.omni_shared_config ?? DEFAULT_OMNI_SHARED_CONFIG),
        ...patch,
      },
    }));
  };

  const updateOmniEndpoint = (endpoint: string) => {
    setAsrConfig((prev) => {
      const cur = prev.omni ?? DEFAULT_OMNI_ASR_CONFIG;
      if (cur.active_profile_key === OMNI_CUSTOM_PRESET_KEY) {
        return {
          ...prev,
          omni: { ...cur, endpoint },
        };
      }

      const profiles = { ...cur.endpoint_profiles };
      profiles[cur.active_profile_key] = {
        endpoint: cur.endpoint,
        api_key: cur.api_key,
        model: cur.model,
        thinking_supported: cur.thinking_supported,
        skip_post_processing: cur.skip_post_processing,
      };

      return {
        ...prev,
        omni: {
          ...cur,
          active_profile_key: OMNI_CUSTOM_PRESET_KEY,
          endpoint,
          endpoint_profiles: profiles,
        },
      };
    });
  };

  const switchPreset = (newPresetKey: string) => {
    setAsrConfig((prev) => {
      const cur = prev.omni ?? DEFAULT_OMNI_ASR_CONFIG;
      const profiles = { ...cur.endpoint_profiles };
      const currentPresetKey = cur.active_profile_key || DEFAULT_OMNI_ASR_CONFIG.active_profile_key;

      profiles[currentPresetKey] = {
        endpoint: cur.endpoint,
        api_key: cur.api_key,
        model: cur.model,
        thinking_supported: cur.thinking_supported,
        skip_post_processing: cur.skip_post_processing,
      };
      const targetPreset = OMNI_ENDPOINT_PRESETS.find((preset) => preset.profileKey === newPresetKey);
      const cached = profiles[newPresetKey];
      const presetDefaults = targetPreset?.defaults ?? {
        endpoint: "",
        api_key: "",
        model: "",
        thinking_supported: false,
        skip_post_processing: true,
      };
      const restored = cached ? { ...presetDefaults, ...cached } : presetDefaults;

      return {
        ...prev,
        omni: {
          ...cur,
          active_profile_key: newPresetKey,
          endpoint: restored.endpoint || presetDefaults.endpoint,
          endpoint_profiles: profiles,
          api_key: restored.api_key,
          model: restored.model,
          thinking_supported: restored.thinking_supported,
          skip_post_processing: restored.skip_post_processing,
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
          <span>当前分支仅保留 Omni 识别链路。支持任意 OpenAI 兼容多模态模型进行语音直转文本。</span>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">Omni 主模型</h4>
          <div className="grid gap-4 lg:grid-cols-[1.15fr_0.85fr]">
            <section className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
              <div className="space-y-1">
                <div className="flex items-center justify-between gap-3">
                  <h5 className="text-sm font-bold text-[var(--ink)]">服务商连接配置</h5>
                  <span className="px-2 py-1 rounded-full bg-white border border-stone-200 text-[10px] font-bold uppercase tracking-[0.18em] text-stone-500">
                    Provider
                  </span>
                </div>
                <p className="text-[11px] text-stone-400">
                  这里只控制当前服务商预设的地址、密钥、模型和能力识别。
                </p>
              </div>

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">服务商</label>
                <div className="rounded-xl border border-[var(--stone)] bg-white px-3 py-2 text-sm font-semibold text-[var(--ink)]">
                  {ASR_PROVIDERS.omni.name}
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">服务商预设</label>
                <div className="flex flex-wrap gap-2">
                  {OMNI_ENDPOINT_PRESETS.map((preset) => (
                    <button
                      key={preset.label}
                      type="button"
                      disabled={isRunning}
                      onClick={() => switchPreset(preset.profileKey)}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold border transition-colors disabled:opacity-60 ${
                        activeOmniPresetKey === preset.profileKey
                          ? "bg-violet-100 border-violet-300 text-violet-700"
                          : "bg-white border-stone-200 text-stone-500 hover:border-stone-300"
                      }`}
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">请求 URL</label>
                <input
                  type="text"
                  value={omniConfig.endpoint}
                  disabled={isRunning}
                  onChange={(e) => updateOmniEndpoint(e.target.value)}
                  placeholder="https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                  className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                />
                <p className="text-[11px] text-stone-400">
                  支持 OpenAI 兼容地址，填写 base URL 或完整 /chat/completions 均可
                </p>
              </div>

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

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">模型名称</label>
                <input
                  type="text"
                  value={omniConfig.model}
                  disabled={isRunning}
                  onChange={(e) => updateOmniConfig({ model: e.target.value })}
                  placeholder="例如 gemini-3-flash、LongCat-Flash-Omni-2603"
                  className="w-full px-3 py-2 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] transition-colors disabled:opacity-60"
                />
              </div>

              <div className="rounded-xl border border-dashed border-stone-300 bg-white/70 px-3 py-2 text-[11px] text-stone-500">
                当前模型：<span className="font-semibold text-stone-700">{omniConfig.model}</span>
              </div>
            </section>

            <section className="space-y-3 p-4 bg-stone-950 rounded-2xl border border-stone-800 text-stone-100">
              <div className="space-y-1">
                <div className="flex items-center justify-between gap-3">
                  <h5 className="text-sm font-bold">公共转录配置</h5>
                  <span className="px-2 py-1 rounded-full bg-stone-900 border border-stone-700 text-[10px] font-bold uppercase tracking-[0.18em] text-stone-300">
                    Shared
                  </span>
                </div>
                <p className="text-[11px] text-stone-400">
                  这些设置不会跟随服务商预设切换，会稳定作用于所有 Omni 连接配置。
                </p>
              </div>

              <div className="space-y-3 rounded-2xl border border-stone-800 bg-stone-900/70 p-3">
                {omniConfig.thinking_supported ? (
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <span className="text-xs font-bold text-stone-100">深度思考模式</span>
                      <p className="text-[10px] text-stone-400 mt-0.5">启用 thinking 推理链，会增加延迟和 token 消耗</p>
                    </div>
                    <Toggle
                      checked={omniSharedConfig.enable_thinking}
                      onCheckedChange={(v) => updateOmniSharedConfig({ enable_thinking: v })}
                      disabled={isRunning}
                      size="xs"
                      variant="blue"
                    />
                  </div>
                ) : (
                  <div className="rounded-xl border border-stone-800 bg-stone-950/80 px-3 py-2 text-[11px] text-stone-400">
                    当前服务商预设未声明 thinking 支持，深度思考模式暂不可用。
                  </div>
                )}

                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-xs font-bold text-stone-100">包含内置词库</span>
                    <p className="text-[10px] text-stone-400 mt-0.5">开启后应用“词库”页中已选的内置词库领域，并注入 Omni prompt</p>
                  </div>
                  <Toggle
                    checked={omniSharedConfig.include_builtin_dictionary}
                    onCheckedChange={(v) => updateOmniSharedConfig({ include_builtin_dictionary: v })}
                    disabled={isRunning}
                    size="xs"
                    variant="blue"
                  />
                </div>

                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-xs font-bold text-stone-100">包含焦点窗口截图</span>
                    <p className="text-[10px] text-stone-400 mt-0.5">
                      将热键按下时的当前焦点窗口截图一起发送给 Omni，用于辅助纠正术语和上下文
                    </p>
                  </div>
                  <Toggle
                    checked={omniSharedConfig.include_focused_window_screenshot}
                    onCheckedChange={(v) => updateOmniSharedConfig({ include_focused_window_screenshot: v })}
                    disabled={isRunning}
                    size="xs"
                    variant="blue"
                  />
                </div>

                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-xs font-bold text-stone-100">调试保存截图</span>
                    <p className="text-[10px] text-stone-400 mt-0.5">
                      仅调试用。开启后会覆盖保存最近一次截图到
                      <span className="mx-1 font-mono text-stone-300">%TEMP%\\PushToTalkOmni\\latest-focused-window.png</span>
                    </p>
                  </div>
                  <Toggle
                    checked={omniSharedConfig.debug_save_focused_window_screenshot}
                    onCheckedChange={(v) => updateOmniSharedConfig({ debug_save_focused_window_screenshot: v })}
                    disabled={isRunning}
                    size="xs"
                    variant="blue"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-300">自定义转录规则（可选）</label>
                <textarea
                  value={omniSharedConfig.custom_rules}
                  disabled={isRunning}
                  onChange={(e) => updateOmniSharedConfig({ custom_rules: e.target.value })}
                  placeholder={"- \"PushToTalk Omni\" 是当前产品名\n- 说到\"克劳德\"时写成 \"Claude\"\n- 金额用阿拉伯数字+单位（如 35万）"}
                  rows={6}
                  className="w-full px-3 py-2 bg-stone-900 border border-stone-700 rounded-xl text-sm text-stone-100 placeholder:text-stone-500 focus:outline-none focus:border-stone-500 transition-colors disabled:opacity-60 resize-y"
                />
                <p className="text-[11px] text-stone-500">
                  适合放产品名、专有名词和格式偏好，不必因为切换服务商重复维护。
                </p>
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
