import {
  AlertCircle,
  BookOpen,
  Github,
  Globe,
  KeyRound,
  Mic,
  ScrollText,
  Settings,
  Wand2,
  Zap,
} from "lucide-react";
import { ASR_PROVIDERS, EXTERNAL_LINKS } from "../../constants";
import type { AsrConfig, LlmConfig } from "../../types";
import { Toggle } from "../common";

export type SettingsSectionProps = {
  asrConfig: AsrConfig;
  llmConfig: LlmConfig;
  enablePostProcess: boolean;
  useRealtime: boolean;
  activePresetName: string | null;
  isRunning: boolean;
  isAsrPrimaryValid: boolean;
  onOpenServiceModal: (tab: "asr" | "llm" | "assistant" | "dictionary") => void;
  onUseRealtimeChange: (value: boolean) => void;
  onEnablePostProcessChange: (value: boolean) => void;
  onOpenUrl: (url: string) => void;
};

export function SettingsSection({
  asrConfig,
  llmConfig,
  enablePostProcess,
  useRealtime,
  activePresetName,
  isRunning,
  isAsrPrimaryValid,
  onOpenServiceModal,
  onUseRealtimeChange,
  onEnablePostProcessChange,
  onOpenUrl,
}: SettingsSectionProps) {
  return (
    <div className="space-y-5">
      <div className="flex items-center gap-2 text-slate-900 font-semibold">
        <Settings size={18} />
        <h2>配置</h2>
      </div>

      <div className="flex items-center justify-between p-4 bg-gradient-to-r from-blue-50 to-indigo-50 rounded-xl border border-blue-100">
        <div className="flex items-center gap-3 flex-1">
          <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
            <Mic size={18} />
          </div>
          <div className="flex-1">
            <div className="text-sm font-medium text-slate-700 mb-1">ASR 语音识别</div>
            <div className="text-xs text-slate-500 space-y-0.5">
              <div>
                主模型：{ASR_PROVIDERS[asrConfig.primary.provider].name} ·{" "}
                {ASR_PROVIDERS[asrConfig.primary.provider].model}
              </div>
              <div>
                备用：
                {asrConfig.fallback && asrConfig.enable_fallback
                  ? `${ASR_PROVIDERS[asrConfig.fallback.provider].name} · ${ASR_PROVIDERS[asrConfig.fallback.provider].model}`
                  : "未配置"}
              </div>
            </div>
          </div>
        </div>
        <button
          onClick={() => onOpenServiceModal("asr")}
          disabled={isRunning}
          className="p-2 rounded-lg bg-blue-100 text-blue-600 hover:bg-blue-200 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="配置 ASR"
        >
          <Settings size={16} />
        </button>
      </div>

      {!isAsrPrimaryValid && (
        <div className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-100 rounded-xl text-amber-600 text-xs animate-in slide-in-from-top-2 fade-in duration-300">
          <AlertCircle size={14} />
          <span>请点击设置按钮配置 ASR API Key</span>
        </div>
      )}

      <div className="flex items-center justify-between p-4 bg-slate-50/80 rounded-xl border border-slate-100">
        <div className="flex items-center gap-3">
          <div
            className={`p-2 rounded-lg transition-colors ${
              useRealtime ? "bg-amber-100 text-amber-600" : "bg-blue-100 text-blue-600"
            }`}
          >
            {useRealtime ? <Zap size={18} /> : <Globe size={18} />}
          </div>
          <div>
            <div className="text-sm font-medium text-slate-700">{useRealtime ? "实时流式模式" : "HTTP 传统模式"}</div>
            <div className="text-xs text-slate-400">{useRealtime ? "边录边传，延迟更低" : "录完再传，更稳定"}</div>
          </div>
        </div>
        <Toggle
          checked={useRealtime}
          onCheckedChange={onUseRealtimeChange}
          disabled={isRunning}
          size="md"
          variant="amber"
        />
      </div>

      <div className="flex items-center justify-between p-4 bg-slate-50/80 rounded-xl border border-slate-100">
        <div className="flex items-center gap-3">
          <div
            className={`p-2 rounded-lg transition-colors ${
              enablePostProcess ? "bg-violet-100 text-violet-600" : "bg-slate-100 text-slate-400"
            }`}
          >
            <Wand2 size={18} />
          </div>
          <div className="flex-1">
            <div className="text-sm font-medium text-slate-700 flex items-center gap-2">
              LLM 智能润色
              {enablePostProcess && (
                <span className="text-[10px] bg-violet-100 text-violet-600 px-1.5 py-0.5 rounded border border-violet-200">
                  {activePresetName}
                </span>
              )}
            </div>
            <div className="text-xs text-slate-400">
              {enablePostProcess
                ? llmConfig.api_key
                  ? "自动去重、润色转录文本"
                  : "?? 未配置 API Key"
                : "直接输出原始转录"}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {enablePostProcess && (
            <button
              onClick={() => onOpenServiceModal("llm")}
              disabled={isRunning}
              className="p-2 rounded-lg bg-violet-50 text-violet-600 hover:bg-violet-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              title="配置预设"
            >
              <Settings size={16} />
            </button>
          )}
          <Toggle
            checked={enablePostProcess}
            onCheckedChange={onEnablePostProcessChange}
            disabled={isRunning}
            size="md"
            variant="violet"
          />
        </div>
      </div>

      {enablePostProcess && !llmConfig.api_key && (
        <div className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-100 rounded-xl text-amber-600 text-xs animate-in slide-in-from-top-2 fade-in duration-300">
          <AlertCircle size={14} />
          <span>请点击设置按钮配置 LLM API Key</span>
        </div>
      )}

      <div className="flex justify-end gap-4 text-xs text-slate-400">
        <button
          onClick={() => onOpenUrl(EXTERNAL_LINKS.tutorial)}
          className="hover:text-blue-600 transition-colors flex items-center gap-1 group cursor-pointer"
        >
          <BookOpen size={13} className="group-hover:scale-110 transition-transform" />
          使用教程 ↗
        </button>
        <button
          onClick={() => onOpenUrl(EXTERNAL_LINKS.apiKeyGuide)}
          className="hover:text-emerald-600 transition-colors flex items-center gap-1 group cursor-pointer"
        >
          <KeyRound size={13} className="group-hover:scale-110 transition-transform" />
          API Key 申请 ↗
        </button>
        <button
          onClick={() => onOpenUrl(EXTERNAL_LINKS.changelog)}
          className="hover:text-violet-600 transition-colors flex items-center gap-1 group cursor-pointer"
        >
          <ScrollText size={13} className="group-hover:scale-110 transition-transform" />
          更新日志 ↗
        </button>
        <button
          onClick={() => onOpenUrl(EXTERNAL_LINKS.github)}
          className="hover:text-slate-700 transition-colors flex items-center gap-1 group cursor-pointer"
        >
          <Github size={13} className="group-hover:scale-110 transition-transform" />
          GitHub ↗
        </button>
      </div>
    </div>
  );
}

