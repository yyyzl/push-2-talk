import { invoke } from "@tauri-apps/api/core";
import type { Dispatch, SetStateAction } from "react";
import {
  AlertCircle,
  BookText,
  Download,
  Keyboard,
  MessageSquareQuote,
  Mic,
  Power,
  RefreshCw,
  RotateCcw,
  Settings,
  VolumeX,
  Wand2,
  X,
} from "lucide-react";
import type {
  AsrConfig,
  AssistantConfig,
  DualHotkeyConfig,
  HotkeyKey,
  HotkeyRecordingMode,
  LlmConfig,
  LlmPreset,
  ServiceModalTab,
} from "../../types";
import { ASR_PROVIDERS, DEFAULT_SMART_COMMAND_CONFIG, KEY_DISPLAY_NAMES } from "../../constants";
import { formatHotkeyDisplay, isAsrConfigValid } from "../../utils";
import { Toggle } from "../common";

export type SettingsModalUpdateInfo = { version: string; notes?: string };

export type SettingsModalProps = {
  open: boolean;
  onClose: () => void;

  apiKey: string;
  fallbackApiKey: string;
  useRealtime: boolean;
  enablePostProcess: boolean;

  status: "idle" | "running" | "recording" | "transcribing";

  isRecordingHotkey: boolean;
  setIsRecordingHotkey: (next: boolean) => void;
  recordingMode: HotkeyRecordingMode;
  setRecordingMode: (next: HotkeyRecordingMode) => void;
  recordingKeys: HotkeyKey[];
  hotkeyError: string | null;
  dualHotkeyConfig: DualHotkeyConfig;
  setDualHotkeyConfig: Dispatch<SetStateAction<DualHotkeyConfig>>;
  resetHotkeyToDefault: (mode: "dictation" | "assistant" | "release") => void;

  asrConfig: AsrConfig;
  llmConfig: LlmConfig;
  activePreset: LlmPreset;
  assistantConfig: AssistantConfig;
  dictionary: string[];

  setServiceModalTab: (tab: ServiceModalTab) => void;
  setShowServiceModal: (next: boolean) => void;

  enableAutostart: boolean;
  handleAutostartToggle: () => void;

  enableMuteOtherApps: boolean;
  setEnableMuteOtherApps: Dispatch<SetStateAction<boolean>>;

  updateStatus: "idle" | "checking" | "available" | "downloading" | "ready";
  updateInfo: SettingsModalUpdateInfo | null;
  currentVersion: string;
  handleCheckUpdate: () => void;
  setShowUpdateModal: (next: boolean) => void;
};

