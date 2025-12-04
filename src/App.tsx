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
  RotateCcw
} from "lucide-react";

interface LlmConfig {
  endpoint: string;
  model: string;
  api_key: string;
  system_prompt: string;
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
  asr_time_ms: number;
  llm_time_ms: number | null;
  total_time_ms: number;
}

const DEFAULT_LLM_CONFIG: LlmConfig = {
  endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
  model: "glm-4-flash-250414",
  api_key: "",
  system_prompt: "你是一个语音转写润色助手。请在不改变原意的前提下：1）删除重复或意义相近的句子；2）合并同一主题的内容；3）去除「嗯」「啊」等口头禅；4）保留数字与关键信息；5）相关数字和时间不要使用中文；6）整理成自然的段落。输出纯文本即可。"
};

function App() {
  const [apiKey, setApiKey] = useState("");
  const [fallbackApiKey, setFallbackApiKey] = useState("");
  const [useRealtime, setUseRealtime] = useState(true); // 默认启用实时模式
  const [enablePostProcess, setEnablePostProcess] = useState(false); // LLM 后处理开关
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG); // LLM 配置
  const [showLlmModal, setShowLlmModal] = useState(false); // LLM 配置弹框
  const [showApiKey, setShowApiKey] = useState(false);
  const [status, setStatus] = useState<"idle" | "running" | "recording" | "transcribing">("idle");
  const [transcript, setTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [recordingTime, setRecordingTime] = useState(0);
  const [asrTime, setAsrTime] = useState<number | null>(null); // ASR 转录耗时
  const [llmTime, setLlmTime] = useState<number | null>(null); // LLM 润色耗时
  const [totalTime, setTotalTime] = useState<number | null>(null); // 总耗时
  const [showSuccessToast, setShowSuccessToast] = useState(false);
  // 用于转录框自动滚动到底部
  const transcriptEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (transcriptEndRef.current) {
      transcriptEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [transcript]);

  // 加载配置
  useEffect(() => {
    const init = async () => {
      try {
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

  // 计时器逻辑
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
      setLlmConfig(config.llm_config || DEFAULT_LLM_CONFIG);
      if (config.dashscope_api_key && config.dashscope_api_key.trim() !== "") {
        autoStartApp(config.dashscope_api_key, config.siliconflow_api_key || "", config.use_realtime_asr ?? true, config.enable_llm_post_process ?? false, config.llm_config || DEFAULT_LLM_CONFIG);
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
        setAsrTime(result.asr_time_ms);
        setLlmTime(result.llm_time_ms);
        setTotalTime(result.total_time_ms);
        setStatus("running");
      });
      await listen<string>("error", (event) => {
        setError(event.payload);
        setStatus("running");
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
      // 3秒后自动消失
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

  // UI 辅助函数
  const isRecording = status === "recording";
  const isTranscribing = status === "transcribing";
  const isRunning = status !== "idle";

  return (
    // 背景：使用细腻的网格渐变，模仿 macOS 壁纸质感
    <div className="min-h-screen w-full bg-[#f5f5f7] text-slate-800 font-sans selection:bg-blue-500/20 selection:text-blue-700 flex items-center justify-center p-6">
      
      {/* 主容器：Glassmorphism 风格 */}
      <div className="w-full max-w-3xl bg-white/80 backdrop-blur-2xl border border-white/50 shadow-2xl rounded-3xl overflow-hidden transition-all duration-500">
        
        {/* 顶部状态栏 */}
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

          {/* 状态胶囊 */}
          <div className="flex items-center gap-2">
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
            {/* 取消按钮 - 仅在录音或转录中显示 */}
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

        {/* 自定义 Toast 提示气泡  */}
        <div className={`absolute top-24 left-0 right-0 flex justify-center pointer-events-none transition-all duration-500 z-10 ${
            showSuccessToast ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-4'
          }`}>
          <div className="bg-white/90 backdrop-blur text-emerald-600 px-4 py-2 rounded-full shadow-xl shadow-emerald-500/10 border border-emerald-100 flex items-center gap-2 text-sm font-medium">
             <CheckCircle2 size={16} className="fill-emerald-100" />
             <span>配置已保存成功</span>
          </div>
        </div>

        <div className="p-6 space-y-5">
          
          {/* 错误提示条 */}
          {error && (
            <div className="flex items-center gap-3 p-4 bg-red-50/80 border border-red-100 rounded-2xl text-red-600 text-sm animate-in slide-in-from-top-2 fade-in duration-300">
              <AlertCircle size={18} />
              <span>{error}</span>
            </div>
          )}

          {/* 转录显示区域 - 模仿 iOS 备忘录 */}
          <div className="relative group">
            <div className="absolute -inset-0.5 bg-gradient-to-r from-blue-300 to-indigo-300 rounded-2xl blur opacity-20 group-hover:opacity-40 transition duration-500"></div>
            <div className="relative flex flex-col h-64 bg-white/60 backdrop-blur-sm border border-white/60 rounded-2xl p-6 shadow-inner transition-all">
              <div className="flex items-center justify-between mb-4">
                <label className="text-xs font-bold text-slate-400 uppercase tracking-wider flex items-center gap-1">
                  <Activity size={14} /> 实时转写内容
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
            </div>
          </div>

          {/* 设置区域 */}
          <div className="space-y-5">
            <div className="flex items-center gap-2 text-slate-900 font-semibold">
              <Settings size={18} />
              <h2>API 配置</h2>
            </div>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
              {/* 主 API Input */}
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

              {/* 备用 API Input */}
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

            {/* 传输模式切换 */}
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

            {/* LLM 后处理开关 */}
            <div className="flex items-center justify-between p-4 bg-slate-50/80 rounded-xl border border-slate-100">
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-lg transition-colors ${enablePostProcess ? 'bg-violet-100 text-violet-600' : 'bg-slate-100 text-slate-400'}`}>
                  <Wand2 size={18} />
                </div>
                <div>
                  <div className="text-sm font-medium text-slate-700">
                    LLM 智能润色
                  </div>
                  <div className="text-xs text-slate-400">
                    {enablePostProcess ? '自动去重、润色转录文本' : '直接输出原始转录'}
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {/* 配置按钮 */}
                {enablePostProcess && (
                  <button
                    onClick={() => setShowLlmModal(true)}
                    disabled={isRunning}
                    className="p-2 rounded-lg bg-violet-50 text-violet-600 hover:bg-violet-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    title="配置 LLM"
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

            {/* LLM 未配置提示 */}
            {enablePostProcess && !llmConfig.api_key && (
              <div className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-100 rounded-xl text-amber-600 text-xs animate-in slide-in-from-top-2 fade-in duration-300">
                <AlertCircle size={14} />
                <span>请点击设置按钮配置 LLM API Key</span>
              </div>
            )}

            {/* 文档链接 */}
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

        {/* 底部操作栏 */}
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

      {/* LLM 配置弹框 */}
      {showLlmModal && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-lg mx-4 max-h-[90vh] overflow-hidden animate-in zoom-in-95 duration-200">
            {/* 弹框头部 */}
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between bg-gradient-to-r from-violet-50 to-purple-50">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-violet-100 rounded-xl text-violet-600">
                  <Wand2 size={20} />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-slate-900">LLM 润色配置</h3>
                  <p className="text-xs text-slate-500">自定义模型、API 地址和提示词</p>
                </div>
              </div>
              <button
                onClick={() => setShowLlmModal(false)}
                className="p-2 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-600 transition-colors"
              >
                <X size={20} />
              </button>
            </div>

            {/* 弹框内容 */}
            <div className="p-6 space-y-5 overflow-y-auto max-h-[60vh]">
              {/* API 地址 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-700">API 地址</label>
                <input
                  type="text"
                  value={llmConfig.endpoint}
                  onChange={(e) => setLlmConfig({ ...llmConfig, endpoint: e.target.value })}
                  className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all"
                  placeholder="https://api.openai.com/v1/chat/completions"
                />
                <p className="text-xs text-slate-400">支持 OpenAI 兼容格式的 API</p>
              </div>

              {/* 模型名称 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-700">模型名称</label>
                <input
                  type="text"
                  value={llmConfig.model}
                  onChange={(e) => setLlmConfig({ ...llmConfig, model: e.target.value })}
                  className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all"
                  placeholder="gpt-4o-mini"
                />
              </div>

              {/* API Key */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-700">API Key</label>
                <div className="relative">
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={llmConfig.api_key}
                    onChange={(e) => setLlmConfig({ ...llmConfig, api_key: e.target.value })}
                    className="w-full px-4 py-3 pr-10 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all"
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

              {/* System Prompt */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-sm font-medium text-slate-700">System Prompt</label>
                  <button
                    onClick={() => setLlmConfig({ ...llmConfig, system_prompt: DEFAULT_LLM_CONFIG.system_prompt })}
                    className="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 transition-colors"
                  >
                    <RotateCcw size={12} /> 重置默认
                  </button>
                </div>
                <textarea
                  value={llmConfig.system_prompt}
                  onChange={(e) => setLlmConfig({ ...llmConfig, system_prompt: e.target.value })}
                  rows={4}
                  className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all resize-none"
                  placeholder="输入系统提示词..."
                />
              </div>
            </div>

            {/* 弹框底部 */}
            <div className="px-6 py-4 border-t border-slate-100 bg-slate-50/50 flex items-center justify-end gap-3">
              <button
                onClick={() => setShowLlmModal(false)}
                className="px-5 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 hover:bg-slate-100 rounded-xl transition-colors"
              >
                取消
              </button>
              <button
                onClick={() => {
                  setShowLlmModal(false);
                  handleSaveConfig();
                }}
                className="px-5 py-2.5 text-sm font-medium text-white bg-violet-500 hover:bg-violet-600 rounded-xl shadow-lg shadow-violet-500/20 transition-all"
              >
                保存配置
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;