// src/App.tsx

import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Mic,
  StopCircle,
  Settings,
  Key,
  Activity,
  CheckCircle2,
  AlertCircle,
  Eye,
  EyeOff,
  Cpu,
  Sparkles,
  Zap,
  Globe,
  XCircle,
  Wand2,
  X,
  RotateCcw,
  Plus,
  Trash2,
  MessageSquareQuote,
  History,
  Copy,
  Clock
} from "lucide-react";
import { nanoid } from 'nanoid';

// --- 新的接口定义 ---

interface LlmPreset {
  id: string;
  name: string;
  system_prompt: string;
}

interface LlmConfig {
  endpoint: string;
  model: string;
  api_key: string;
  presets: LlmPreset[];
  active_preset_id: string;
}

interface AppConfig {
  dashscope_api_key: string;
  siliconflow_api_key: string;
  use_realtime_asr: boolean;
  enable_llm_post_process: boolean;
  llm_config: LlmConfig;
}

interface TranscriptionResult {
  text: string;
  original_text: string | null;
  asr_time_ms: number;
  llm_time_ms: number | null;
  total_time_ms: number;
}

// --- 历史记录 ---
interface HistoryRecord {
  id: string;
  timestamp: number;
  originalText: string;
  polishedText: string | null;
  presetName: string | null;
  asrTimeMs: number;
  llmTimeMs: number | null;
  totalTimeMs: number;
  success: boolean;
  errorMessage: string | null;
}

const HISTORY_KEY = 'pushtotalk_history';
const MAX_HISTORY = 50;

const loadHistory = (): HistoryRecord[] => {
  try {
    const data = localStorage.getItem(HISTORY_KEY);
    return data ? JSON.parse(data) : [];
  } catch { return []; }
};

const saveHistory = (records: HistoryRecord[]) => {
  localStorage.setItem(HISTORY_KEY, JSON.stringify(records.slice(0, MAX_HISTORY)));
};

