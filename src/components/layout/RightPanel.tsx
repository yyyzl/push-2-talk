import type { Dispatch, SetStateAction } from "react";
import { ArrowRight, HelpCircle, Plus } from "lucide-react";
import type {
  AsrConfig,
  DictionaryEntry,
  DualHotkeyConfig,
  HotkeyKey,
} from "../../types";
import type { AppPage } from "../../pages/types";
import {
  ASR_PROVIDERS,
  DEFAULT_OMNI_ASR_CONFIG,
  DEFAULT_OMNI_SHARED_CONFIG,
} from "../../constants";
import { formatHotkeyDisplay, formatHotkeyKeysDisplay } from "../../utils";
import { Toggle, Tooltip } from "../common";
import { useConfigSave } from "../../contexts/ConfigSaveContext";

// 首页词库最多显示的词条数（约两行）
const DICTIONARY_DISPLAY_LIMIT = 7;

export type RightPanelProps = {
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  dualHotkeyConfig: DualHotkeyConfig;

  dictionary: DictionaryEntry[];
  newWord: string;
  setNewWord: (next: string) => void;
  onAddWord: () => void;
  onNavigate: (page: AppPage) => void;

  isRunning: boolean;
};

export function RightPanel({
  asrConfig,
  setAsrConfig,
  dualHotkeyConfig,
  dictionary,
  newWord,
  setNewWord,
  onAddWord,
  onNavigate,
  isRunning,
}: RightPanelProps) {
  const releaseModeKeys =
    dualHotkeyConfig.dictation.release_mode_keys?.length
      ? dualHotkeyConfig.dictation.release_mode_keys
      : (["f2"] as HotkeyKey[]);

  const { saveImmediately } = useConfigSave();
  const omniConfig = asrConfig.omni ?? DEFAULT_OMNI_ASR_CONFIG;
  const omniSharedConfig = {
    ...DEFAULT_OMNI_SHARED_CONFIG,
    ...(asrConfig.omni_shared_config ?? {}),
  };

  return (
    <aside className="flex shrink-0 w-80 h-full min-h-0 bg-[var(--paper)] border-l border-[var(--stone)] flex-col p-5 gap-5 overflow-y-auto custom-scroll font-sans">
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            当前引擎
          </label>
        </div>
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm space-y-2">
          <div className="text-sm font-bold text-[var(--ink)]">
            {ASR_PROVIDERS.omni.name}
          </div>
          <div className="text-xs text-stone-500">
            {omniConfig.model}
            {omniSharedConfig.enable_thinking ? " · 深度思考" : ""}
          </div>
          <div className="text-[11px] text-stone-400 break-all">
            {omniConfig.endpoint}
          </div>
        </div>
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
        </div>
      </div>

      <div className="space-y-3">
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm space-y-3">
          <div className="flex items-center gap-2">
            <span className="text-xs font-bold">公共转录配置</span>
            <Tooltip content="作用于所有 Omni 服务商预设，不会因为切换连接配置而变化。">
              <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
            </Tooltip>
          </div>

          <div className="rounded-xl border border-dashed border-stone-200 bg-stone-50 px-3 py-2 text-[11px] text-stone-500">
            作用于所有 Omni 服务商预设
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-xs font-bold text-stone-700">深度思考模式</div>
              <div className="text-[11px] text-stone-400">
                {omniConfig.thinking_supported
                  ? (omniSharedConfig.enable_thinking ? "已启用" : "未启用")
                  : "当前预设暂不支持"}
              </div>
            </div>
            <div className={`px-2 py-1 rounded-full text-[10px] font-bold ${omniConfig.thinking_supported ? "bg-sky-50 text-sky-700 border border-sky-200" : "bg-stone-100 text-stone-400 border border-stone-200"}`}>
              {omniConfig.thinking_supported ? "Shared" : "Unsupported"}
            </div>
          </div>

          <div className="flex items-center justify-between">
              <div>
                <div className="text-xs font-bold text-stone-700">包含内置词库</div>
                <div className="text-[11px] text-stone-400">
                {omniSharedConfig.include_builtin_dictionary ? "已应用词库页所选领域并注入 Omni prompt" : "未应用词库页所选领域"}
                </div>
              </div>
            <Toggle
              checked={omniSharedConfig.include_builtin_dictionary}
              onCheckedChange={async (checked) => {
                const nextConfig = {
                  ...asrConfig,
                  omni_shared_config: {
                    ...omniSharedConfig,
                    include_builtin_dictionary: checked,
                  },
                };
                setAsrConfig(nextConfig);
                await saveImmediately({ asrConfig: nextConfig });
              }}
              disabled={isRunning}
              size="xs"
              variant="blue"
              aria-label="切换内置词库注入"
            />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-xs font-bold text-stone-700">焦点窗口截图</div>
              <div className="text-[11px] text-stone-400">
                {omniSharedConfig.include_focused_window_screenshot
                  ? "已将焦点窗口截图作为 Omni 辅助线索"
                  : "仅发送语音，不附带截图"}
              </div>
            </div>
            <div className={`px-2 py-1 rounded-full text-[10px] font-bold ${omniSharedConfig.include_focused_window_screenshot ? "bg-sky-50 text-sky-700 border border-sky-200" : "bg-stone-100 text-stone-400 border border-stone-200"}`}>
              {omniSharedConfig.include_focused_window_screenshot ? "Enabled" : "Disabled"}
            </div>
          </div>

        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            个人词库
          </label>
          <Tooltip content="添加专业术语、人名、地名等自定义词汇，提高语音识别准确率
          这些词条会直接参与 Omni prompt 构建。">
            <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
          </Tooltip>
        </div>
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
        <div className="flex flex-wrap gap-1.5 pt-1 items-center">
          {[...dictionary].reverse().slice(0, dictionary.length > DICTIONARY_DISPLAY_LIMIT ? DICTIONARY_DISPLAY_LIMIT - 1 : DICTIONARY_DISPLAY_LIMIT).map((entry) => (
            <span
              key={entry.id}
              className="px-2 py-0.5 bg-stone-50 text-stone-500 rounded text-[10px] font-medium border border-stone-200"
            >
              {entry.word}
            </span>
          ))}
          {dictionary.length > DICTIONARY_DISPLAY_LIMIT && (
            <button
              onClick={() => onNavigate("dictionary")}
              className="group flex items-center gap-0.5 px-2 py-0.5 bg-stone-200 hover:bg-stone-700 text-stone-600 hover:text-white rounded-full text-[10px] font-bold transition-all duration-200"
              title="查看全部词库"
            >
              <span className="tabular-nums">+{dictionary.length - DICTIONARY_DISPLAY_LIMIT + 1}</span>
              <ArrowRight className="w-3 h-3 opacity-60 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
            </button>
          )}
        </div>
      </div>

      <div className="mt-auto text-center">
        <p className="text-[10px] text-stone-300 mono uppercase tracking-widest">
          PushToTalk Omni
        </p>
      </div>
    </aside>
  );
}
