import { Keyboard, Mic, MessageSquare, RotateCcw, ToggleLeft } from "lucide-react";
import type { AppStatus, DualHotkeyConfig, HotkeyKey, HotkeyRecordingMode } from "../types";
import { formatHotkeyDisplay, formatHotkeyKeysDisplay } from "../utils";

export type HotkeysPageProps = {
  status: AppStatus;
  isRecordingHotkey: boolean;
  setIsRecordingHotkey: (next: boolean) => void;
  recordingMode: HotkeyRecordingMode;
  setRecordingMode: (next: HotkeyRecordingMode) => void;
  recordingKeys: HotkeyKey[];
  hotkeyError: string | null;
  dualHotkeyConfig: DualHotkeyConfig;
  resetHotkeyToDefault: (mode: "dictation" | "assistant" | "release") => void;
};

const RenderKeys = ({ text, isRecording }: { text: string; isRecording?: boolean }) => {
  if (isRecording) {
    return (
      <span className="inline-flex items-center px-3 py-1 rounded-md bg-[var(--crail)] text-white text-xs font-bold animate-pulse">
        请按下按键...
      </span>
    );
  }

  if (!text) return <span className="text-stone-400 text-xs">未设置</span>;

  // Split by " + " or just "+"
  const parts = text.split(/\s*\+\s*/);
  return (
    <div className="flex items-center gap-1 flex-wrap">
      {parts.map((part, i) => (
        <div key={i} className="flex items-center">
          <kbd className="kbd-shortcut">{part}</kbd>
          {i < parts.length - 1 && <span className="text-stone-400 mx-0.5 text-xs">+</span>}
        </div>
      ))}
    </div>
  );
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

  return (
    <div className="mx-auto max-w-2xl font-sans space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div className="bg-white border border-[var(--stone)] rounded-2xl overflow-hidden shadow-sm">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[var(--stone)] bg-[var(--paper)] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-[var(--stone)] rounded-lg text-[var(--ink)]">
              <Keyboard size={20} />
            </div>
            <div>
              <h2 className="text-base font-bold text-[var(--ink)]">快捷键映射</h2>
              <p className="text-xs text-stone-500 font-medium mt-0.5">自定义全局快捷键以适应你的工作流</p>
            </div>
          </div>
        </div>

        {hotkeyError && (
          <div className="px-6 py-3 bg-red-50 border-b border-red-100 text-xs font-bold text-red-600 flex items-center gap-2">
            <span>⚠️</span> {hotkeyError}
          </div>
        )}

        {/* List */}
        <div className="divide-y divide-[var(--stone)]">
          {/* Dictation */}
          <div className="p-5 flex items-center justify-between group hover:bg-[var(--paper)] transition-colors">
            <div className="flex items-center gap-4">
              <div className="w-10 h-10 rounded-full bg-blue-50 text-blue-600 flex items-center justify-center border border-blue-100">
                <Mic size={20} />
              </div>
              <div>
                <div className="text-sm font-bold text-[var(--ink)]">语音听写</div>
                <div className="mt-1.5">
                  <RenderKeys
                    text={
                      isRecordingHotkey && recordingMode === "dictation"
                        ? recordingKeys.join(" + ") // Show live keys if recording this one
                        : formatHotkeyDisplay(dualHotkeyConfig.dictation)
                    }
                    isRecording={isRecordingHotkey && recordingMode === "dictation"}
                  />
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {isRecordingHotkey && recordingMode === "dictation" ? (
                <button
                  onClick={() => setIsRecordingHotkey(false)}
                  className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-xs font-bold hover:bg-stone-50"
                >
                  取消
                </button>
              ) : (
                <>
                  <button
                    onClick={() => {
                      setRecordingMode("dictation");
                      setIsRecordingHotkey(true);
                    }}
                    disabled={!canRecord}
                    className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-[var(--ink)] text-xs font-bold hover:border-stone-400 disabled:opacity-50 transition-colors"
                  >
                    录制
                  </button>
                  <button
                    onClick={() => resetHotkeyToDefault("dictation")}
                    disabled={!canRecord}
                    className="p-1.5 rounded-lg text-stone-400 hover:text-[var(--ink)] hover:bg-stone-100 disabled:opacity-30 transition-colors"
                    title="恢复默认"
                  >
                    <RotateCcw size={16} />
                  </button>
                </>
              )}
            </div>
          </div>

          {/* Assistant */}
          <div className="p-5 flex items-center justify-between group hover:bg-[var(--paper)] transition-colors">
            <div className="flex items-center gap-4">
              <div className="w-10 h-10 rounded-full bg-purple-50 text-purple-600 flex items-center justify-center border border-purple-100">
                <MessageSquare size={20} />
              </div>
              <div>
                <div className="text-sm font-bold text-[var(--ink)]">AI 助手</div>
                <div className="mt-1.5">
                  <RenderKeys
                    text={
                      isRecordingHotkey && recordingMode === "assistant"
                        ? recordingKeys.join(" + ")
                        : formatHotkeyDisplay(dualHotkeyConfig.assistant)
                    }
                    isRecording={isRecordingHotkey && recordingMode === "assistant"}
                  />
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {isRecordingHotkey && recordingMode === "assistant" ? (
                <button
                  onClick={() => setIsRecordingHotkey(false)}
                  className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-xs font-bold hover:bg-stone-50"
                >
                  取消
                </button>
              ) : (
                <>
                  <button
                    onClick={() => {
                      setRecordingMode("assistant");
                      setIsRecordingHotkey(true);
                    }}
                    disabled={!canRecord}
                    className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-[var(--ink)] text-xs font-bold hover:border-stone-400 disabled:opacity-50 transition-colors"
                  >
                    录制
                  </button>
                  <button
                    onClick={() => resetHotkeyToDefault("assistant")}
                    disabled={!canRecord}
                    className="p-1.5 rounded-lg text-stone-400 hover:text-[var(--ink)] hover:bg-stone-100 disabled:opacity-30 transition-colors"
                    title="恢复默认"
                  >
                    <RotateCcw size={16} />
                  </button>
                </>
              )}
            </div>
          </div>

          {/* Release Mode */}
          <div className="p-5 flex items-center justify-between group hover:bg-[var(--paper)] transition-colors">
            <div className="flex items-center gap-4">
              <div className="w-10 h-10 rounded-full bg-orange-50 text-orange-600 flex items-center justify-center border border-orange-100">
                <ToggleLeft size={20} />
              </div>
              <div>
                <div className="text-sm font-bold text-[var(--ink)]">
                  短按开关录音
                </div>
                <div className="mt-1.5">
                  <RenderKeys
                    text={
                      isRecordingHotkey && recordingMode === "release"
                        ? recordingKeys.join(" + ")
                        : formatHotkeyKeysDisplay(releaseModeKeys)
                    }
                    isRecording={isRecordingHotkey && recordingMode === "release"}
                  />
                </div>
                <div className="text-[10px] text-stone-400 mt-1 font-medium">
                  按一下开始，再按一下结束
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {isRecordingHotkey && recordingMode === "release" ? (
                <button
                  onClick={() => setIsRecordingHotkey(false)}
                  className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-xs font-bold hover:bg-stone-50"
                >
                  取消
                </button>
              ) : (
                <>
                  <button
                    onClick={() => {
                      setRecordingMode("release");
                      setIsRecordingHotkey(true);
                    }}
                    disabled={!canRecord}
                    className="px-3 py-1.5 rounded-lg border border-[var(--stone)] bg-white text-[var(--ink)] text-xs font-bold hover:border-stone-400 disabled:opacity-50 transition-colors"
                  >
                    录制
                  </button>
                  <button
                    onClick={() => resetHotkeyToDefault("release")}
                    disabled={!canRecord}
                    className="p-1.5 rounded-lg text-stone-400 hover:text-[var(--ink)] hover:bg-stone-100 disabled:opacity-30 transition-colors"
                    title="恢复默认"
                  >
                    <RotateCcw size={16} />
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