const formatTimestamp = (ts: number): string => {
  const d = new Date(ts);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`;
};

// 默认配置
const DEFAULT_PRESETS: LlmPreset[] = [
  {
    id: "polishing",
    name: "文本润色",
    system_prompt: "你是一个语音转写润色助手。请在不改变原意的前提下：1）删除重复或意义相近的句子；2）合并同一主题的内容；3）去除「嗯」「啊」等口头禅；4）保留数字与关键信息；5）相关数字和时间不要使用中文；6）整理成自然的段落。输出纯文本即可。"
  },
  {
    id: "email",
    name: "邮件整理",
    system_prompt: "你是一个专业的邮件助手。请将用户的语音转写内容整理成一封格式规范、语气得体的工作邮件。请提取核心意图，补充必要的开场白和结语。输出仅包含邮件正文。"
  },
  {
    id: "translation",
    name: "中译英",
    system_prompt: "你是一个专业的翻译助手。请将用户的中文语音转写内容翻译成地道、流畅的英文。不要输出任何解释性文字，只输出翻译结果。"
  }
];

const DEFAULT_LLM_CONFIG: LlmConfig = {
  endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
  model: "glm-4-flash-250414",
  api_key: "",
  presets: DEFAULT_PRESETS,
  active_preset_id: "polishing"
};

function App() {
  const [apiKey, setApiKey] = useState("");
  const [fallbackApiKey, setFallbackApiKey] = useState("");
  const [useRealtime, setUseRealtime] = useState(true);
  const [enablePostProcess, setEnablePostProcess] = useState(false);
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG);
  const [showLlmModal, setShowLlmModal] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  const [status, setStatus] = useState<"idle" | "running" | "recording" | "transcribing">("idle");
  const [transcript, setTranscript] = useState("");
  const [originalTranscript, setOriginalTranscript] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recordingTime, setRecordingTime] = useState(0);
  const [asrTime, setAsrTime] = useState<number | null>(null);
  const [llmTime, setLlmTime] = useState<number | null>(null);
  const [totalTime, setTotalTime] = useState<number | null>(null);
  const [showSuccessToast, setShowSuccessToast] = useState(false);
  const [history, setHistory] = useState<HistoryRecord[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [copyToast, setCopyToast] = useState<string | null>(null);

  const transcriptEndRef = useRef<HTMLDivElement>(null);

  // 获取当前选中的预设对象
  const activePreset = llmConfig.presets.find(p => p.id === llmConfig.active_preset_id) || llmConfig.presets[0];

  useEffect(() => {
    if (transcriptEndRef.current) {
      transcriptEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [transcript]);

  useEffect(() => {
    const init = async () => {
      try {
        setHistory(loadHistory());
        await new Promise(resolve => setTimeout(resolve, 100));
        await setupEventListeners();
        await loadConfig();
      } catch (err) {
        console.error("初始化失败:", err);
        setError("应用初始化失败: " + String(err));
      }
    };
    init();
  }, []);

  useEffect(() => {
    let interval: number;
    if (status === "recording") {
      setRecordingTime(0);
      interval = setInterval(() => {
        setRecordingTime(prev => prev + 1);
      }, 1000);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [status]);

  const loadConfig = async () => {
    try {
      const config = await invoke<AppConfig>("load_config");
      setApiKey(config.dashscope_api_key);
      setFallbackApiKey(config.siliconflow_api_key || "");
      setUseRealtime(config.use_realtime_asr ?? true);
      setEnablePostProcess(config.enable_llm_post_process ?? false);
      
      // 修改这里：
      // 只要 config.llm_config 存在，我们就直接用。
      // 不再判断 presets.length === 0 就强行填充 DEFAULT_PRESETS。
      // 这样如果你在 UI 上删光了预设，下次进来就是空的，你可以从头开始加。
      const loadedLlmConfig = config.llm_config || DEFAULT_LLM_CONFIG;
      
      // 只有当 active_preset_id 无效时（比如对应的预设被删了），才重置选中状态
      if (loadedLlmConfig.presets && loadedLlmConfig.presets.length > 0) {
          const activeExists = loadedLlmConfig.presets.find(p => p.id === loadedLlmConfig.active_preset_id);
          if (!activeExists) {
              loadedLlmConfig.active_preset_id = loadedLlmConfig.presets[0].id;
          }
      }

      setLlmConfig(loadedLlmConfig);

      if (config.dashscope_api_key && config.dashscope_api_key.trim() !== "") {
        autoStartApp(config.dashscope_api_key, config.siliconflow_api_key || "", config.use_realtime_asr ?? true, config.enable_llm_post_process ?? false, loadedLlmConfig);
      }
    } catch (err) {
      console.error("加载配置失败:", err);
    }
  };

  const autoStartApp = async (apiKey: string, fallbackApiKey: string, useRealtimeMode: boolean, enablePostProcessMode: boolean, llmCfg: LlmConfig) => {
    try {
      await new Promise(resolve => setTimeout(resolve, 100));
      await invoke<string>("start_app", { apiKey, fallbackApiKey, useRealtime: useRealtimeMode, enablePostProcess: enablePostProcessMode, llmConfig: llmCfg });
      setStatus("running");
      setError(null);
    } catch (err) {
      setStatus("idle");
    }
  };

  const setupEventListeners = async () => {
    try {
      await listen("recording_started", () => {
        setStatus("recording");
        setError(null);
      });
      await listen("recording_stopped", () => {
        setStatus("transcribing");
      });
      await listen("transcribing", () => {
        setStatus("transcribing");
      });
      await listen<TranscriptionResult>("transcription_complete", (event) => {
        const result = event.payload;
        setTranscript(result.text);
        setOriginalTranscript(result.original_text);
        setAsrTime(result.asr_time_ms);
        setLlmTime(result.llm_time_ms);
        setTotalTime(result.total_time_ms);
        setStatus("running");
        // 添加成功记录到历史
        const record: HistoryRecord = {
          id: nanoid(8),
          timestamp: Date.now(),
          originalText: result.original_text || result.text,
          polishedText: result.original_text ? result.text : null,
          presetName: result.original_text ? (llmConfig.presets.find(p => p.id === llmConfig.active_preset_id)?.name || null) : null,
          asrTimeMs: result.asr_time_ms,
          llmTimeMs: result.llm_time_ms,
          totalTimeMs: result.total_time_ms,
          success: true,
          errorMessage: null
        };
        setHistory(prev => {
          const updated = [record, ...prev].slice(0, MAX_HISTORY);
          saveHistory(updated);
          return updated;
        });
      });
      await listen<string>("error", (event) => {
        const errMsg = event.payload;
        setError(errMsg);
        setStatus("running");
        // 添加失败记录到历史
        const record: HistoryRecord = {
          id: nanoid(8),
          timestamp: Date.now(),
          originalText: '',
          polishedText: null,
          presetName: null,
          asrTimeMs: 0,
          llmTimeMs: null,
          totalTimeMs: 0,
          success: false,
          errorMessage: errMsg
        };
        setHistory(prev => {
          const updated = [record, ...prev].slice(0, MAX_HISTORY);
          saveHistory(updated);
          return updated;
        });
      });
      await listen("transcription_cancelled", () => {
        setStatus("running");
        setError(null);
      });
    } catch (err) {
      throw err;
    }
  };

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const handleSaveConfig = async () => {
    try {
      await invoke<string>("save_config", { apiKey, fallbackApiKey, useRealtime, enablePostProcess, llmConfig });
      setError(null);
      setShowSuccessToast(true);
      setTimeout(() => setShowSuccessToast(false), 3000);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleStartStop = async () => {
    try {
      if (status === "idle") {
        if (!apiKey) {
          setError("请先输入 DashScope API Key");
          return;
        }
        await invoke<string>("save_config", { apiKey, fallbackApiKey, useRealtime, enablePostProcess, llmConfig });
        await invoke<string>("start_app", { apiKey, fallbackApiKey, useRealtime, enablePostProcess, llmConfig });
        setStatus("running");
        setError(null);
      } else {
        await invoke<string>("stop_app");
        setStatus("idle");
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleCancelTranscription = async () => {
    try {
      await invoke<string>("cancel_transcription");
    } catch (err) {
      setError(String(err));
    }
  };

  // --- 预设管理函数 ---

  const handleAddPreset = () => {
    const newPreset: LlmPreset = {
      id: nanoid(8),
      name: "新预设",
      system_prompt: ""
    };
    setLlmConfig(prev => ({
      ...prev,
      presets: [...prev.presets, newPreset],
      active_preset_id: newPreset.id
    }));
  };

  const handleDeletePreset = (id: string) => {
    if (llmConfig.presets.length <= 1) return; // 至少保留一个
    
    setLlmConfig(prev => {
      const newPresets = prev.presets.filter(p => p.id !== id);
      // 如果删除了当前选中的，选中第一个
      const newActiveId = prev.active_preset_id === id ? newPresets[0].id : prev.active_preset_id;
      return {
        ...prev,
        presets: newPresets,
        active_preset_id: newActiveId
      };
    });
  };

  const handleUpdateActivePreset = (key: keyof LlmPreset, value: string) => {
    setLlmConfig(prev => ({
      ...prev,
      presets: prev.presets.map(p =>
        p.id === prev.active_preset_id ? { ...p, [key]: value } : p
      )
    }));
  };

  // --- 历史记录操作 ---
  const handleCopyRecord = (record: HistoryRecord) => {
    const text = record.polishedText || record.originalText;
    navigator.clipboard.writeText(text);
    setCopyToast('已复制到剪贴板');
    setTimeout(() => setCopyToast(null), 2000);
  };

  const handleClearHistory = () => {
    setHistory([]);
    saveHistory([]);
  };

  const isRecording = status === "recording";
  const isTranscribing = status === "transcribing";
  const isRunning = status !== "idle";

  return (
    <div className="min-h-screen w-full bg-[#f5f5f7] text-slate-800 font-sans selection:bg-blue-500/20 selection:text-blue-700 flex items-center justify-center p-6">
      
      <div className="w-full max-w-3xl bg-white/80 backdrop-blur-2xl border border-white/50 shadow-2xl rounded-3xl overflow-hidden transition-all duration-500">
        
        {/* Top Status Bar */}
        <div className="px-6 py-4 border-b border-slate-100/50 flex items-center justify-between bg-white/40">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-500/10 rounded-xl text-blue-600">
              <Sparkles size={20} strokeWidth={2.5} />
            </div>
            <div>
              <h1 className="text-xl font-bold tracking-tight text-slate-900">PushToTalk</h1>
              <p className="text-xs text-slate-500 font-medium">AI 语音转写助手</p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => setShowHistory(true)}
              className="p-2 rounded-lg bg-slate-100 hover:bg-blue-100 text-slate-500 hover:text-blue-600 transition-all"
              title="历史记录"
            >
              <History size={18} />
            </button>
            <div className={`flex items-center gap-2 px-4 py-1.5 rounded-full border text-sm font-medium transition-all duration-300 ${
              isRecording ? "bg-red-50 border-red-100 text-red-600" :
              isTranscribing ? "bg-amber-50 border-amber-100 text-amber-600" :
              status === "running" ? "bg-emerald-50 border-emerald-100 text-emerald-600" :
              "bg-slate-100 border-slate-200 text-slate-500"
            }`}>
              <span className="relative flex h-2.5 w-2.5">
                {(isRecording || isTranscribing || status === 'running') && (
                  <span className={`animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 ${
                    isRecording ? "bg-red-400" : isTranscribing ? "bg-amber-400" : "bg-emerald-400"
                  }`}></span>
                )}
                <span className={`relative inline-flex rounded-full h-2.5 w-2.5 ${
                  isRecording ? "bg-red-500" :
                  isTranscribing ? "bg-amber-500" :
                  status === "running" ? "bg-emerald-500" : "bg-slate-400"
                }`}></span>
              </span>
              <span>
                {isRecording ? `正在录音 ${formatTime(recordingTime)}` :
                 isTranscribing ? "AI 转写中..." :
                 status === "running" ? "运行中 (Ctrl+Win)" : "已停止"}
              </span>
            </div>
            {(isRecording || isTranscribing) && (
              <button
                onClick={handleCancelTranscription}
                className="p-1.5 rounded-full bg-slate-100 hover:bg-red-100 text-slate-500 hover:text-red-600 transition-all duration-200"
                title="取消转录"
              >
                <XCircle size={18} />
              </button>
            )}
          </div>
        </div>

        <div className={`absolute top-24 left-0 right-0 flex justify-center pointer-events-none transition-all duration-500 z-10 ${
            showSuccessToast ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-4'
          }`}>
          <div className="bg-white/90 backdrop-blur text-emerald-600 px-4 py-2 rounded-full shadow-xl shadow-emerald-500/10 border border-emerald-100 flex items-center gap-2 text-sm font-medium">
             <CheckCircle2 size={16} className="fill-emerald-100" />
             <span>配置已保存成功</span>
          </div>
        </div>

        <div className="p-6 space-y-5">
          {error && (
            <div className="flex items-center gap-3 p-4 bg-red-50/80 border border-red-100 rounded-2xl text-red-600 text-sm animate-in slide-in-from-top-2 fade-in duration-300">
              <AlertCircle size={18} />
              <span>{error}</span>
            </div>
          )}

          {/* Transcript Display Area */}
          <div className="relative group">
            <div className="absolute -inset-0.5 bg-gradient-to-r from-blue-300 to-indigo-300 rounded-2xl blur opacity-20 group-hover:opacity-40 transition duration-500"></div>
            <div className="relative flex flex-col h-64 bg-white/60 backdrop-blur-sm border border-white/60 rounded-2xl p-6 shadow-inner transition-all">
              <div className="flex items-center justify-between mb-4">
                <label className="text-xs font-bold text-slate-400 uppercase tracking-wider flex items-center gap-1">
                  <Activity size={14} /> {originalTranscript ? '转写结果' : '实时转写内容'}
                </label>
                {transcript && (
                    <div className="flex items-center gap-2 flex-wrap justify-end">
                      {asrTime !== null && (
                        <span className="text-xs text-blue-600 bg-blue-50 px-2 py-1 rounded-md" title="语音转录耗时">
                          ASR {(asrTime / 1000).toFixed(2)}s
                        </span>
                      )}
                      {llmTime !== null && (
                        <span className="text-xs text-violet-600 bg-violet-50 px-2 py-1 rounded-md" title="LLM 润色耗时">
                          LLM {(llmTime / 1000).toFixed(2)}s
                        </span>
                      )}
                      {totalTime !== null && (
                        <span className="text-xs text-slate-500 bg-slate-100 px-2 py-1 rounded-md" title="总耗时">
                          共 {(totalTime / 1000).toFixed(2)}s
                        </span>
                      )}
                      <span className="text-xs text-slate-400 bg-slate-100 px-2 py-1 rounded-md">
                        {transcript.length} 字
                      </span>
                    </div>
                )}
              </div>

              {originalTranscript ? (
                <div className="flex-1 grid grid-cols-2 gap-4 min-h-0">
                  <div className="flex flex-col min-h-0 border-r border-slate-200 pr-4">
                    <div className="text-xs text-slate-400 mb-2 flex items-center gap-1">
                      <Mic size={12} /> 原始转录
                    </div>
                    <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                      <p className="text-slate-500 text-sm leading-relaxed whitespace-pre-wrap">{originalTranscript}</p>
                    </div>
                  </div>
                  <div className="flex flex-col min-h-0">
                    <div className="text-xs text-violet-500 mb-2 flex items-center gap-1">
                      <Wand2 size={12} /> 
                      {/* 显示使用的预设名称 */}
                      {llmConfig.presets.find(p => p.id === llmConfig.active_preset_id)?.name || "智能"}润色
                    </div>
                    <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                      <p className="text-slate-700 text-base leading-relaxed whitespace-pre-wrap">{transcript}</p>
                      <div ref={transcriptEndRef} />
                    </div>
                  </div>
                </div>
              ) : (
                <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                  {transcript ? (
                    <p className="text-slate-700 text-lg leading-relaxed whitespace-pre-wrap">{transcript}</p>
                  ) : (
                    <div className="h-full flex flex-col items-center justify-center text-slate-300 space-y-3">
                      <Mic size={48} strokeWidth={1} />
                      <p className="text-sm font-medium">按下快捷键开始说话...</p>
                    </div>
                  )}
                  <div ref={transcriptEndRef} />
                </div>
              )}
            </div>
          </div>

          {/* Settings Area */}
          <div className="space-y-5">
            <div className="flex items-center gap-2 text-slate-900 font-semibold">
              <Settings size={18} />
              <h2>API 配置</h2>
            </div>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-600 ml-1">DashScope (千问)</label>
                <div className="relative group">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-slate-400 group-focus-within:text-blue-500 transition-colors">
                    <Key size={16} />
                  </div>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    disabled={isRunning}
                    className="w-full pl-10 pr-10 py-3 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all disabled:opacity-60 disabled:cursor-not-allowed hover:border-slate-300"
                    placeholder="sk-..."
                  />
                  <button 
                    onClick={() => setShowApiKey(!showApiKey)}
                    className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600 transition-colors"
                  >
                    {showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>

              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <label className="text-sm font-medium text-slate-600 ml-1">SiliconFlow (备用)</label>
                  <span className="text-[10px] bg-slate-100 text-slate-500 px-1.5 py-0.5 rounded">可选</span>
                </div>
                <div className="relative group">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-slate-400 group-focus-within:text-indigo-500 transition-colors">
                    <Cpu size={16} />
                  </div>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={fallbackApiKey}
                    onChange={(e) => setFallbackApiKey(e.target.value)}
                    disabled={isRunning}
                    className="w-full pl-10 pr-10 py-3 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 transition-all disabled:opacity-60 disabled:cursor-not-allowed hover:border-slate-300"
                    placeholder="sk-..."
                  />
                </div>
              </div>
            </div>

            {/* Mode Switches */}
            <div className="flex items-center justify-between p-4 bg-slate-50/80 rounded-xl border border-slate-100">
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-lg transition-colors ${useRealtime ? 'bg-amber-100 text-amber-600' : 'bg-blue-100 text-blue-600'}`}>
                  {useRealtime ? <Zap size={18} /> : <Globe size={18} />}
                </div>
                <div>
                  <div className="text-sm font-medium text-slate-700">
                    {useRealtime ? '实时流式模式' : 'HTTP 传统模式'}
                  </div>
                  <div className="text-xs text-slate-400">
                    {useRealtime ? '边录边传，延迟更低' : '录完再传，更稳定'}
                  </div>
                </div>
              </div>
              <button
                onClick={() => setUseRealtime(!useRealtime)}
                disabled={isRunning}
                className={`relative w-14 h-7 rounded-full transition-all duration-300 ${
                  useRealtime
                    ? 'bg-amber-500'
                    : 'bg-slate-300'
                } ${isRunning ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:opacity-90'}`}
              >
                <span className={`absolute top-0.5 w-6 h-6 bg-white rounded-full shadow-md transition-all duration-300 ${
                  useRealtime ? 'left-7' : 'left-0.5'
                }`} />
              </button>
            </div>

            <div className="flex items-center justify-between p-4 bg-slate-50/80 rounded-xl border border-slate-100">
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-lg transition-colors ${enablePostProcess ? 'bg-violet-100 text-violet-600' : 'bg-slate-100 text-slate-400'}`}>
                  <Wand2 size={18} />
                </div>
                <div className="flex-1">
                  <div className="text-sm font-medium text-slate-700 flex items-center gap-2">
                    LLM 智能润色
                    {enablePostProcess && (
                      <span className="text-[10px] bg-violet-100 text-violet-600 px-1.5 py-0.5 rounded border border-violet-200">
                        {activePreset?.name}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-slate-400">
                    {enablePostProcess ? '自动去重、润色转录文本' : '直接输出原始转录'}
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {enablePostProcess && (
                  <button
                    onClick={() => setShowLlmModal(true)}
                    disabled={isRunning}
                    className="p-2 rounded-lg bg-violet-50 text-violet-600 hover:bg-violet-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    title="配置预设"
                  >
                    <Settings size={16} />
                  </button>
                )}
                <button
                  onClick={() => setEnablePostProcess(!enablePostProcess)}
                  disabled={isRunning}
                  className={`relative w-14 h-7 rounded-full transition-all duration-300 ${
                    enablePostProcess
                      ? 'bg-violet-500'
                      : 'bg-slate-300'
                  } ${isRunning ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:opacity-90'}`}
                >
                  <span className={`absolute top-0.5 w-6 h-6 bg-white rounded-full shadow-md transition-all duration-300 ${
                    enablePostProcess ? 'left-7' : 'left-0.5'
                  }`} />
                </button>
              </div>
            </div>

            {enablePostProcess && !llmConfig.api_key && (
              <div className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-100 rounded-xl text-amber-600 text-xs animate-in slide-in-from-top-2 fade-in duration-300">
                <AlertCircle size={14} />
                <span>请点击设置按钮配置 LLM API Key</span>
              </div>
            )}

            <div className="flex justify-end gap-4 text-xs text-slate-400">
               <a href="https://help.aliyun.com/zh/dashscope/developer-reference/quick-start" target="_blank" className="hover:text-blue-600 transition-colors flex items-center gap-1">
                 DashScope 文档 ↗
               </a>
               <a href="https://cloud.siliconflow.cn/" target="_blank" className="hover:text-indigo-600 transition-colors flex items-center gap-1">
                 硅基流动 ↗
               </a>
            </div>
          </div>
        </div>

        {/* Bottom Actions */}
        <div className="px-6 py-4 bg-slate-50/80 backdrop-blur border-t border-slate-100 flex items-center gap-4">
          <button
            onClick={handleSaveConfig}
            disabled={isRunning}
            className="flex-1 px-6 py-3.5 bg-white border border-slate-200 text-slate-700 font-medium rounded-xl shadow-sm hover:bg-slate-50 hover:border-slate-300 focus:ring-2 focus:ring-slate-200 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 group"
          >
            <CheckCircle2 size={18} className="group-hover:text-green-600 transition-colors"/>
            保存配置
          </button>

          <button
            onClick={handleStartStop}
            disabled={isRecording || isTranscribing}
            className={`flex-[2] px-6 py-3.5 font-medium rounded-xl shadow-lg shadow-blue-500/20 text-white transition-all transform active:scale-[0.98] flex items-center justify-center gap-2 ${
              status === "idle"
                ? "bg-slate-900 hover:bg-slate-800"
                : "bg-red-500 hover:bg-red-600 shadow-red-500/30"
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            {status === "idle" ? (
              <>
                <Sparkles size={18} /> 启动助手
              </>
            ) : (
              <>
                <StopCircle size={18} /> 停止服务
              </>
            )}
          </button>
        </div>
      </div>

      {/* Enhanced LLM Configuration Modal */}
      {showLlmModal && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-4xl mx-4 h-[80vh] flex flex-col overflow-hidden animate-in zoom-in-95 duration-200">
            
            {/* Modal Header */}
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between bg-gradient-to-r from-violet-50 to-purple-50">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-violet-100 rounded-xl text-violet-600">
                  <Wand2 size={20} />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-slate-900">LLM 润色配置</h3>
                  <p className="text-xs text-slate-500">管理不同场景的提示词预设</p>
                </div>
              </div>
              <button onClick={() => setShowLlmModal(false)} className="p-2 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-600 transition-colors">
                <X size={20} />
              </button>
            </div>

            {/* Modal Body - 2 Columns */}
            <div className="flex-1 flex overflow-hidden">
              
              {/* Left Sidebar: Presets List */}
              <div className="w-1/3 bg-slate-50 border-r border-slate-200 flex flex-col">
                <div className="p-4 border-b border-slate-200 bg-slate-50/50">
                  <h4 className="text-xs font-bold text-slate-400 uppercase tracking-wider mb-3">场景预设</h4>
                  <button 
                    onClick={handleAddPreset}
                    className="w-full py-2 bg-white border border-slate-200 rounded-lg text-sm text-slate-600 font-medium hover:border-violet-300 hover:text-violet-600 transition-all flex items-center justify-center gap-2 shadow-sm"
                  >
                    <Plus size={14} /> 新增预设
                  </button>
                </div>
                
                <div className="flex-1 overflow-y-auto p-2 space-y-1">
                  {llmConfig.presets.map(preset => (
                    <div 
                      key={preset.id}
                      onClick={() => setLlmConfig(prev => ({ ...prev, active_preset_id: preset.id }))}
                      className={`group flex items-center justify-between p-3 rounded-xl cursor-pointer transition-all ${
                        llmConfig.active_preset_id === preset.id 
                          ? 'bg-white shadow-md border border-violet-100 ring-1 ring-violet-500/20' 
                          : 'hover:bg-slate-100 border border-transparent'
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <div className={`p-1.5 rounded-lg ${
                          llmConfig.active_preset_id === preset.id ? 'bg-violet-100 text-violet-600' : 'bg-slate-200 text-slate-500'
                        }`}>
                          <MessageSquareQuote size={14} />
                        </div>
                        <span className={`text-sm font-medium ${
                          llmConfig.active_preset_id === preset.id ? 'text-slate-900' : 'text-slate-600'
                        }`}>
                          {preset.name}
                        </span>
                      </div>
                      
                      {llmConfig.presets.length > 1 && (
                        <button
                          onClick={(e) => { e.stopPropagation(); handleDeletePreset(preset.id); }}
                          className={`p-1.5 rounded-md text-slate-400 hover:bg-red-50 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100 ${
                            llmConfig.active_preset_id === preset.id ? 'opacity-100' : ''
                          }`}
                          title="删除预设"
                        >
                          <Trash2 size={14} />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>

              {/* Right Content: Preset Details & Global Config */}
              <div className="flex-1 flex flex-col bg-white overflow-hidden">
                
                {/* Active Preset Editor */}
                <div className="flex-1 overflow-y-auto p-6 space-y-6">
                  
                  {/* Preset Name */}
                  <div className="space-y-2">
                    <label className="text-sm font-medium text-slate-700">预设名称</label>
                    <input
                      type="text"
                      value={activePreset?.name || ""}
                      onChange={(e) => handleUpdateActivePreset('name', e.target.value)}
                      className="w-full px-4 py-2.5 bg-white border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all font-medium text-slate-900"
                      placeholder="例如：邮件整理"
                    />
                  </div>

                  {/* System Prompt */}
                  <div className="space-y-2 flex-1 flex flex-col">
                    <div className="flex justify-between items-center">
                      <label className="text-sm font-medium text-slate-700">系统提示词 (System Prompt)</label>
                      <button
                        onClick={() => {
                          const original = DEFAULT_PRESETS.find(p => p.id === activePreset.id);
                          if(original) handleUpdateActivePreset('system_prompt', original.system_prompt);
                        }}
                        className="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 transition-colors"
                      >
                        <RotateCcw size={12} /> 恢复默认
                      </button>
                    </div>
                    <textarea
                      value={activePreset?.system_prompt || ""}
                      onChange={(e) => handleUpdateActivePreset('system_prompt', e.target.value)}
                      className="w-full flex-1 min-h-[200px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                      placeholder="在这里定义 AI 的行为，例如：你是一个翻译助手..."
                    />
                  </div>

                  <div className="h-px bg-slate-100 my-6"></div>

                  {/* Global Settings Section (Collapsed style) */}
                  <div className="space-y-4">
                    <h4 className="text-xs font-bold text-slate-400 uppercase tracking-wider">全局模型设置</h4>
                    <div className="grid grid-cols-2 gap-4">
                      {/* API Key */}
                      <div className="col-span-2 space-y-1.5">
                        <label className="text-xs font-medium text-slate-500">API Key</label>
                        <div className="relative">
                          <input
                            type={showApiKey ? "text" : "password"}
                            value={llmConfig.api_key}
                            onChange={(e) => setLlmConfig({ ...llmConfig, api_key: e.target.value })}
                            className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                            placeholder="sk-..."
                          />
                          <button
                            onClick={() => setShowApiKey(!showApiKey)}
                            className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600"
                          >
                            {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </div>
                      </div>
                      
                      {/* Model */}
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-500">模型名称</label>
                        <input
                          type="text"
                          value={llmConfig.model}
                          onChange={(e) => setLlmConfig({ ...llmConfig, model: e.target.value })}
                          className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                          placeholder="gpt-4o-mini"
                        />
                      </div>

                      {/* Endpoint */}
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-500">API 地址</label>
                        <input
                          type="text"
                          value={llmConfig.endpoint}
                          onChange={(e) => setLlmConfig({ ...llmConfig, endpoint: e.target.value })}
                          className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                          placeholder="https://api..."
                        />
                      </div>
                    </div>
                  </div>

                </div>
              </div>
            </div>

            {/* Modal Footer */}
            <div className="px-6 py-4 border-t border-slate-100 bg-slate-50/50 flex items-center justify-end gap-3">
              <button
                onClick={() => setShowLlmModal(false)}
                className="px-5 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 hover:bg-slate-100 rounded-xl transition-colors"
              >
                关闭
              </button>
              <button
                onClick={() => {
                  handleSaveConfig();
                  setShowLlmModal(false);
                }}
                className="px-5 py-2.5 text-sm font-medium text-white bg-violet-500 hover:bg-violet-600 rounded-xl shadow-lg shadow-violet-500/20 transition-all"
              >
                保存并应用
              </button>
            </div>
          </div>
        </div>
      )}

      {/* History Drawer */}
      {showHistory && (
        <div className="fixed inset-0 z-50 flex justify-end">
          <div className="absolute inset-0 bg-black/30 backdrop-blur-sm" onClick={() => setShowHistory(false)} />
          <div className="relative w-full max-w-md bg-white shadow-2xl flex flex-col animate-in slide-in-from-right duration-300">
            {/* Header */}
            <div className="px-5 py-4 border-b border-slate-100 flex items-center justify-between bg-gradient-to-r from-blue-50 to-indigo-50">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-blue-100 rounded-xl text-blue-600">
                  <History size={20} />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-slate-900">历史记录</h3>
                  <p className="text-xs text-slate-500">共 {history.length} 条</p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {history.length > 0 && (
                  <button
                    onClick={handleClearHistory}
                    className="px-3 py-1.5 text-xs font-medium text-red-600 bg-red-50 hover:bg-red-100 rounded-lg transition-colors"
                  >
                    清空全部
                  </button>
                )}
                <button onClick={() => setShowHistory(false)} className="p-2 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-600 transition-colors">
                  <X size={20} />
                </button>
              </div>
            </div>

            {/* List */}
            <div className="flex-1 overflow-y-auto p-3 space-y-2">
              {history.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-slate-300 space-y-3">
                  <Clock size={48} strokeWidth={1} />
                  <p className="text-sm font-medium">暂无历史记录</p>
                </div>
              ) : (
                history.map(record => (
                  <div
                    key={record.id}
                    onClick={() => record.success && handleCopyRecord(record)}
                    className={`p-4 rounded-xl border transition-all ${
                      record.success
                        ? 'bg-white border-slate-100 hover:border-blue-200 hover:shadow-md cursor-pointer'
                        : 'bg-red-50/50 border-red-100'
                    }`}
                  >
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-xs text-slate-400 flex items-center gap-1">
                        <Clock size={12} />
                        {formatTimestamp(record.timestamp)}
                      </span>
                      {record.success ? (
                        <div className="flex items-center gap-2">
                          {record.presetName && (
                            <span className="text-[10px] bg-violet-100 text-violet-600 px-1.5 py-0.5 rounded">
                              {record.presetName}
                            </span>
                          )}
                          <span className="text-[10px] bg-slate-100 text-slate-500 px-1.5 py-0.5 rounded">
                            {(record.totalTimeMs / 1000).toFixed(1)}s
                          </span>
                          <Copy size={14} className="text-slate-400" />
                        </div>
                      ) : (
                        <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">失败</span>
                      )}
                    </div>
                    {record.success ? (
                      <p className="text-sm text-slate-700 line-clamp-3">
                        {record.polishedText || record.originalText}
                      </p>
                    ) : (
                      <p className="text-sm text-red-600 line-clamp-2">{record.errorMessage}</p>
                    )}
                  </div>
                ))
              )}
            </div>

            {/* Copy Toast */}
            {copyToast && (
              <div className="absolute bottom-4 left-1/2 -translate-x-1/2 bg-slate-900 text-white px-4 py-2 rounded-full text-sm font-medium shadow-lg animate-in fade-in zoom-in duration-200">
                {copyToast}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;