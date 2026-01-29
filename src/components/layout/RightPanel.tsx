import type { Dispatch, SetStateAction } from "react";
import { ArrowRight, Plus, HelpCircle } from "lucide-react";
import type {
  AsrConfig,
  AsrProvider,
  DictionaryEntry,
  DualHotkeyConfig,
  HotkeyKey,
  LlmConfig,
} from "../../types";
import type { AppPage } from "../../pages/types";
import { ASR_PROVIDERS } from "../../constants";
import { formatHotkeyDisplay, formatHotkeyKeysDisplay } from "../../utils";
import { Toggle, ConfigSelect, ConfigToggle, Tooltip } from "../common";
import { useConfigSave } from "../../contexts/ConfigSaveContext";

// 首页词库最多显示的词条数（约两行）
const DICTIONARY_DISPLAY_LIMIT = 7;

export type RightPanelProps = {
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  useRealtime: boolean;
  setUseRealtime: Dispatch<SetStateAction<boolean>>;

  enablePostProcess: boolean;
  setEnablePostProcess: Dispatch<SetStateAction<boolean>>;
  enableDictionaryEnhancement: boolean;
  setEnableDictionaryEnhancement: Dispatch<SetStateAction<boolean>>;
  llmConfig: LlmConfig;
  setLlmConfig: Dispatch<SetStateAction<LlmConfig>>;

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
  useRealtime,
  setUseRealtime,
  enablePostProcess,
  setEnablePostProcess,
  enableDictionaryEnhancement,
  setEnableDictionaryEnhancement,
  llmConfig,
  setLlmConfig,
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

  const { saveImmediately, syncStatus } = useConfigSave();

  return (
    <aside className="flex shrink-0 w-80 h-full min-h-0 bg-[var(--paper)] border-l border-[var(--stone)] flex-col p-5 gap-5 overflow-y-auto custom-scroll font-sans">
      {/* ASR 引擎选择 */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            语音识别引擎
          </label>
        </div>
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
            {
              value: "qwen" as AsrProvider,
              label: `${ASR_PROVIDERS.qwen.name} · ${ASR_PROVIDERS.qwen.model}`,
            },
            {
              value: "doubao" as AsrProvider,
              label: `${ASR_PROVIDERS.doubao.name} · ${ASR_PROVIDERS.doubao.model}`,
            },
          ]}
        />
      </div>

      {/* 快捷键显示 */}
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

      {/* 语句润色（热更新，不需要重启服务） */}
      <div className="space-y-3">
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <span className="text-xs font-bold">语句润色</span>
              <Tooltip content="使用 AI 对识别结果进行智能优化，如纠错、润色、翻译等">
                <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
              </Tooltip>
            </div>
            <Toggle
              checked={enablePostProcess}
              onCheckedChange={setEnablePostProcess}
              disabled={isRunning}
              size="sm"
              variant="orange"
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
          {/* 虚线分割线 */}
          <div className="my-3 border-t border-dashed border-stone-200" />
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-bold text-stone-700">词库增强</span>
              <Tooltip content="将个人词库注入提示词，用于同音词纠错与专业术语优先匹配；可独立于语句润色开关单独生效（仍会调用 LLM）">
                <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
              </Tooltip>
            </div>
            <Toggle
              checked={enableDictionaryEnhancement}
              onCheckedChange={setEnableDictionaryEnhancement}
              disabled={isRunning}
              size="sm"
              variant="orange"
            />
          </div>
          {!llmConfig.shared.api_key && (enablePostProcess || enableDictionaryEnhancement) && (
            <div className="mt-3 text-[10px] font-bold text-amber-600">
              LLM API Key 未配置，请到 Presets 中设置
            </div>
          )}
        </div>
      </div>

      {/* 实时/HTTP 模式切换（需要重启服务） */}
      <div className="space-y-3">
        <div className="bg-white border border-[var(--stone)] rounded-2xl p-4 shadow-sm flex items-center justify-between">
          <div>
            <div className="flex items-center gap-1.5">
              <div className="text-xs font-bold text-stone-700">
                {useRealtime ? "实时流式模式" : "HTTP模式"}
              </div>
              <Tooltip content="HTTP模式: 录完后一次性上传音频文件，网络不稳定时更可靠。语音较长时，识别较慢
              实时流式模式: 边录制边上传，网络不稳定时可能会丢失部分结果。语音较长时，识别较快">
                <HelpCircle className="w-3.5 h-3.5 text-stone-400 hover:text-stone-600 transition-colors cursor-help" />
              </Tooltip>
            </div>
            <div className="text-[10px] text-stone-400 font-semibold">
              {useRealtime ? "边录边传，延迟更低" : "录完再传，更稳定"}
            </div>
          </div>
          <ConfigToggle
            checked={useRealtime}
            onCheckedChange={(checked) => {
              // onChange 只负责乐观更新 UI
              setUseRealtime(checked);
            }}
            onCommit={async (checked) => {
              // onCommit 只负责保存，不重复触发 setState
              await saveImmediately({ useRealtime: checked });
            }}
            syncStatus={syncStatus}
            disabled={isRunning}
            size="sm"
            variant="amber"
          />
        </div>
      </div>

      {/* 个人词库 */}
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <label className="text-[10px] font-bold text-stone-400 uppercase tracking-widest">
            个人词库
          </label>
          <Tooltip content="添加专业术语、人名、地名等自定义词汇，提高语音识别准确率
          备注：豆包实时流式模式下不生效">
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
          PushToTalk
        </p>
      </div>
    </aside>
  );
}
