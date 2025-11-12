import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface AppConfig {
  dashscope_api_key: string;
}

function App() {
  const [apiKey, setApiKey] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [status, setStatus] = useState<"idle" | "running" | "recording" | "transcribing">("idle");
  const [transcript, setTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [recordingTime, setRecordingTime] = useState(0);

  // 加载配置
  useEffect(() => {
    const init = async () => {
      try {
        // 等待 Tauri 完全初始化
        await new Promise(resolve => setTimeout(resolve, 100));

        // 先设置事件监听器
        await setupEventListeners();
        // 然后加载配置（可能会触发自动启动）
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
      // 重置计时器
      setRecordingTime(0);

      // 每秒更新
      interval = setInterval(() => {
        setRecordingTime(prev => prev + 1);
      }, 1000);
    }

    return () => {
      if (interval) {
        clearInterval(interval);
      }
    };
  }, [status]);

  const loadConfig = async () => {
    try {
      console.log("开始加载配置...");
      const config = await invoke<AppConfig>("load_config");
      console.log("配置加载成功:", { apiKeyLength: config.dashscope_api_key.length });
      setApiKey(config.dashscope_api_key);

      // 如果已经配置了 API Key，自动启动应用
      if (config.dashscope_api_key && config.dashscope_api_key.trim() !== "") {
        console.log("检测到已保存的 API Key，自动启动应用...");
        autoStartApp(config.dashscope_api_key);
      } else {
        console.log("未检测到已保存的 API Key");
      }
    } catch (err) {
      console.error("加载配置失败:", err);
    }
  };

  const autoStartApp = async (apiKey: string) => {
    try {
      // 确保事件监听器已完全设置
      await new Promise(resolve => setTimeout(resolve, 100));

      const result = await invoke<string>("start_app", { apiKey });
      console.log("自动启动成功:", result);
      setStatus("running");
      setError(null);
    } catch (err) {
      console.error("自动启动失败:", err);
      // 自动启动失败不显示错误，让用户手动启动
      setStatus("idle");
    }
  };

  const setupEventListeners = async () => {
    try {
      console.log("开始设置事件监听器...");

      // 监听录音开始
      await listen("recording_started", () => {
        console.log("录音开始");
        setStatus("recording");
        setError(null);
      });

      // 监听录音停止
      await listen("recording_stopped", () => {
        console.log("录音停止");
        setStatus("transcribing");
      });

      // 监听转录中
      await listen("transcribing", () => {
        console.log("正在转录...");
        setStatus("transcribing");
      });

      // 监听转录完成
      await listen<string>("transcription_complete", (event) => {
        console.log("转录完成:", event.payload);
        setTranscript(event.payload);
        setStatus("running");
      });

      // 监听错误
      await listen<string>("error", (event) => {
        console.error("错误:", event.payload);
        setError(event.payload);
        setStatus("running");
      });

      console.log("事件监听器设置完成");
    } catch (err) {
      console.error("设置事件监听器失败:", err);
      throw err;
    }
  };

  const getStatusColor = () => {
    switch (status) {
      case "idle":
        return "bg-gray-400";
      case "running":
        return "bg-green-500 animate-pulse";
      case "recording":
        return "bg-red-500 animate-pulse";
      case "transcribing":
        return "bg-yellow-500 animate-pulse";
      default:
        return "bg-gray-400";
    }
  };

  const getStatusText = () => {
    switch (status) {
      case "idle":
        return "准备就绪";
      case "running":
        return "运行中 - 按 Ctrl+Win 录音";
      case "recording":
        return "录音中...";
      case "transcribing":
        return "转录中...";
      default:
        return "准备就绪";
    }
  };

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const handleSaveConfig = async () => {
    try {
      const result = await invoke<string>("save_config", { apiKey });
      console.log(result);
      setError(null);
      alert("配置已保存");
    } catch (err) {
      const errorMsg = String(err);
      setError(errorMsg);
      console.error("保存配置失败:", err);
    }
  };

  const handleStartStop = async () => {
    try {
      if (status === "idle") {
        if (!apiKey) {
          alert("请先输入 DashScope API Key");
          return;
        }
        // 启动前先保存配置
        console.log("启动前保存配置...");
        await invoke<string>("save_config", { apiKey });
        console.log("配置保存成功，开始启动应用...");
        const result = await invoke<string>("start_app", { apiKey });
        console.log(result);
        setStatus("running");
        setError(null);
      } else {
        const result = await invoke<string>("stop_app");
        console.log(result);
        setStatus("idle");
      }
    } catch (err) {
      const errorMsg = String(err);
      setError(errorMsg);
      console.error("启动/停止失败:", err);
    }
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 p-6">
      <div className="max-w-2xl mx-auto bg-white rounded-xl shadow-xl p-8">
        <h1 className="text-3xl font-bold text-gray-800 mb-6 text-center">
          PushToTalk
        </h1>

        {/* 状态指示器 */}
        <div className="mb-8 p-4 bg-gray-50 rounded-lg">
          <div className="flex items-center gap-3 mb-2">
            <div className={`w-4 h-4 rounded-full ${getStatusColor()}`}></div>
            <span className="text-lg font-medium text-gray-700">
              {status === "recording" ? (
                <>录音中 {formatTime(recordingTime)}</>
              ) : (
                getStatusText()
              )}
            </span>
          </div>
          {status === "running" && (
            <p className="text-sm text-gray-500 ml-7">
              💡 按住 <kbd className="px-2 py-1 bg-gray-200 rounded">Ctrl+Win</kbd> 开始录音
            </p>
          )}
        </div>

        {/* 错误提示 */}
        {error && (
          <div className="mb-6 p-4 bg-red-50 border border-red-200 rounded-lg">
            <p className="text-red-700 text-sm">
              ❌ {error}
            </p>
          </div>
        )}

        {/* 转录结果显示 */}
        <div className="mb-8">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            最新转录结果:
          </label>
          <div className="min-h-[120px] p-4 border-2 border-gray-200 rounded-lg bg-gray-50">
            {transcript ? (
              <p className="text-gray-800">{transcript}</p>
            ) : (
              <span className="text-gray-400 italic">转录内容将显示在这里...</span>
            )}
          </div>
        </div>

        {/* API 配置 */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            DashScope API Key:
          </label>
          <div className="flex gap-2">
            <input
              type={showApiKey ? "text" : "password"}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              className="flex-1 px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              disabled={status !== "idle"}
            />
            <button
              className="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300 transition"
              onClick={() => setShowApiKey(!showApiKey)}
            >
              {showApiKey ? "隐藏" : "显示"}
            </button>
          </div>
          <p className="mt-2 text-xs text-gray-500">
            获取 API Key: <a href="https://help.aliyun.com/zh/dashscope/developer-reference/quick-start" target="_blank" rel="noopener noreferrer" className="text-blue-600 hover:underline">DashScope 文档</a>
          </p>
        </div>

        {/* 保存配置按钮 */}
        <button
          onClick={handleSaveConfig}
          disabled={status !== "idle"}
          className="w-full mb-4 px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        >
          保存配置
        </button>

        {/* 启动/停止按钮 */}
        <button
          onClick={handleStartStop}
          disabled={status === "recording" || status === "transcribing"}
          className={`w-full px-6 py-3 rounded-lg transition font-medium text-white disabled:opacity-50 disabled:cursor-not-allowed ${
            status === "idle"
              ? "bg-green-600 hover:bg-green-700"
              : "bg-red-600 hover:bg-red-700"
          }`}
        >
          {status === "idle" ? "🚀 启动应用" : "⏹️ 停止应用"}
        </button>

        {/* 底部提示 */}
        <div className="mt-6 text-center text-sm text-gray-500">
          <p>快捷键: <kbd className="px-2 py-1 bg-gray-100 rounded">Ctrl+Win</kbd></p>
        </div>
      </div>
    </div>
  );
}

export default App;
