import { Keyboard, RotateCcw } from "lucide-react";
import type { DualHotkeyConfig, HotkeyKey, HotkeyRecordingMode } from "../types";
import { formatHotkeyDisplay, formatHotkeyKeysDisplay } from "../utils";

export type HotkeysPageProps = {
  status: "idle" | "running" | "recording" | "transcribing";

  isRecordingHotkey: boolean;
  setIsRecordingHotkey: (next: boolean) => void;
  recordingMode: HotkeyRecordingMode;
  setRecordingMode: (next: HotkeyRecordingMode) => void;
  recordingKeys: HotkeyKey[];
  hotkeyError: string | null;
  dualHotkeyConfig: DualHotkeyConfig;
  resetHotkeyToDefault: (mode: "dictation" | "assistant" | "release") => void;
};

export function HotkeysPage({
  status,
  isRecordingHotkey,
  setIsRecordingHotkey,
  recordingMode,
  setRecordingMode,
  recordingKeys,
  hotkeyError,
  dualHotkeyConfig,
  resetHotkeyToDefault,
}: HotkeysPageProps) {
  const isConfigLocked = status === "recording" || status === "transcribing";
  const canRecord = !isConfigLocked && !isRecordingHotkey;
  const releaseModeKeys =
    dualHotkeyConfig.dictation.release_mode_keys?.length
      ? dualHotkeyConfig.dictation.release_mode_keys
      : (["f2"] as HotkeyKey[]);

  const recordingTargetLabel =
    recordingMode === "assistant"
      ? "AI 助手"
      : recordingMode === "release"
        ? "短按开关录音"
        : "听写";

  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-6">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <Keyboard size={14} />
          <span>快捷键映射</span>
        </div>

        {hotkeyError && (
          <div className="px-4 py-3 bg-red-50 border border-red-100 rounded-2xl text-sm font-semibold text-red-700">
            {hotkeyError}
          </div>
        )}

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-2xl p-4 space-y-2">
            <div className="text-xs font-bold text-stone-500 uppercase tracking-widest">听写</div>
            <div className="text-sm font-bold text-[var(--ink)] mono">
              {formatHotkeyDisplay(dualHotkeyConfig.dictation)}
            </div>
            <div className="flex items-center gap-2 pt-2">
              <button
                onClick={() => {
                  setRecordingMode("dictation");
                  setIsRecordingHotkey(true);
                }}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50"
              >
                录制
              </button>
              <button
                onClick={() => resetHotkeyToDefault("dictation")}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-600 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                <RotateCcw size={14} />
                默认
              </button>
            </div>
          </div>

          <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-2xl p-4 space-y-2">
            <div className="text-xs font-bold text-stone-500 uppercase tracking-widest">AI 助手</div>
            <div className="text-sm font-bold text-[var(--ink)] mono">
              {formatHotkeyDisplay(dualHotkeyConfig.assistant)}
            </div>
            <div className="flex items-center gap-2 pt-2">
              <button
                onClick={() => {
                  setRecordingMode("assistant");
                  setIsRecordingHotkey(true);
                }}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50"
              >
                录制
              </button>
              <button
                onClick={() => resetHotkeyToDefault("assistant")}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-600 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                <RotateCcw size={14} />
                默认
              </button>
            </div>
          </div>

          <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-2xl p-4 space-y-2 sm:col-span-2">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-xs font-bold text-stone-500 uppercase tracking-widest">
                  短按开关录音
                </div>
                <div className="text-[11px] text-stone-400 font-semibold">
                  按一下开始录音，再按一下或点击悬浮条完成
                </div>
              </div>
              <div className="text-sm font-bold text-[var(--ink)] mono text-right">
                {formatHotkeyKeysDisplay(releaseModeKeys)}
              </div>
            </div>
            <div className="flex items-center gap-2 pt-2">
              <button
                onClick={() => {
                  setRecordingMode("release");
                  setIsRecordingHotkey(true);
                }}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-700 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50"
              >
                录制
              </button>
              <button
                onClick={() => resetHotkeyToDefault("release")}
                disabled={!canRecord}
                className="px-3 py-2 rounded-xl bg-white border border-[var(--stone)] text-stone-600 font-bold hover:border-[rgba(176,174,165,0.75)] transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                <RotateCcw size={14} />
                默认
              </button>
            </div>
          </div>
        </div>

        {isRecordingHotkey && (
          <div className="px-4 py-3 bg-[var(--panel)] border border-[var(--stone)] rounded-2xl text-sm font-semibold text-[var(--ink)]">
            <div className="text-xs font-bold text-stone-500 uppercase tracking-widest mb-1">
              正在录制：{recordingTargetLabel}
            </div>
            <div className="mono">{recordingKeys.length ? recordingKeys.join(" + ") : "请按下组合键..."}</div>
          </div>
        )}
      </div>
    </div>
  );
}