export function SettingsModal({
  open,
  onClose,
  apiKey,
  fallbackApiKey,
  useRealtime,
  enablePostProcess,
  status,
  isRecordingHotkey,
  setIsRecordingHotkey,
  recordingMode,
  setRecordingMode,
  recordingKeys,
  hotkeyError,
  dualHotkeyConfig,
  setDualHotkeyConfig,
  resetHotkeyToDefault,
  asrConfig,
  llmConfig,
  activePreset,
  assistantConfig,
  dictionary,
  setServiceModalTab,
  setShowServiceModal,
  enableAutostart,
  handleAutostartToggle,
  enableMuteOtherApps,
  setEnableMuteOtherApps,
  updateStatus,
  updateInfo,
  currentVersion,
  handleCheckUpdate,
  setShowUpdateModal,
}: SettingsModalProps) {
  if (!open) return null;

  const smartCommandConfig = DEFAULT_SMART_COMMAND_CONFIG;

  return (

            <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
              {/* 增加 max-h-[85vh] 确保上下有留白，flex flex-col 支持内部滚动 */}
              <div className="bg-white rounded-3xl border border-[var(--stone)] shadow-2xl w-full max-w-sm mx-4 max-h-[85vh] flex flex-col overflow-hidden animate-in zoom-in-95 duration-200">
                {/* Modal Header - 与其他弹窗风格一致 */}
                <div className="px-6 py-4 border-b border-[var(--stone)] flex items-center justify-between bg-[var(--paper)] flex-shrink-0">
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-[var(--panel)] border border-[var(--stone)] rounded-xl text-stone-600">
                      <Settings size={20} />
                    </div>
                    <div>
                      <h3 className="text-lg font-bold text-slate-900">设置</h3>
                      <p className="text-xs text-slate-500">应用偏好设置</p>
                    </div>
                  </div>
                  <button
                    onClick={() => onClose()}
                    className="p-2 hover:bg-slate-200 rounded-lg text-slate-500 hover:text-slate-700 transition-colors"
                  >
                    <X size={18} />
                  </button>
                </div>
                {/* Modal Body - flex-1 和 overflow-y-auto 启用内部滚动 */}
                <div className="p-4 space-y-4 overflow-y-auto flex-1">
                  {/* 快捷键配置 - 双模式 */}
                  <div className="p-4 bg-slate-50/80 rounded-xl border border-slate-100">
                    <div className="flex items-center gap-3 mb-3">
                      <div className={`p-2 rounded-lg transition-colors ${
                        isRecordingHotkey ? 'bg-blue-100 text-blue-600' : 'bg-slate-100 text-slate-400'
                      }`}>
                        <Keyboard size={18} />
                      </div>
                      <div>
                        <div className="text-sm font-medium text-slate-700">快捷键（双模式）</div>
                        <div className="text-xs text-slate-400">
                          点击下方区域录制新的快捷键组合
                        </div>
                      </div>
                    </div>
                    {/* 听写模式快捷键 - 并排显示长按和松手模式 */}
                    <div className="space-y-2 mb-3">
                      <label className="text-xs font-medium text-slate-600">听写模式（语音转文字）</label>
                      {/* 并排的两个快捷键配置 */}
                      <div className="grid grid-cols-2 gap-2">
                        {/* 左侧：长按录音 */}
                        <div className="space-y-1">
                          <div className="text-[10px] text-slate-400 pl-1">长按录音</div>
                          <div
                            onClick={() => {
                              if (status === 'idle') {
                                setRecordingMode('dictation');
                                setIsRecordingHotkey(true);
                              }
                            }}
                            className={`flex items-center gap-1 p-2 bg-white border rounded-lg cursor-pointer transition-all min-h-[40px] ${
                              isRecordingHotkey && recordingMode === 'dictation'
                                ? 'border-blue-500 ring-2 ring-blue-200'
                                : 'border-slate-200 hover:border-slate-300'
                            } ${status !== 'idle' ? 'opacity-50 cursor-not-allowed' : ''}`}
                          >
                            <div className="flex-1 flex flex-wrap gap-1">
                              {isRecordingHotkey && recordingMode === 'dictation' ? (
                                recordingKeys.length > 0 ? (
                                  recordingKeys.map(key => (
                                    <span key={key} className="px-1.5 py-0.5 bg-blue-100 text-blue-700 text-[10px] font-medium rounded">
                                      {KEY_DISPLAY_NAMES[key]}
                                    </span>
                                  ))
                                ) : (
                                  <span className="text-xs text-blue-600 animate-pulse">按下...</span>
                                )
                              ) : (
                                dualHotkeyConfig.dictation.keys.map(key => (
                                  <span key={key} className="px-1.5 py-0.5 bg-slate-100 text-slate-700 text-[10px] font-medium rounded">
                                    {KEY_DISPLAY_NAMES[key]}
                                  </span>
                                ))
                              )}
                            </div>
                            <button
                              onClick={(e) => { e.stopPropagation(); resetHotkeyToDefault('dictation'); }}
                              disabled={status !== 'idle'}
                              className="p-1 text-slate-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors disabled:opacity-50"
                              title="重置为默认"
                            >
                              <RotateCcw size={12} />
                            </button>
                          </div>
                        </div>
                        {/* 右侧：松手模式 */}
                        <div className="space-y-1">
                          <div className="text-[10px] text-slate-400 pl-1">松手模式 <span className="text-blue-500">省力版</span></div>
                          <div
                            onClick={() => {
                              if (status === 'idle') {
                                setRecordingMode('release');
                                setIsRecordingHotkey(true);
                              }
                            }}
                            className={`flex items-center gap-1 p-2 bg-white border rounded-lg cursor-pointer transition-all min-h-[40px] ${
                              isRecordingHotkey && recordingMode === 'release'
                                ? 'border-blue-500 ring-2 ring-blue-200'
                                : 'border-slate-200 hover:border-slate-300'
                            } ${status !== 'idle' ? 'opacity-50 cursor-not-allowed' : ''}`}
                          >
                            <div className="flex-1 flex flex-wrap gap-1">
                              {isRecordingHotkey && recordingMode === 'release' ? (
                                recordingKeys.length > 0 ? (
                                  recordingKeys.map(key => (
                                    <span key={key} className="px-1.5 py-0.5 bg-blue-100 text-blue-700 text-[10px] font-medium rounded">
                                      {KEY_DISPLAY_NAMES[key]}
                                    </span>
                                  ))
                                ) : (
                                  <span className="text-xs text-blue-600 animate-pulse">按下...</span>
                                )
                              ) : (
                                (dualHotkeyConfig.dictation.release_mode_keys || ['f2']).map(key => (
                                  <span key={key} className="px-1.5 py-0.5 bg-blue-50 text-blue-700 text-[10px] font-medium rounded">
                                    {KEY_DISPLAY_NAMES[key as HotkeyKey] || key}
                                  </span>
                                ))
                              )}
                            </div>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                // 重置松手模式为默认 F2
                                const newConfig = {
                                  ...dualHotkeyConfig,
                                  dictation: {
                                    ...dualHotkeyConfig.dictation,
                                    release_mode_keys: ['f2'] as HotkeyKey[]
                                  }
                                };
                                setDualHotkeyConfig(newConfig);
                                invoke("save_config", {
                                  apiKey,
                                  fallbackApiKey,
                                  useRealtime,
                                  enablePostProcess,
                                  llmConfig,
                                  smartCommandConfig,
                                  assistantConfig,
                                  asrConfig,
                                  dualHotkeyConfig: newConfig,
                                });
                              }}
                              disabled={status !== 'idle'}
                              className="p-1 text-slate-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors disabled:opacity-50"
                              title="重置为默认 F2"
                            >
                              <RotateCcw size={12} />
                            </button>
                          </div>
                        </div>
                      </div>
                      {/* 说明文字 */}
                      <p className="text-[10px] text-slate-400 pl-1">
                        松手模式：按一下开始录音，再按一下或点击悬浮窗按钮完成
                      </p>
                    </div>
                    {/* AI 助手模式快捷键 */}
                    <div className="space-y-2">
                      <label className="text-xs font-medium text-slate-600">AI 助手模式（智能处理）</label>
                      <div
                        onClick={() => {
                          if (status === 'idle') {
                            setRecordingMode('assistant');
                            setIsRecordingHotkey(true);
                          }
                        }}
                        className={`flex items-center gap-2 p-3 bg-white border rounded-xl cursor-pointer transition-all min-h-[44px] ${
                          isRecordingHotkey && recordingMode === 'assistant'
                            ? 'border-blue-500 ring-2 ring-blue-200'
                            : 'border-slate-200 hover:border-slate-300'
                        } ${status !== 'idle' ? 'opacity-50 cursor-not-allowed' : ''}`}
                      >
                        <div className="flex-1 flex flex-wrap gap-1.5">
                          {isRecordingHotkey && recordingMode === 'assistant' ? (
                            recordingKeys.length > 0 ? (
                              recordingKeys.map(key => (
                                <span key={key} className="px-2.5 py-1 bg-blue-100 text-blue-700 text-xs font-medium rounded-md">
                                  {KEY_DISPLAY_NAMES[key]}
                                </span>
                              ))
                            ) : (
                              <span className="text-sm text-blue-600 animate-pulse">按下快捷键...</span>
                            )
                          ) : (
                            dualHotkeyConfig.assistant.keys.map(key => (
                              <span key={key} className="px-2.5 py-1 bg-slate-100 text-slate-700 text-xs font-medium rounded-md">
                                {KEY_DISPLAY_NAMES[key]}
                              </span>
                            ))
                          )}
                        </div>
                        <button
                          onClick={(e) => { e.stopPropagation(); resetHotkeyToDefault('assistant'); }}
                          disabled={status !== 'idle'}
                          className="p-1.5 text-slate-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors disabled:opacity-50"
                          title="重置为默认 (Alt+Space)"
                        >
                          <RotateCcw size={14} />
                        </button>
                      </div>
                    </div>
                    {hotkeyError && (
                      <div className="mt-2 flex items-center gap-1.5 p-2 bg-red-50 border border-red-100 rounded-lg text-xs text-red-600 animate-in slide-in-from-top-2 fade-in duration-200">
                        <AlertCircle size={12} className="flex-shrink-0" />
                        <span>{hotkeyError}</span>
                      </div>
                    )}
                    {status !== 'idle' && (
                      <div className="mt-2 flex items-center gap-1.5 text-xs text-amber-600">
                        <AlertCircle size={12} />
                        <span>请先停止服务后再修改快捷键</span>
                      </div>
                    )}
                  </div>
                  {/* 服务配置入口 - 分组列表风格，与系统设置统一 */}
                  <div className="space-y-2">
                    <div className="text-xs font-medium text-slate-500 px-1">服务配置</div>
                    <div className="bg-slate-50/80 rounded-xl border border-slate-100 divide-y divide-slate-100 overflow-hidden">
                      {/* ASR 语音识别 */}
                      <button
                        onClick={() => {
                          onClose();
                          setServiceModalTab('asr');
                          setShowServiceModal(true);
                        }}
                        className="w-full flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-all"
                      >
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            isAsrConfigValid(asrConfig.primary) ? 'bg-blue-100 text-blue-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <Mic size={16} />
                          </div>
                          <div className="text-left">
                            <div className="text-sm font-medium text-slate-700">ASR 语音识别</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {isAsrConfigValid(asrConfig.primary)
                                ? `${ASR_PROVIDERS[asrConfig.primary.provider].name} · 已配置`
                                : '未配置'}
                            </div>
                          </div>
                        </div>
                        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" className="text-slate-300">
                          <path d="M1 1L5 5L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                      </button>
                      {/* LLM 润色 */}
                      <button
                        onClick={() => {
                          onClose();
                          setServiceModalTab('llm');
                          setShowServiceModal(true);
                        }}
                        className="w-full flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-all"
                      >
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            llmConfig.api_key ? 'bg-violet-100 text-violet-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <Wand2 size={16} />
                          </div>
                          <div className="text-left">
                            <div className="text-sm font-medium text-slate-700">LLM 润色</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {llmConfig.api_key
                                ? `${activePreset?.name || '文本润色'} · 已配置`
                                : '未配置（可选）'}
                            </div>
                          </div>
                        </div>
                        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" className="text-slate-300">
                          <path d="M1 1L5 5L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                      </button>
                      {/* AI 助手 */}
                      <button
                        onClick={() => {
                          onClose();
                          setServiceModalTab('assistant');
                          setShowServiceModal(true);
                        }}
                        className="w-full flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-all"
                      >
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            assistantConfig.api_key ? 'bg-emerald-100 text-emerald-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <MessageSquareQuote size={16} />
                          </div>
                          <div className="text-left">
                            <div className="text-sm font-medium text-slate-700">AI 助手</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {assistantConfig.api_key
                                ? `${formatHotkeyDisplay(dualHotkeyConfig.assistant)} · 已配置`
                                : '未配置（可选）'}
                            </div>
                          </div>
                        </div>
                        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" className="text-slate-300">
                          <path d="M1 1L5 5L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                      </button>
                      {/* 个人词典 */}
                      <button
                        onClick={() => {
                          onClose();
                          setServiceModalTab('dictionary');
                          setShowServiceModal(true);
                        }}
                        className="w-full flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-all"
                      >
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            dictionary.length > 0 ? 'bg-orange-100 text-orange-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <BookText size={16} />
                          </div>
                          <div className="text-left">
                            <div className="text-sm font-medium text-slate-700">个人词典</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {dictionary.length > 0 ? `${dictionary.length} 个词条` : '未添加词条'}
                            </div>
                          </div>
                        </div>
                        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" className="text-slate-300">
                          <path d="M1 1L5 5L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                      </button>
                    </div>
                  </div>
                  {/* 系统设置分组 */}
                  <div className="space-y-2">
                    <div className="text-xs font-medium text-slate-500 px-1">系统设置</div>
                    <div className="bg-slate-50/80 rounded-xl border border-slate-100 divide-y divide-slate-100 overflow-hidden">
                      {/* 开机自启动 */}
                      <div className="flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-colors">
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            enableAutostart ? 'bg-green-100 text-green-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <Power size={16} />
                          </div>
                          <div>
                            <div className="text-sm font-medium text-slate-700">开机自启动</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {enableAutostart ? '随系统启动' : '手动启动'}
                            </div>
                          </div>
                        </div>
                        <Toggle
                          checked={enableAutostart}
                          onCheckedChange={() => { void handleAutostartToggle(); }}
                          size="sm"
                          variant="green"
                        />
                      </div>
                      {/* 录音时静音其他应用 */}
                      <div className="flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-colors">
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            enableMuteOtherApps ? 'bg-orange-100 text-orange-600' : 'bg-slate-200 text-slate-500'
                          }`}>
                            <VolumeX size={16} />
                          </div>
                          <div>
                            <div className="text-sm font-medium text-slate-700">录音时静音其他应用</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {enableMuteOtherApps ? '录音期间自动静音' : '不干预音频'}
                            </div>
                          </div>
                        </div>
                        <Toggle
                          checked={enableMuteOtherApps}
                          onCheckedChange={setEnableMuteOtherApps}
                          disabled={status === "recording" || status === "transcribing"}
                          size="sm"
                          variant="orange"
                        />
                      </div>
                      {/* 检查更新 */}
                      <button
                        onClick={() => {
                          if (updateStatus === "available") {
                            onClose();
                            setShowUpdateModal(true);
                          } else {
                            handleCheckUpdate();
                          }
                        }}
                        disabled={updateStatus === "checking" || updateStatus === "downloading"}
                        className="w-full flex items-center justify-between p-3.5 hover:bg-slate-100/80 transition-all text-left disabled:opacity-60"
                      >
                        <div className="flex items-center gap-3">
                          <div className={`p-1.5 rounded-lg transition-colors ${
                            updateStatus === "available"
                              ? 'bg-green-100 text-green-600'
                              : updateStatus === "checking"
                              ? 'bg-blue-100 text-blue-600'
                              : 'bg-slate-200 text-slate-500'
                          }`}>
                            {updateStatus === "checking" ? (
                              <RefreshCw size={16} className="animate-spin" />
                            ) : (
                              <Download size={16} />
                            )}
                          </div>
                          <div>
                            <div className="text-sm font-medium text-slate-700">检查更新</div>
                            <div className="text-[11px] text-slate-400 leading-tight">
                              {updateStatus === "available" && updateInfo
                                ? `发现新版本 v${updateInfo.version}`
                                : updateStatus === "checking"
                                ? '正在连接服务器...'
                                : `当前版本 v${currentVersion}`}
                            </div>
                          </div>
                        </div>
                        <div className="flex items-center gap-2">
                          {updateStatus === "available" && (
                            <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
                          )}
                          <svg width="6" height="10" viewBox="0 0 6 10" fill="none" className="text-slate-300">
                            <path d="M1 1L5 5L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                          </svg>
                        </div>
                      </button>
                    </div>
                  </div>
                </div>
                {/* Modal Footer */}
                <div className="px-4 py-3 bg-[var(--paper)] border-t border-[var(--stone)] flex-shrink-0">
                  <p className="text-xs text-slate-400 text-center">
                    听写: {formatHotkeyDisplay(dualHotkeyConfig.dictation)} · AI助手: {formatHotkeyDisplay(dualHotkeyConfig.assistant)}
                  </p>
                </div>
              </div>
            </div>

  );
}
