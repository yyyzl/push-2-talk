import type { Dispatch, SetStateAction } from "react";
import { Plus } from "lucide-react";
import type {
  AsrCache,
  AsrConfig,
  AsrProvider,
  DualHotkeyConfig,
  HotkeyKey,
  LlmConfig,
} from "../../types";
import { ASR_PROVIDERS } from "../../constants";
import { formatHotkeyDisplay, formatHotkeyKeysDisplay } from "../../utils";
import { Toggle } from "../common";
import type { AppPage } from "../../pages/types";

export type RightPanelProps = {
  asrCache: AsrCache;
  setAsrCache: Dispatch<SetStateAction<AsrCache>>;
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  useRealtime: boolean;
  setUseRealtime: Dispatch<SetStateAction<boolean>>;

  enablePostProcess: boolean;
  setEnablePostProcess: Dispatch<SetStateAction<boolean>>;
  llmConfig: LlmConfig;
  setLlmConfig: Dispatch<SetStateAction<LlmConfig>>;

  dualHotkeyConfig: DualHotkeyConfig;

  dictionary: string[];
  newWord: string;
  setNewWord: (next: string) => void;
  onAddWord: () => void;

  isRunning: boolean;
  onNavigate: (page: AppPage) => void;
};

export function RightPanel({
  asrCache,
  setAsrCache,
  asrConfig,
  setAsrConfig,
  useRealtime,
  setUseRealtime,
  enablePostProcess,
  setEnablePostProcess,
  llmConfig,
  setLlmConfig,
  dualHotkeyConfig,
  dictionary,
  newWord,
  setNewWord,
  onAddWord,
  isRunning,
  onNavigate,
}: RightPanelProps) {
  void onNavigate;
  const releaseModeKeys =
    dualHotkeyConfig.dictation.release_mode_keys?.length
      ? dualHotkeyConfig.dictation.release_mode_keys
      : (["f2"] as HotkeyKey[]);
  return (
    <aside className="flex shrink-0 w-80 h-full min-h-0 bg-[var(--paper)] border-l border-[var(--stone)] flex-col p-5 gap-5 overflow-y-auto custom-scroll font-sans">
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            语音识别引擎
          </label>
          {/* <button
            onClick={() => onNavigate("asr")}
            className="text-[10px] font-bold text-[var(--steel)] hover:opacity-80 transition-opacity"
          >
            Edit
          </button> */}
        </div>
        <div className="relative">
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
            className="w-full bg-white border border-[var(--stone)] rounded-xl px-3 py-2 text-xs font-bold outline-none appearance-none cursor-pointer focus:border-[var(--steel)] shadow-sm"
          >
            <option value="qwen">
              {ASR_PROVIDERS.qwen.name} · {ASR_PROVIDERS.qwen.model}
            </option>
            <option value="doubao">
              {ASR_PROVIDERS.doubao.name} · {ASR_PROVIDERS.doubao.model}
            </option>
          </select>
          <div className="absolute right-3 top-2.5 pointer-events-none text-stone-400">
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M19 9l-7 7-7-7" strokeWidth="2" />
            </svg>
          </div>
        </div>
        {/* <div className="text-[10px] text-stone-400 font-semibold">
          Primary: {ASR_PROVIDERS[asrConfig.primary.provider].name}
          {" · "}
          {useRealtime ? "Realtime" : "HTTP"}
        </div> */}
      </div>

      <div className="space-y-3">
        <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
          快捷键
        </label>
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs text-stone-500">按住录音</span>
            <kbd className="px-2 py-1 bg-[var(--panel)] border border-[var(--stone)] rounded text-[10px] font-bold mono">
              {formatHotkeyDisplay(dualHotkeyConfig.dictation)}
            </kbd>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-xs text-stone-500">短按开关录音</span>
            <kbd className="px-2 py-1 bg-[var(--panel)] border border-[var(--stone)] rounded text-[10px] font-bold mono">
              {formatHotkeyKeysDisplay(releaseModeKeys)}
            </kbd>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-xs text-stone-500">按住唤起快捷助手</span>
            <kbd className="px-2 py-1 bg-[var(--panel)] border border-[var(--stone)] rounded text-[10px] font-bold mono">
              {formatHotkeyDisplay(dualHotkeyConfig.assistant)}
            </kbd>
          </div>
        </div>
      </div>

      <div className="space-y-3">
        {/* <div className="flex items-center justify-between">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            Polishing
          </label>
          <button
            onClick={() => onNavigate("llm")}
            className="text-[10px] font-bold text-[var(--steel)] hover:opacity-80 transition-opacity"
          >
            Presets
          </button>
        </div> */}
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              {/* <div className={["w-2 h-2 rounded-full", enablePostProcess ? "bg-[var(--crail)]" : "bg-stone-300"].join(" ")} /> */}
              <span className="text-xs font-bold">语句润色</span>
            </div>
          <Toggle
            checked={enablePostProcess}
            onCheckedChange={setEnablePostProcess}
            disabled={isRunning}
            size="sm"
            variant="indigo"
          />
          </div>
          <select
            value={llmConfig.active_preset_id}
            onChange={(e) => {
              const id = e.target.value;
              setLlmConfig((prev) => ({ ...prev, active_preset_id: id }));
            }}
            disabled={!enablePostProcess || isRunning}
            className="w-full text-[10px] font-bold text-stone-500 bg-[var(--paper)] rounded-lg px-2 py-2 outline-none border border-[var(--stone)] disabled:opacity-50"
          >
            {llmConfig.presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          {!llmConfig.api_key && enablePostProcess && (
            <div className="mt-3 text-[10px] font-bold text-amber-600">
              LLM API Key 未配置，请到 Presets 中设置
            </div>
          )}
        </div>
      </div>

      <div className="space-y-3">
        {/* <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
          Realtime
        </label> */}
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm flex items-center justify-between">
          <div>
            <div className="text-xs font-bold text-stone-700">
              {useRealtime ? "实时流式模式" : "HTTP 传统模式"}
            </div>
            <div className="text-[10px] text-stone-400 font-semibold">
              {useRealtime ? "边录边传，延迟更低" : "录完再传，更稳定"}
            </div>
          </div>
          <Toggle
            checked={useRealtime}
            onCheckedChange={setUseRealtime}
            disabled={isRunning}
            size="sm"
            variant="amber"
          />
        </div>
      </div>

      <div className="space-y-3">
        <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
          个人词库
        </label>
        <div className="relative group">
          <input
            type="text"
            value={newWord}
            onChange={(e) => setNewWord(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onAddWord();
            }}
            placeholder="输入并回车..."
            className="w-full bg-white border border-[var(--stone)] rounded-xl px-3 py-2 text-xs outline-none focus:border-[var(--steel)] shadow-sm"
          />
          <button
            onClick={onAddWord}
            className="absolute right-3 top-2 text-[var(--steel)] opacity-50 hover:opacity-100"
            title="添加"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>
        <div className="flex flex-wrap gap-1.5 pt-1">
          {dictionary.slice(0, 18).map((w) => (
            <span
              key={w}
              className="px-2 py-0.5 bg-stone-50 text-stone-500 rounded text-[10px] font-medium border border-stone-200"
            >
              {w}
            </span>
          ))}
        </div>
      </div>

      <div className="mt-auto text-center">
        <p className="text-[10px] text-stone-300 mono uppercase tracking-widest">
          PushToTalk
        </p>
      </div>
    </aside>
  );
}
