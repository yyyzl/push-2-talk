// src/App.tsx

import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Mic,
  StopCircle,
  Settings,
  Activity,
  CheckCircle2,
  AlertCircle,
  Eye,
  EyeOff,
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
  Clock,
  Minus,
  Power,
  Download,
  RefreshCw,
  Keyboard,
  BookOpen,
  KeyRound,
  ScrollText,
  Github,
  VolumeX,
  BookText
} from "lucide-react";
import { nanoid } from 'nanoid';

// --- 新的接口定义 ---

// 热键类型定义
type HotkeyKey =
  | 'control_left' | 'control_right'
  | 'shift_left' | 'shift_right'
  | 'alt_left' | 'alt_right'
  | 'meta_left' | 'meta_right'
  | 'space' | 'tab' | 'caps_lock' | 'escape'
  | 'f1' | 'f2' | 'f3' | 'f4' | 'f5' | 'f6' | 'f7' | 'f8' | 'f9' | 'f10' | 'f11' | 'f12'
  | 'key_a' | 'key_b' | 'key_c' | 'key_d' | 'key_e' | 'key_f' | 'key_g' | 'key_h' | 'key_i' | 'key_j'
  | 'key_k' | 'key_l' | 'key_m' | 'key_n' | 'key_o' | 'key_p' | 'key_q' | 'key_r' | 'key_s' | 'key_t'
  | 'key_u' | 'key_v' | 'key_w' | 'key_x' | 'key_y' | 'key_z'
  | 'num_0' | 'num_1' | 'num_2' | 'num_3' | 'num_4' | 'num_5' | 'num_6' | 'num_7' | 'num_8' | 'num_9'
  | 'up' | 'down' | 'left' | 'right'
  | 'return' | 'backspace' | 'delete' | 'insert' | 'home' | 'end' | 'page_up' | 'page_down';

interface HotkeyConfig {
  keys: HotkeyKey[];
  enable_release_lock?: boolean;  // 已弃用，保留用于向后兼容
  release_mode_keys?: HotkeyKey[];  // 松手模式独立快捷键
}

// 双热键配置（听写模式 + AI助手模式）
interface DualHotkeyConfig {
  dictation: HotkeyConfig;  // 听写模式（默认 Ctrl+Win）
  assistant: HotkeyConfig;  // AI助手模式（默认 Alt+Space）
}

// 按键显示名称映射
const KEY_DISPLAY_NAMES: Record<HotkeyKey, string> = {
  control_left: 'Ctrl(左)', control_right: 'Ctrl(右)',
  shift_left: 'Shift(左)', shift_right: 'Shift(右)',
  alt_left: 'Alt(左)', alt_right: 'Alt(右)',
  meta_left: 'Win(左)', meta_right: 'Win(右)',
  space: 'Space', tab: 'Tab', caps_lock: 'CapsLock', escape: 'Esc',
  f1: 'F1', f2: 'F2', f3: 'F3', f4: 'F4', f5: 'F5', f6: 'F6',
  f7: 'F7', f8: 'F8', f9: 'F9', f10: 'F10', f11: 'F11', f12: 'F12',
  key_a: 'A', key_b: 'B', key_c: 'C', key_d: 'D', key_e: 'E', key_f: 'F',
  key_g: 'G', key_h: 'H', key_i: 'I', key_j: 'J', key_k: 'K', key_l: 'L',
  key_m: 'M', key_n: 'N', key_o: 'O', key_p: 'P', key_q: 'Q', key_r: 'R',
  key_s: 'S', key_t: 'T', key_u: 'U', key_v: 'V', key_w: 'W', key_x: 'X',
  key_y: 'Y', key_z: 'Z',
  num_0: '0', num_1: '1', num_2: '2', num_3: '3', num_4: '4',
  num_5: '5', num_6: '6', num_7: '7', num_8: '8', num_9: '9',
  up: '↑', down: '↓', left: '←', right: '→',
  return: 'Enter', backspace: 'Backspace', delete: 'Delete', insert: 'Insert',
  home: 'Home', end: 'End', page_up: 'PageUp', page_down: 'PageDown',
};

type AsrProvider = 'qwen' | 'doubao' | 'siliconflow';

interface AsrProviderConfig {
  provider: AsrProvider;
  api_key: string;
  app_id?: string;
  access_token?: string;
}

interface AsrConfig {
  primary: AsrProviderConfig;
  fallback: AsrProviderConfig | null;
  enable_fallback: boolean;
}

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

// Smart Command 配置（保留用于向后兼容，但不再使用）
interface SmartCommandConfig {
  enabled: boolean;
  endpoint: string;
  model: string;
  api_key: string;
  system_prompt: string;
}

// AI 助手配置（双系统提示词）
interface AssistantConfig {
  enabled: boolean;
  endpoint: string;
  model: string;
  api_key: string;
  qa_system_prompt: string;               // 问答模式提示词（无选中文本时）
  text_processing_system_prompt: string;  // 文本处理提示词（有选中文本时）
}

interface AppConfig {
  dashscope_api_key: string;
  siliconflow_api_key: string;
  asr_config: AsrConfig;
  use_realtime_asr: boolean;
  enable_llm_post_process: boolean;
  llm_config: LlmConfig;
  smart_command_config: SmartCommandConfig;
  assistant_config: AssistantConfig;      // 新增：AI 助手配置
  close_action: "close" | "minimize" | null;
  show_tray_icon: boolean;               // 是否显示系统托盘图标
  hotkey_config: HotkeyConfig;            // 保留用于迁移
  dual_hotkey_config: DualHotkeyConfig;   // 新增：双热键配置
  enable_mute_other_apps: boolean;        // 录音时静音其他应用
  dictionary: string[];                   // 个人词典（热词列表）
}

interface TranscriptionResult {
  text: string;
  original_text: string | null;
  asr_time_ms: number;
  llm_time_ms: number | null;
  total_time_ms: number;
  mode?: string; // "normal" | "smartcommand"
  inserted?: boolean;
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

// Smart Command 默认配置
const DEFAULT_SMART_COMMAND_CONFIG: SmartCommandConfig = {
  enabled: false,
  endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
  model: "glm-4-flash-250414",
  api_key: "",
  system_prompt: `你是一个智能语音助手。用户会通过语音向你提问，你需要：
1. 理解用户的问题
2. 给出简洁、准确、有用的回答
3. 如果问题不够明确，给出最可能的解答

注意：
- 回答要简洁明了，适合直接粘贴使用
- 避免过多的解释和废话
- 如果是代码相关问题，直接给出代码`
};

// AI 助手默认配置
const DEFAULT_ASSISTANT_CONFIG: AssistantConfig = {
  enabled: false,
  endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
  model: "glm-4-flash-250414",
  api_key: "",
  qa_system_prompt: `你是一个智能语音助手。用户会通过语音向你提问，你需要：
1. 理解用户的问题
2. 给出简洁、准确、有用的回答
3. 如果问题不够明确，给出最可能的解答

注意：
- 回答要简洁明了，适合直接粘贴使用
- 避免过多的解释和废话
- 如果是代码相关问题，直接给出代码`,
  text_processing_system_prompt: `你是一个文本处理助手。用户会选中一段文本，然后通过语音告诉你要如何处理这段文本。

你的任务：
1. 理解用户的语音指令
2. 对选中的文本执行相应操作（润色、翻译、总结、改写等）
3. 直接输出处理后的文本

注意：
- 只输出处理后的结果，不要输出任何解释
- 保持原文的格式和结构（除非用户要求改变）
- 如果指令不明确，按最合理的方式处理`
};

// ASR 服务商元数据
const ASR_PROVIDERS: Record<AsrProvider, { name: string; model: string; docsUrl: string }> = {
  qwen: {
    name: '阿里千问',
    model: 'qwen3-asr-flash',
    docsUrl: 'https://help.aliyun.com/zh/dashscope/developer-reference/quick-start',
  },
  doubao: {
    name: '豆包',
    model: 'Doubao-Seed-ASR-2.0',
    docsUrl: 'https://www.volcengine.com/docs/6561',
  },
  siliconflow: {
    name: '硅基移动',
    model: 'SenseVoiceSmall',
    docsUrl: 'https://cloud.siliconflow.cn/',
  },
};

function App() {
  const [currentVersion, setCurrentVersion] = useState(() =>
    localStorage.getItem('app_version') || ''
  );
  const [apiKey, setApiKey] = useState("");
  const [fallbackApiKey, setFallbackApiKey] = useState("");

  // Cache for different ASR providers to prevent data loss when switching
  const CACHE_STORAGE_KEY = 'pushtotalk_asr_cache';
  const [asrCache, setAsrCache] = useState<{
    active_provider: AsrProvider;
    qwen: { api_key: string };
    doubao: { app_id: string; access_token: string };
    siliconflow: { api_key: string };
  }>(() => {
    // Initialize from localStorage to persist across sessions
    try {
      const saved = localStorage.getItem(CACHE_STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        return {
          active_provider: parsed.active_provider || 'qwen',
          ...parsed
        };
      }
    } catch (e) {
      console.error("Failed to load ASR cache:", e);
    }
    return {
      active_provider: 'qwen',
      qwen: { api_key: '' },
      doubao: { app_id: '', access_token: '' },
      siliconflow: { api_key: '' }
    };
  });

  const [asrConfig, setAsrConfig] = useState<AsrConfig>(() => {
    const provider = asrCache.active_provider;
    return {
      primary: provider === 'qwen'
        ? { provider: 'qwen', api_key: asrCache.qwen.api_key }
        : { provider: 'doubao', api_key: '', app_id: asrCache.doubao.app_id, access_token: asrCache.doubao.access_token },
      fallback: null,
      enable_fallback: false,
    };
  });
  const [useRealtime, setUseRealtime] = useState(true);
  const [enablePostProcess, setEnablePostProcess] = useState(false);
  const [llmConfig, setLlmConfig] = useState<LlmConfig>(DEFAULT_LLM_CONFIG);
  // 统一服务配置弹窗
  const [showServiceModal, setShowServiceModal] = useState(false);
  const [serviceModalTab, setServiceModalTab] = useState<'asr' | 'llm' | 'assistant' | 'dictionary'>('asr');
  // smartCommandConfig 保留用于向后兼容（加载旧配置时不会报错），使用常量替代 useState
  const smartCommandConfig = DEFAULT_SMART_COMMAND_CONFIG;
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
  // 个人词典状态
  const [dictionary, setDictionary] = useState<string[]>([]);
  const [newWord, setNewWord] = useState('');
  const [duplicateHint, setDuplicateHint] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState('');
  const [copyToast, setCopyToast] = useState<string | null>(null);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [rememberChoice, setRememberChoice] = useState(false);
  const [enableAutostart, setEnableAutostart] = useState(false);
  const [enableMuteOtherApps, setEnableMuteOtherApps] = useState(false);
  const [closeAction, setCloseAction] = useState<"close" | "minimize" | null>(null);
  const [showTrayIcon, setShowTrayIcon] = useState(true);
  const [updateStatus, setUpdateStatus] = useState<"idle" | "checking" | "available" | "downloading" | "ready">("idle");
  const [updateInfo, setUpdateInfo] = useState<{ version: string; notes?: string } | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  // hotkeyConfig 已迁移到 dualHotkeyConfig，不再单独使用
  const [dualHotkeyConfig, setDualHotkeyConfig] = useState<DualHotkeyConfig>({
    dictation: { keys: ['control_left', 'meta_left'] },
    assistant: { keys: ['alt_left', 'space'] }
  });
  const [assistantConfig, setAssistantConfig] = useState<AssistantConfig>(DEFAULT_ASSISTANT_CONFIG);
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [recordingMode, setRecordingMode] = useState<'dictation' | 'assistant' | 'release'>('dictation'); // 当前录制模式
  const [recordingKeys, setRecordingKeys] = useState<HotkeyKey[]>([]); // 录制时实时显示的按键
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [currentMode, setCurrentMode] = useState<string | null>(null); // 当前转录模式: "normal" | "smartcommand"

  const transcriptEndRef = useRef<HTMLDivElement>(null);
  const hasCheckedUpdateOnStartup = useRef(false);

  // 获取当前选中的预设对象
  const activePreset = llmConfig.presets.find(p => p.id === llmConfig.active_preset_id) || llmConfig.presets[0];

  // Auto-save asrCache to localStorage whenever it changes
  useEffect(() => {
    localStorage.setItem(CACHE_STORAGE_KEY, JSON.stringify(asrCache));
  }, [asrCache]);

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

        // 启动时自动检查更新（只执行一次）
        if (!hasCheckedUpdateOnStartup.current) {
          hasCheckedUpdateOnStartup.current = true;
          try {
            const update = await check();
            if (update) {
              setUpdateInfo({
                version: update.version,
                notes: update.body || undefined
              });
              setUpdateStatus("available");
              setShowUpdateModal(true);
            }
          } catch (err) {
            console.error("启动时检查更新失败:", err);
            // 静默失败，不显示错误提示
          }
        }
      } catch (err) {
        console.error("初始化失败:", err);
        setError("应用初始化失败: " + String(err));
      }
    };
    init();
  }, []);

  useEffect(() => {
    getVersion().then(v => {
      setCurrentVersion(v);
      localStorage.setItem('app_version', v);
    }).catch(() => {});
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

  // 热键录制监听
  useEffect(() => {
    if (!isRecordingHotkey) {
      setRecordingKeys([]); // 清空录制状态
      return;
    }

    console.log("开始热键录制监听");
    const pressedKeysSet = new Set<HotkeyKey>();
    let hasRecordedKeys = false; // 标记是否已经录制到按键

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      console.log("KeyDown:", e.key, e.code, e.location);
      const key = mapDomKeyToHotkeyKey(e);
      console.log("Mapped key:", key);
      if (key && !pressedKeysSet.has(key)) {
        pressedKeysSet.add(key);
        hasRecordedKeys = true;
        // 立即更新 UI 显示
        setRecordingKeys(Array.from(pressedKeysSet));
        console.log("当前按下的键:", Array.from(pressedKeysSet));
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      console.log("KeyUp:", e.key, e.code, "hasRecordedKeys:", hasRecordedKeys, "size:", pressedKeysSet.size);

      // 只有录制到按键后，松开任意键才结束录制
      if (hasRecordedKeys && pressedKeysSet.size > 0) {
        const keysArray = Array.from(pressedKeysSet);
        console.log("准备保存的按键:", keysArray);

        // 验证：必须有修饰键或功能键
        const hasModifier = keysArray.some(k => isModifierKey(k));
        const isFunctionKey = keysArray.every(k => /^f([1-9]|1[0-2])$/.test(k));
        console.log("是否包含修饰键:", hasModifier, "是否为功能键:", isFunctionKey);

        if (hasModifier || isFunctionKey) {
          // 根据录制模式更新对应的热键配置
          const newDualHotkeyConfig = { ...dualHotkeyConfig };
          if (recordingMode === 'dictation') {
            newDualHotkeyConfig.dictation = {
              ...newDualHotkeyConfig.dictation,
              keys: keysArray
            };
          } else if (recordingMode === 'release') {
            newDualHotkeyConfig.dictation = {
              ...newDualHotkeyConfig.dictation,
              release_mode_keys: keysArray
            };
          } else {
            newDualHotkeyConfig.assistant = {
              ...newDualHotkeyConfig.assistant,
              keys: keysArray
            };
          }
          setDualHotkeyConfig(newDualHotkeyConfig);
          setHotkeyError(null);

          // 保存配置
          invoke<string>("save_config", {
            apiKey,
            fallbackApiKey,
            useRealtime,
            enablePostProcess,
            llmConfig,
            smartCommandConfig,
            assistantConfig,
            asrConfig,
            dualHotkeyConfig: newDualHotkeyConfig,
            showTrayIcon,
            enableMuteOtherApps
          }).then(() => {
            const modeName = recordingMode === 'dictation' ? '听写' : recordingMode === 'release' ? '松手' : 'AI助手';
            console.log(`${modeName}模式热键配置已保存:`, keysArray);
          }).catch(err => {
            console.error("保存热键配置失败:", err);
          });
        } else {
          setHotkeyError("必须包含修饰键(Ctrl/Alt/Shift/Win) 或 功能键(F1-F12)");
          setTimeout(() => setHotkeyError(null), 3000);
        }

        setIsRecordingHotkey(false);
        setRecordingKeys([]);
      }
    };

    // 使用 capture 阶段捕获事件
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);

    return () => {
      console.log("停止热键录制监听");
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [isRecordingHotkey, apiKey, fallbackApiKey, useRealtime, enablePostProcess, llmConfig, asrConfig]);

  const loadConfig = async () => {
    try {
      const config = await invoke<AppConfig>("load_config");
      setApiKey(config.dashscope_api_key);
      setFallbackApiKey(config.siliconflow_api_key || "");

      // 智能填充缓存：更新后端读到的 Key 到对应的缓存槽位
      if (config.asr_config) {
        const backendPrimary = config.asr_config.primary;

        setAsrCache(prev => {
          const newCache = { ...prev };
          // 更新千问缓存
          if (backendPrimary.provider === 'qwen') {
            newCache.qwen.api_key = backendPrimary.api_key;
          } else if (config.dashscope_api_key) {
            newCache.qwen.api_key = config.dashscope_api_key;
          }
          // 更新豆包缓存
          if (backendPrimary.provider === 'doubao') {
            newCache.doubao.app_id = backendPrimary.app_id || '';
            newCache.doubao.access_token = backendPrimary.access_token || '';
          }
          // 更新硅基缓存
          if (config.asr_config.fallback?.provider === 'siliconflow') {
            newCache.siliconflow.api_key = config.asr_config.fallback.api_key;
          } else if (config.siliconflow_api_key) {
            newCache.siliconflow.api_key = config.siliconflow_api_key;
          }
          return newCache;
        });

        // 使用缓存中的 active_provider，而不是后端返回的
        const preferredProvider = asrCache.active_provider;

        setAsrConfig({
          ...config.asr_config,
          primary: {
            provider: preferredProvider,
            api_key: preferredProvider === 'qwen' ? (config.dashscope_api_key || asrCache.qwen.api_key) : '',
            app_id: preferredProvider === 'doubao' ? asrCache.doubao.app_id : undefined,
            access_token: preferredProvider === 'doubao' ? asrCache.doubao.access_token : undefined,
          }
        });
      }

      setUseRealtime(config.use_realtime_asr ?? true);
      setEnablePostProcess(config.enable_llm_post_process ?? false);

      const loadedLlmConfig = config.llm_config || DEFAULT_LLM_CONFIG;

      if (loadedLlmConfig.presets && loadedLlmConfig.presets.length > 0) {
          const activeExists = loadedLlmConfig.presets.find(p => p.id === loadedLlmConfig.active_preset_id);
          if (!activeExists) {
              loadedLlmConfig.active_preset_id = loadedLlmConfig.presets[0].id;
          }
      }

      setLlmConfig(loadedLlmConfig);

      // Smart Command 配置已移除，不再使用

      // 加载 AI 助手配置
      if (config.assistant_config) {
        setAssistantConfig(config.assistant_config);
      } else {
        setAssistantConfig(DEFAULT_ASSISTANT_CONFIG);
      }

      // 加载双热键配置（优先使用 dual_hotkey_config，向后兼容 hotkey_config）
      if (config.dual_hotkey_config) {
        setDualHotkeyConfig(config.dual_hotkey_config);
      } else if (config.hotkey_config && config.hotkey_config.keys.length > 0) {
        // 如果只有旧的 hotkey_config，则迁移到 dual_hotkey_config
        setDualHotkeyConfig({
          dictation: config.hotkey_config,
          assistant: { keys: ['alt_left', 'space'] }
        });
      }

      // 加载关闭行为配置
      if (config.close_action) {
        setCloseAction(config.close_action);
      }

      // 加载托盘图标显示配置（默认显示）
      setShowTrayIcon(config.show_tray_icon ?? true);

      // 加载开机自启动状态
      try {
        const autostart = await invoke<boolean>("get_autostart");
        setEnableAutostart(autostart);
      } catch (err) {
        console.error("获取开机自启状态失败:", err);
      }

      // 加载录音时静音其他应用的配置
      setEnableMuteOtherApps(config.enable_mute_other_apps ?? false);

      // 加载个人词典
      const loadedDictionary = (config.dictionary && Array.isArray(config.dictionary)) ? config.dictionary : [];
      setDictionary(loadedDictionary);

      // 自动启动时也需要传递 asrConfig、dualHotkeyConfig 和 assistantConfig
      const loadedAsrConfig = config.asr_config || null;
      const loadedDualHotkeyConfig = config.dual_hotkey_config || {
        dictation: config.hotkey_config || { keys: ['control_left', 'meta_left'] as HotkeyKey[] },
        assistant: { keys: ['alt_left', 'space'] as HotkeyKey[] }
      };
      const loadedSmartCommandConfig = config.smart_command_config || DEFAULT_SMART_COMMAND_CONFIG;
      const loadedAssistantConfig = config.assistant_config || DEFAULT_ASSISTANT_CONFIG;

      if (loadedAsrConfig && isAsrConfigValid(loadedAsrConfig.primary)) {
        autoStartApp(
          config.dashscope_api_key,
          config.siliconflow_api_key || "",
          config.use_realtime_asr ?? true,
          config.enable_llm_post_process ?? false,
          loadedLlmConfig,
          loadedSmartCommandConfig,
          loadedAssistantConfig,
          loadedAsrConfig,
          loadedDualHotkeyConfig,
          config.enable_mute_other_apps ?? false,
          loadedDictionary
        );
      }
    } catch (err) {
      console.error("加载配置失败:", err);
    }
  };

  const autoStartApp = async (
    apiKey: string,
    fallbackApiKey: string,
    useRealtimeMode: boolean,
    enablePostProcessMode: boolean,
    llmCfg: LlmConfig,
    smartCmdCfg: SmartCommandConfig,
    assistantCfg: AssistantConfig,
    asrCfg: AsrConfig | null,
    dualHotkeyCfg: DualHotkeyConfig,
    enableMuteOtherAppsMode: boolean,
    dict: string[]
  ) => {
    try {
      await new Promise(resolve => setTimeout(resolve, 100));
      await invoke<string>("start_app", {
        apiKey,
        fallbackApiKey,
        useRealtime: useRealtimeMode,
        enablePostProcess: enablePostProcessMode,
        llmConfig: llmCfg,
        smartCommandConfig: smartCmdCfg,
        assistantConfig: assistantCfg,
        asrConfig: asrCfg,
        dualHotkeyConfig: dualHotkeyCfg,
        enableMuteOtherApps: enableMuteOtherAppsMode,
        dictionary: dict
      });
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
        console.log('[DEBUG] 收到转录结果:', {
          mode: result.mode,
          hasOriginalText: !!result.original_text,
          textLength: result.text.length,
          originalTextLength: result.original_text?.length || 0,
          llmTime: result.llm_time_ms
        });
        setTranscript(result.text);
        setOriginalTranscript(result.original_text);
        setCurrentMode(result.mode || null); // 保存当前模式
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
      // 监听窗口关闭请求
      await listen("close_requested", async () => {
        try {
          const config = await invoke<AppConfig>("load_config");
          if (config.close_action === "close") {
            await invoke("quit_app");
          } else if (config.close_action === "minimize") {
            await invoke("hide_to_tray");
          } else {
            setShowCloseDialog(true);
          }
        } catch {
          setShowCloseDialog(true);
        }
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

  // 词典操作函数
  const handleAddWord = () => {
    const word = newWord.trim();
    if (!word) return;
    if (dictionary.includes(word)) {
      setDuplicateHint(true);
      setTimeout(() => setDuplicateHint(false), 2000);
      return;
    }
    setDictionary(prev => [...prev, word]);
    setNewWord('');
  };

  const handleDeleteWord = (index: number) => {
    setDictionary(prev => prev.filter((_, i) => i !== index));
    if (editingIndex === index) {
      // 删除的是正在编辑的词条，取消编辑
      setEditingIndex(null);
      setEditingValue('');
    } else if (editingIndex !== null && index < editingIndex) {
      // 删除的词条在编辑词条之前，需要调整索引
      setEditingIndex(editingIndex - 1);
    }
  };

  const handleStartEdit = (index: number) => {
    setEditingIndex(index);
    setEditingValue(dictionary[index]);
  };

  const handleSaveEdit = () => {
    if (editingIndex !== null) {
      const word = editingValue.trim();
      // 检查是否与其他词条重复（排除当前编辑的词条）
      const isDuplicate = dictionary.some((w, i) => i !== editingIndex && w === word);
      if (isDuplicate) {
        setDuplicateHint(true);
        setTimeout(() => setDuplicateHint(false), 2000);
        return;
      }
      if (word) {
        setDictionary(prev => prev.map((w, i) => i === editingIndex ? word : w));
      }
      setEditingIndex(null);
      setEditingValue('');
    }
  };

  const handleCancelEdit = () => {
    setEditingIndex(null);
    setEditingValue('');
  };

  const handleSaveConfig = async () => {
    try {
      // 过滤空词条
      const validDictionary = dictionary.filter(w => w.trim());
      await invoke<string>("save_config", {
        apiKey,
        fallbackApiKey,
        useRealtime,
        enablePostProcess,
        llmConfig,
        smartCommandConfig,
        assistantConfig,
        asrConfig,
        dualHotkeyConfig,
        showTrayIcon,
        enableMuteOtherApps,
        dictionary: validDictionary
      });
      // 更新本地状态（移除空词条）
      setDictionary(validDictionary);

      // 如果服务正在运行，重启以应用新词库
      if (status === "running") {
        await invoke<string>("stop_app");
        await invoke<string>("start_app", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary: validDictionary
        });
      }

      setError(null);
      setShowSuccessToast(true);
      setTimeout(() => setShowSuccessToast(false), 3000);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleAutostartToggle = async () => {
    try {
      const newValue = !enableAutostart;
      await invoke<string>("set_autostart", { enabled: newValue });
      setEnableAutostart(newValue);
      setShowSuccessToast(true);
      setTimeout(() => setShowSuccessToast(false), 3000);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleCheckUpdate = async () => {
    try {
      setUpdateStatus("checking");
      const update = await check();

      if (update) {
        setUpdateInfo({
          version: update.version,
          notes: update.body || undefined
        });
        setUpdateStatus("available");
        setShowUpdateModal(true);
      } else {
        setUpdateStatus("idle");
        // 当前已是最新版本，显示提示
        setCopyToast("当前已是最新版本");
        setTimeout(() => setCopyToast(null), 2000);
      }
    } catch (err) {
      console.error("检查更新失败:", err);
      setUpdateStatus("idle");

      // 将技术错误转换为用户友好的中文提示
      const errorStr = String(err).toLowerCase();
      let errorMsg = "检查更新失败，请稍后重试";

      if (errorStr.includes("timeout") || errorStr.includes("timed out")) {
        errorMsg = "检查更新超时，请检查网络连接";
      } else if (errorStr.includes("network") || errorStr.includes("fetch") || errorStr.includes("connect")) {
        errorMsg = "网络连接失败，请检查网络设置";
      } else if (errorStr.includes("404") || errorStr.includes("not found")) {
        errorMsg = "未找到更新信息，可能尚未发布新版本";
      } else if (errorStr.includes("certificate") || errorStr.includes("ssl") || errorStr.includes("tls")) {
        errorMsg = "安全连接失败，请检查系统时间或网络环境";
      } else if (errorStr.includes("signature") || errorStr.includes("verify")) {
        errorMsg = "更新签名验证失败，请从官方渠道下载";
      }

      setError(errorMsg);
    }
  };

  const handleDownloadAndInstall = async () => {
    try {
      setUpdateStatus("downloading");
      const update = await check();

      if (update) {
        let downloaded = 0;
        let contentLength = 0;

        await update.downloadAndInstall((event) => {
          switch (event.event) {
            case 'Started':
              contentLength = event.data.contentLength || 0;
              break;
            case 'Progress':
              downloaded += event.data.chunkLength;
              if (contentLength > 0) {
                setDownloadProgress(Math.round((downloaded / contentLength) * 100));
              }
              break;
            case 'Finished':
              setDownloadProgress(100);
              break;
          }
        });

        setUpdateStatus("ready");
        // 自动重启应用
        await relaunch();
      }
    } catch (err) {
      console.error("下载更新失败:", err);
      setUpdateStatus("available");

      // 将技术错误转换为用户友好的中文提示
      const errorStr = String(err).toLowerCase();
      let errorMsg = "下载更新失败，请稍后重试";

      if (errorStr.includes("timeout") || errorStr.includes("timed out")) {
        errorMsg = "下载超时，请检查网络连接后重试";
      } else if (errorStr.includes("network") || errorStr.includes("fetch") || errorStr.includes("connect")) {
        errorMsg = "网络连接中断，请检查网络后重试";
      } else if (errorStr.includes("space") || errorStr.includes("disk")) {
        errorMsg = "磁盘空间不足，请清理后重试";
      } else if (errorStr.includes("permission") || errorStr.includes("access")) {
        errorMsg = "没有写入权限，请以管理员身份运行";
      } else if (errorStr.includes("signature") || errorStr.includes("verify")) {
        errorMsg = "安装包签名验证失败，请从官方渠道下载";
      }

      setError(errorMsg);
    }
  };

  const isAsrConfigValid = (config: AsrProviderConfig): boolean => {
    if (config.provider === 'qwen' || config.provider === 'siliconflow') {
      return config.api_key.trim() !== '';
    } else if (config.provider === 'doubao') {
      return (config.app_id?.trim() || '') !== '' && (config.access_token?.trim() || '') !== '';
    }
    return false;
  };

  const handleStartStop = async () => {
    try {
      if (status === "idle") {
        if (!isAsrConfigValid(asrConfig.primary)) {
          setError("请先配置 ASR API Key");
          return;
        }
        await invoke<string>("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          closeAction,
          dualHotkeyConfig,
          showTrayIcon,
          enableMuteOtherApps,
          dictionary
        });
        await invoke<string>("start_app", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          dualHotkeyConfig,
          enableMuteOtherApps,
          dictionary
        });
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

  const handleCloseAction = async (action: "close" | "minimize") => {
    if (rememberChoice) {
      setCloseAction(action);
      try {
        await invoke("save_config", {
          apiKey,
          fallbackApiKey,
          useRealtime,
          enablePostProcess,
          llmConfig,
          smartCommandConfig,
          assistantConfig,
          asrConfig,
          closeAction: action,
          dualHotkeyConfig,
          showTrayIcon,
        });
      } catch (err) {
        console.error("保存关闭配置失败:", err);
      }
    }
    setShowCloseDialog(false);
    setRememberChoice(false);
    if (action === "close") {
      await invoke("quit_app");
    } else {
      await invoke("hide_to_tray");
    }
  };

  // 热键相关辅助函数
  const mapDomKeyToHotkeyKey = (e: KeyboardEvent): HotkeyKey | null => {
    const { key, code, location } = e;

    // 修饰键（带位置）
    if (key === 'Control') return location === 1 ? 'control_left' : 'control_right';
    if (key === 'Shift') return location === 1 ? 'shift_left' : 'shift_right';
    if (key === 'Alt') return location === 1 ? 'alt_left' : 'alt_right';
    if (key === 'Meta') return location === 1 ? 'meta_left' : 'meta_right';

    // 特殊键
    if (key === ' ') return 'space';
    if (key === 'Tab') return 'tab';
    if (key === 'Escape') return 'escape';
    if (key === 'CapsLock') return 'caps_lock';

    // 功能键
    if (/^F([1-9]|1[0-2])$/.test(key)) {
      return `f${key.slice(1).toLowerCase()}` as HotkeyKey;
    }

    // 字母键
    if (/^Key[A-Z]$/.test(code)) {
      return `key_${code.slice(3).toLowerCase()}` as HotkeyKey;
    }

    // 数字键 (Top Row)
    if (/^Digit[0-9]$/.test(code)) {
      return `num_${code.slice(5)}` as HotkeyKey;
    }

    // 小键盘数字键 (Numpad)
    if (/^Numpad[0-9]$/.test(code)) {
      return `num_${code.slice(6)}` as HotkeyKey;
    }

    // 方向键
    if (key === 'ArrowUp') return 'up';
    if (key === 'ArrowDown') return 'down';
    if (key === 'ArrowLeft') return 'left';
    if (key === 'ArrowRight') return 'right';

    // 编辑键
    if (key === 'Enter') return 'return';
    if (key === 'Backspace') return 'backspace';
    if (key === 'Delete') return 'delete';
    if (key === 'Insert') return 'insert';
    if (key === 'Home') return 'home';
    if (key === 'End') return 'end';
    if (key === 'PageUp') return 'page_up';
    if (key === 'PageDown') return 'page_down';

    return null;
  };

  const isModifierKey = (key: HotkeyKey): boolean => {
    return ['control_left', 'control_right', 'shift_left', 'shift_right', 'alt_left', 'alt_right', 'meta_left', 'meta_right'].includes(key);
  };

  const formatHotkeyDisplay = (config: HotkeyConfig): string => {
    return config.keys.map(k => KEY_DISPLAY_NAMES[k] || k).join(' + ');
  };

  const resetHotkeyToDefault = (mode: 'dictation' | 'assistant') => {
    const defaultKeys = mode === 'dictation'
      ? { keys: ['control_left', 'meta_left'] as HotkeyKey[] }
      : { keys: ['alt_left', 'space'] as HotkeyKey[] };

    const newDualConfig = { ...dualHotkeyConfig };
    if (mode === 'dictation') {
      newDualConfig.dictation = defaultKeys;
    } else {
      newDualConfig.assistant = defaultKeys;
    }
    setDualHotkeyConfig(newDualConfig);
    handleSaveConfig();
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
  const handleCopyText = (text: string, e?: React.MouseEvent) => {
    if (e) {
      e.stopPropagation(); // 阻止事件冒泡
    }
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
            {/* 词典按钮 */}
            <button
              onClick={() => { setServiceModalTab('dictionary'); setShowServiceModal(true); }}
              className="p-2 rounded-lg bg-slate-100 hover:bg-orange-100 text-slate-400 hover:text-orange-600 transition-all"
              title="个人词典"
            >
              <BookText size={18} />
            </button>
            {/* 设置按钮 */}
            <button
              onClick={() => setShowSettingsModal(true)}
              className={`relative p-2 rounded-lg transition-all ${
                updateStatus === "available"
                  ? 'bg-green-100 text-green-600 hover:bg-green-200'
                  : 'bg-slate-100 text-slate-400 hover:bg-slate-200 hover:text-slate-500'
              }`}
              title="设置"
            >
              <Settings size={18} />
              {/* 新版本红点提示 */}
              {updateStatus === "available" && (
                <span className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 bg-red-500 rounded-full border-2 border-white" />
              )}
            </button>
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
                 status === "running" ? "运行中" : "已停止"}
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
                <div className="flex items-center gap-3">
                  <label className="text-xs font-bold text-slate-400 uppercase tracking-wider flex items-center gap-1">
                    <Activity size={14} />
                    {originalTranscript
                      ? (currentMode === 'assistant' ? 'AI 助手' : '转写结果')
                      : '实时转写内容'
                    }
                  </label>
                  {transcript && !originalTranscript && (
                    <button
                      onClick={(e) => handleCopyText(transcript, e)}
                      className="p-1.5 rounded-md bg-white/50 border border-slate-200/50 text-slate-400 hover:text-blue-600 hover:bg-blue-50 hover:border-blue-200 transition-all shadow-sm"
                      title="复制文本"
                    >
                      <Copy size={13} />
                    </button>
                  )}
                </div>
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
                    <div className="flex items-center justify-between mb-2">
                      <div className="text-xs text-slate-400 flex items-center gap-1">
                        <Mic size={12} /> {currentMode === 'assistant' ? '用户问题' : '原始转录'}
                      </div>
                      <button
                        onClick={(e) => handleCopyText(originalTranscript, e)}
                        className="p-1 rounded-md text-slate-400 hover:text-blue-600 hover:bg-blue-50 transition-all"
                        title="复制原始文本"
                      >
                        <Copy size={12} />
                      </button>
                    </div>
                    <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                      <p className="text-slate-500 text-sm leading-relaxed whitespace-pre-wrap">{originalTranscript}</p>
                    </div>
                  </div>
                  <div className="flex flex-col min-h-0">
                    <div className="flex items-center justify-between mb-2">
                      <div className="text-xs text-violet-500 flex items-center gap-1">
                        <Wand2 size={12} />
                        {currentMode === 'assistant'
                          ? 'AI 助手'
                          : `${llmConfig.presets.find(p => p.id === llmConfig.active_preset_id)?.name || "智能"}润色`
                        }
                      </div>
                      <button
                        onClick={(e) => handleCopyText(transcript, e)}
                        className="p-1 rounded-md text-violet-400 hover:text-violet-600 hover:bg-violet-50 transition-all"
                        title="复制结果"
                      >
                        <Copy size={12} />
                      </button>
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
              <h2>配置</h2>
            </div>

            {/* ASR 配置摘要卡片 */}
            <div className="flex items-center justify-between p-4 bg-gradient-to-r from-blue-50 to-indigo-50 rounded-xl border border-blue-100">
              <div className="flex items-center gap-3 flex-1">
                <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
                  <Mic size={18} />
                </div>
                <div className="flex-1">
                  <div className="text-sm font-medium text-slate-700 mb-1">ASR 语音识别</div>
                  <div className="text-xs text-slate-500 space-y-0.5">
                    <div>主模型：{ASR_PROVIDERS[asrConfig.primary.provider].name} · {ASR_PROVIDERS[asrConfig.primary.provider].model}</div>
                    <div>
                      备用：{asrConfig.fallback && asrConfig.enable_fallback
                        ? `${ASR_PROVIDERS[asrConfig.fallback.provider].name} · ${ASR_PROVIDERS[asrConfig.fallback.provider].model}`
                        : '未配置'}
                    </div>
                  </div>
                </div>
              </div>
              <button
                onClick={() => { setServiceModalTab('asr'); setShowServiceModal(true); }}
                disabled={isRunning}
                className="p-2 rounded-lg bg-blue-100 text-blue-600 hover:bg-blue-200 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="配置 ASR"
              >
                <Settings size={16} />
              </button>
            </div>

            {!isAsrConfigValid(asrConfig.primary) && (
              <div className="flex items-center gap-2 p-3 bg-amber-50 border border-amber-100 rounded-xl text-amber-600 text-xs animate-in slide-in-from-top-2 fade-in duration-300">
                <AlertCircle size={14} />
                <span>请点击设置按钮配置 ASR API Key</span>
              </div>
            )}

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
                    {enablePostProcess
                      ? (llmConfig.api_key ? '自动去重、润色转录文本' : '⚠️ 未配置 API Key')
                      : '直接输出原始转录'
                    }
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {enablePostProcess && (
                  <button
                    onClick={() => { setServiceModalTab('llm'); setShowServiceModal(true); }}
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
               <button
                 onClick={() => openUrl("https://ncn18msloi7t.feishu.cn/wiki/NFM3wAcWNi0IGTkUqkVckxWWntb")}
                 className="hover:text-blue-600 transition-colors flex items-center gap-1 group cursor-pointer"
               >
                 <BookOpen size={13} className="group-hover:scale-110 transition-transform"/>
                 使用教程 ↗
               </button>
               <button
                 onClick={() => openUrl("https://ncn18msloi7t.feishu.cn/wiki/ZnBZwSNjpisUdYkKks1cbes8nGb")}
                 className="hover:text-emerald-600 transition-colors flex items-center gap-1 group cursor-pointer"
               >
                 <KeyRound size={13} className="group-hover:scale-110 transition-transform"/>
                 API Key 申请 ↗
               </button>
               <button
                 onClick={() => openUrl("https://ncn18msloi7t.feishu.cn/wiki/EmTFwwtIfigqQDkXjBIc3oDonPd")}
                 className="hover:text-violet-600 transition-colors flex items-center gap-1 group cursor-pointer"
               >
                 <ScrollText size={13} className="group-hover:scale-110 transition-transform"/>
                 更新日志 ↗
               </button>
               <button
                 onClick={() => openUrl("https://github.com/yyyzl/push-2-talk")}
                 className="hover:text-slate-700 transition-colors flex items-center gap-1 group cursor-pointer"
               >
                 <Github size={13} className="group-hover:scale-110 transition-transform"/>
                 GitHub ↗
               </button>
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

      {/* 统一服务配置弹窗（三个 Tab：ASR、LLM 润色、AI 助手） */}
      {showServiceModal && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-4xl mx-4 h-[85vh] flex flex-col overflow-hidden animate-in zoom-in-95 duration-200">

            {/* Modal Header with Tabs */}
            <div className="px-6 py-4 border-b border-slate-100 bg-gradient-to-r from-slate-50 to-gray-50">
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-blue-100 rounded-xl text-blue-600">
                    <Settings size={20} />
                  </div>
                  <div>
                    <h3 className="text-lg font-bold text-slate-900">服务配置</h3>
                    <p className="text-xs text-slate-500">统一管理 ASR、LLM 润色和 AI 助手配置</p>
                  </div>
                </div>
                <button onClick={() => setShowServiceModal(false)} className="p-2 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-600 transition-colors">
                  <X size={20} />
                </button>
              </div>

              {/* Tab Bar */}
              <div className="flex gap-1 p-1 bg-slate-100 rounded-xl">
                <button
                  onClick={() => setServiceModalTab('asr')}
                  className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                    serviceModalTab === 'asr'
                      ? 'bg-white text-blue-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700'
                  }`}
                >
                  <Mic size={16} />
                  ASR 语音识别
                </button>
                <button
                  onClick={() => setServiceModalTab('llm')}
                  className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                    serviceModalTab === 'llm'
                      ? 'bg-white text-violet-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700'
                  }`}
                >
                  <Wand2 size={16} />
                  LLM 润色
                </button>
                <button
                  onClick={() => setServiceModalTab('assistant')}
                  className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                    serviceModalTab === 'assistant'
                      ? 'bg-white text-emerald-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700'
                  }`}
                >
                  <MessageSquareQuote size={16} />
                  AI 助手
                </button>
                <button
                  onClick={() => setServiceModalTab('dictionary')}
                  className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                    serviceModalTab === 'dictionary'
                      ? 'bg-white text-orange-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700'
                  }`}
                >
                  <BookText size={16} />
                  个人词典
                </button>
              </div>
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-hidden">

              {/* ASR Tab */}
              {serviceModalTab === 'asr' && (
                <div className="h-full overflow-y-auto p-6 space-y-6">
                  <div className="flex items-center gap-2 p-3 bg-blue-50 border border-blue-100 rounded-xl text-xs text-blue-700">
                    <AlertCircle size={14} className="flex-shrink-0" />
                    <span>ASR 用于语音转文字，支持阿里千问和豆包两种主模型，以及硅基移动作为备用模型</span>
                  </div>

                  {/* 主模型配置 */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-bold text-slate-700">主模型</h4>
                    <div className="space-y-3 p-4 bg-slate-50 rounded-xl border border-slate-200">
                      <div className="space-y-2">
                        <label className="text-xs font-medium text-slate-600">服务商</label>
                        <select
                          value={asrConfig.primary.provider}
                          onChange={(e) => {
                            const newProvider = e.target.value as AsrProvider;
                            setAsrConfig(prev => ({
                              ...prev,
                              primary: newProvider === 'qwen'
                                ? { provider: 'qwen', api_key: asrCache.qwen.api_key }
                                : { provider: 'doubao', api_key: '', app_id: asrCache.doubao.app_id, access_token: asrCache.doubao.access_token }
                            }));
                            setAsrCache(prev => ({ ...prev, active_provider: newProvider }));
                          }}
                          className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                        >
                          <option value="qwen">{ASR_PROVIDERS.qwen.name}</option>
                          <option value="doubao">{ASR_PROVIDERS.doubao.name}</option>
                        </select>
                      </div>

                      {asrConfig.primary.provider === 'qwen' ? (
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-slate-600">API Key</label>
                          <div className="relative">
                            <input
                              type={showApiKey ? "text" : "password"}
                              value={asrConfig.primary.api_key}
                              onChange={(e) => {
                                const value = e.target.value;
                                setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, api_key: value } }));
                                setAsrCache(prev => ({ ...prev, qwen: { api_key: value } }));
                              }}
                              className="w-full px-3 py-2 pr-10 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                              placeholder="sk-..."
                            />
                            <button onClick={() => setShowApiKey(!showApiKey)} className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600">
                              {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                            </button>
                          </div>
                        </div>
                      ) : (
                        <>
                          <div className="space-y-2">
                            <label className="text-xs font-medium text-slate-600">APP ID</label>
                            <input
                              type="text"
                              value={asrConfig.primary.app_id || ''}
                              onChange={(e) => {
                                const value = e.target.value;
                                setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, app_id: value } }));
                                setAsrCache(prev => ({ ...prev, doubao: { ...prev.doubao, app_id: value } }));
                              }}
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                              placeholder="输入豆包 APP ID"
                            />
                          </div>
                          <div className="space-y-2">
                            <label className="text-xs font-medium text-slate-600">Access Token</label>
                            <div className="relative">
                              <input
                                type={showApiKey ? "text" : "password"}
                                value={asrConfig.primary.access_token || ''}
                                onChange={(e) => {
                                  const value = e.target.value;
                                  setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, access_token: value } }));
                                  setAsrCache(prev => ({ ...prev, doubao: { ...prev.doubao, access_token: value } }));
                                }}
                                className="w-full px-3 py-2 pr-10 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                                placeholder="输入 Access Token"
                              />
                              <button onClick={() => setShowApiKey(!showApiKey)} className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600">
                                {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </div>
                          </div>
                        </>
                      )}
                      <div className="text-xs text-slate-500">模型：{ASR_PROVIDERS[asrConfig.primary.provider].model}</div>
                    </div>
                  </div>

                  {/* 备用模型配置 */}
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <h4 className="text-sm font-bold text-slate-700">备用模型（可选）</h4>
                      <button
                        onClick={() => setAsrConfig(prev => {
                          const isEnabling = !prev.enable_fallback;
                          return {
                            ...prev,
                            enable_fallback: isEnabling,
                            fallback: isEnabling && (!prev.fallback?.api_key)
                              ? { provider: 'siliconflow', api_key: asrCache.siliconflow.api_key }
                              : prev.fallback
                          };
                        })}
                        className={`relative w-11 h-6 rounded-full transition-all duration-300 ${asrConfig.enable_fallback ? 'bg-blue-500' : 'bg-slate-300'}`}
                      >
                        <span className={`absolute top-0.5 w-5 h-5 bg-white rounded-full shadow-md transition-all duration-300 ${asrConfig.enable_fallback ? 'left-5' : 'left-0.5'}`} />
                      </button>
                    </div>

                    {asrConfig.enable_fallback && (
                      <div className="space-y-3 p-4 bg-slate-50 rounded-xl border border-slate-200 animate-in slide-in-from-top-2 fade-in duration-300">
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-slate-600">服务商</label>
                          <select
                            value={asrConfig.fallback?.provider || 'siliconflow'}
                            onChange={(e) => setAsrConfig(prev => ({ ...prev, fallback: { provider: e.target.value as AsrProvider, api_key: prev.fallback?.api_key || '' } }))}
                            className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                          >
                            <option value="siliconflow">{ASR_PROVIDERS.siliconflow.name}</option>
                          </select>
                        </div>
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-slate-600">API Key</label>
                          <div className="relative">
                            <input
                              type={showApiKey ? "text" : "password"}
                              value={asrConfig.fallback?.api_key || ''}
                              onChange={(e) => {
                                const val = e.target.value;
                                setAsrConfig(prev => ({ ...prev, fallback: { provider: prev.fallback?.provider || 'siliconflow', api_key: val } }));
                                setAsrCache(prev => ({ ...prev, siliconflow: { api_key: val } }));
                              }}
                              className="w-full px-3 py-2 pr-10 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                              placeholder="sk-..."
                            />
                            <button onClick={() => setShowApiKey(!showApiKey)} className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600">
                              {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                            </button>
                          </div>
                        </div>
                        <div className="text-xs text-slate-500">模型：{ASR_PROVIDERS[asrConfig.fallback?.provider || 'siliconflow'].model}</div>
                        <div className="flex items-start gap-2 p-3 bg-blue-50 border border-blue-100 rounded-lg text-xs text-blue-700">
                          <AlertCircle size={14} className="mt-0.5 flex-shrink-0" />
                          <span>备用模型在主模型响应较慢时并行请求，先返回结果的模型优先使用</span>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* LLM Tab */}
              {serviceModalTab === 'llm' && (
                <div className="h-full flex overflow-hidden">
                  {/* Left Sidebar: Presets List */}
                  <div className="w-1/3 bg-slate-50 border-r border-slate-200 flex flex-col">
                    <div className="p-4 border-b border-slate-200 bg-slate-50/50">
                      <div className="flex items-center gap-2 p-2 mb-3 bg-violet-50 border border-violet-100 rounded-lg text-xs text-violet-700">
                        <AlertCircle size={12} className="flex-shrink-0" />
                        <span>Ctrl+Win 听写时使用</span>
                      </div>
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
                            <div className={`p-1.5 rounded-lg ${llmConfig.active_preset_id === preset.id ? 'bg-violet-100 text-violet-600' : 'bg-slate-200 text-slate-500'}`}>
                              <MessageSquareQuote size={14} />
                            </div>
                            <span className={`text-sm font-medium ${llmConfig.active_preset_id === preset.id ? 'text-slate-900' : 'text-slate-600'}`}>
                              {preset.name}
                            </span>
                          </div>
                          {llmConfig.presets.length > 1 && (
                            <button
                              onClick={(e) => { e.stopPropagation(); handleDeletePreset(preset.id); }}
                              className={`p-1.5 rounded-md text-slate-400 hover:bg-red-50 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100 ${llmConfig.active_preset_id === preset.id ? 'opacity-100' : ''}`}
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
                    <div className="flex-1 overflow-y-auto p-6 space-y-6">
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
                          className="w-full flex-1 min-h-[150px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                          placeholder="在这里定义 AI 的行为..."
                        />
                      </div>
                      <div className="h-px bg-slate-100 my-4"></div>
                      <div className="space-y-4">
                        <h4 className="text-xs font-bold text-slate-400 uppercase tracking-wider">模型设置</h4>
                        <div className="grid grid-cols-2 gap-4">
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
                              <button onClick={() => setShowApiKey(!showApiKey)} className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600">
                                {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </div>
                          </div>
                          <div className="space-y-1.5">
                            <label className="text-xs font-medium text-slate-500">模型名称</label>
                            <input
                              type="text"
                              value={llmConfig.model}
                              onChange={(e) => setLlmConfig({ ...llmConfig, model: e.target.value })}
                              className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                              placeholder="glm-4-flash"
                            />
                          </div>
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
              )}

              {/* AI Assistant Tab */}
              {serviceModalTab === 'assistant' && (
                <div className="h-full overflow-y-auto p-6 space-y-6">
                  <div className="flex items-center gap-2 p-3 bg-emerald-50 border border-emerald-100 rounded-xl text-xs text-emerald-700">
                    <AlertCircle size={14} className="flex-shrink-0" />
                    <span>AI 助手 ({formatHotkeyDisplay(dualHotkeyConfig.assistant)}) 可智能处理选中文本或回答问题，无需开关，配置 API 即可使用</span>
                  </div>

                  {/* 模型配置 */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-bold text-slate-700">模型配置</h4>
                    <div className="space-y-4 p-4 bg-slate-50 rounded-xl border border-slate-200">
                      <div className="space-y-2">
                        <label className="text-xs font-medium text-slate-600">API Key</label>
                        <div className="relative">
                          <input
                            type={showApiKey ? "text" : "password"}
                            value={assistantConfig.api_key}
                            onChange={(e) => setAssistantConfig(prev => ({ ...prev, api_key: e.target.value }))}
                            className="w-full px-3 py-2 pr-10 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all"
                            placeholder="sk-..."
                          />
                          <button onClick={() => setShowApiKey(!showApiKey)} className="absolute inset-y-0 right-0 pr-3 flex items-center text-slate-400 hover:text-slate-600">
                            {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </div>
                      </div>
                      <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-slate-600">模型名称</label>
                          <input
                            type="text"
                            value={assistantConfig.model}
                            onChange={(e) => setAssistantConfig(prev => ({ ...prev, model: e.target.value }))}
                            className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all"
                            placeholder="glm-4-flash"
                          />
                        </div>
                        <div className="space-y-2">
                          <label className="text-xs font-medium text-slate-600">API 地址</label>
                          <input
                            type="text"
                            value={assistantConfig.endpoint}
                            onChange={(e) => setAssistantConfig(prev => ({ ...prev, endpoint: e.target.value }))}
                            className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all"
                            placeholder="https://api..."
                          />
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* 问答模式提示词 */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-bold text-slate-700">问答模式提示词</h4>
                    <p className="text-xs text-slate-500">无选中文本时，AI 助手将使用此提示词回答问题</p>
                    <textarea
                      value={assistantConfig.qa_system_prompt}
                      onChange={(e) => setAssistantConfig(prev => ({ ...prev, qa_system_prompt: e.target.value }))}
                      className="w-full min-h-[120px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                      placeholder="定义 AI 助手如何回答问题..."
                    />
                  </div>

                  {/* 文本处理提示词 */}
                  <div className="space-y-4">
                    <h4 className="text-sm font-bold text-slate-700">文本处理提示词</h4>
                    <p className="text-xs text-slate-500">有选中文本时，AI 助手将使用此提示词处理文本（翻译、润色、总结等）</p>
                    <textarea
                      value={assistantConfig.text_processing_system_prompt}
                      onChange={(e) => setAssistantConfig(prev => ({ ...prev, text_processing_system_prompt: e.target.value }))}
                      className="w-full min-h-[120px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                      placeholder="定义 AI 助手如何处理选中的文本..."
                    />
                  </div>
                </div>
              )}

              {/* Dictionary Tab */}
              {serviceModalTab === 'dictionary' && (
                <div className="h-full overflow-y-auto p-6 space-y-6">
                  {/* 提示信息 */}
                  <div className="flex items-center gap-2 p-3 bg-orange-50 border border-orange-100 rounded-xl text-xs text-orange-700">
                    <AlertCircle size={14} className="flex-shrink-0" />
                    <span>添加常用词汇（专业术语、人名、产品名等），提升语音识别准确率</span>
                  </div>

                  {/* 添加词条 */}
                  <div className="space-y-2">
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={newWord}
                        onChange={(e) => { setNewWord(e.target.value); setDuplicateHint(false); }}
                        onKeyDown={(e) => e.key === 'Enter' && handleAddWord()}
                        className={`flex-1 px-4 py-2.5 bg-white border rounded-xl text-sm focus:outline-none focus:ring-2 transition-all ${
                          duplicateHint
                            ? 'border-red-300 focus:ring-red-500/20 focus:border-red-500'
                            : 'border-slate-200 focus:ring-orange-500/20 focus:border-orange-500'
                        }`}
                        placeholder="输入词汇，按回车添加"
                      />
                      <button
                        onClick={handleAddWord}
                        disabled={!newWord.trim()}
                        className="px-4 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-xl hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                      >
                        <Plus size={16} />
                        添加
                      </button>
                    </div>
                    {duplicateHint && (
                      <p className="text-xs text-red-500 pl-1">该词汇已存在</p>
                    )}
                  </div>

                  {/* 词条列表 - 气囊/标签云布局 */}
                  {dictionary.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-16 text-slate-300 space-y-3">
                      <BookText size={48} strokeWidth={1} />
                      <p className="text-sm text-slate-400">暂无词条</p>
                      <p className="text-xs text-slate-300">添加常用词汇，让语音识别更准确</p>
                    </div>
                  ) : (
                    <div>
                      <div className="text-xs text-slate-500 mb-3">共 {dictionary.length} 个词条</div>
                      <div className="flex flex-wrap gap-2">
                        {dictionary.map((word, index) => (
                          editingIndex === index ? (
                            /* 编辑模式 */
                            <div key={index} className="flex items-center gap-1 px-2 py-1 bg-white border-2 border-orange-400 rounded-full shadow-sm">
                              <input
                                type="text"
                                value={editingValue}
                                onChange={(e) => setEditingValue(e.target.value)}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') handleSaveEdit();
                                  if (e.key === 'Escape') handleCancelEdit();
                                }}
                                className="w-24 px-2 py-0.5 bg-transparent text-sm focus:outline-none text-slate-700"
                                autoFocus
                              />
                              <button onClick={handleSaveEdit} className="p-0.5 text-emerald-600 hover:text-emerald-700" title="保存">
                                <CheckCircle2 size={14} />
                              </button>
                              <button onClick={handleCancelEdit} className="p-0.5 text-slate-400 hover:text-slate-600" title="取消">
                                <X size={14} />
                              </button>
                            </div>
                          ) : (
                            /* 显示模式 - 气囊样式 */
                            <div
                              key={index}
                              className="group flex items-center gap-1.5 px-3 py-1.5 bg-white border border-slate-200 rounded-full text-sm text-slate-700 hover:border-orange-300 hover:shadow-sm transition-all cursor-default"
                            >
                              <span className="font-medium" onDoubleClick={() => handleStartEdit(index)}>{word}</span>
                              <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                                <button
                                  onClick={() => handleStartEdit(index)}
                                  className="p-0.5 text-slate-400 hover:text-orange-500 transition-colors"
                                  title="编辑"
                                >
                                  <Wand2 size={12} />
                                </button>
                                <button
                                  onClick={() => handleDeleteWord(index)}
                                  className="p-0.5 text-slate-400 hover:text-red-500 transition-colors"
                                  title="删除"
                                >
                                  <X size={12} />
                                </button>
                              </div>
                            </div>
                          )
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Modal Footer */}
            <div className="px-6 py-4 border-t border-slate-100 bg-slate-50/50 flex items-center justify-end gap-3">
              <button
                onClick={() => setShowServiceModal(false)}
                className="px-5 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 hover:bg-slate-100 rounded-xl transition-colors"
              >
                关闭
              </button>
              <button
                onClick={() => {
                  handleSaveConfig();
                  setShowServiceModal(false);
                }}
                className={`px-5 py-2.5 text-sm font-medium text-white rounded-xl shadow-lg transition-all ${
                  serviceModalTab === 'asr' ? 'bg-blue-500 hover:bg-blue-600 shadow-blue-500/20' :
                  serviceModalTab === 'llm' ? 'bg-violet-500 hover:bg-violet-600 shadow-violet-500/20' :
                  serviceModalTab === 'assistant' ? 'bg-emerald-500 hover:bg-emerald-600 shadow-emerald-500/20' :
                  'bg-orange-500 hover:bg-orange-600 shadow-orange-500/20'
                }`}
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
                    className={`p-4 rounded-xl border transition-all ${
                      record.success
                        ? 'bg-white border-slate-100 hover:border-blue-200 hover:shadow-md'
                        : 'bg-red-50/50 border-red-100'
                    }`}
                  >
                    <div className="flex items-center justify-between mb-3">
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
                        </div>
                      ) : (
                        <span className="text-[10px] bg-red-100 text-red-600 px-1.5 py-0.5 rounded">失败</span>
                      )}
                    </div>

                    {record.success ? (
                      record.polishedText ? (
                        // 有LLM处理：显示双栏（原始 + 处理后）
                        <div className="grid grid-cols-2 gap-3">
                          {/* 左侧：原始文本 */}
                          <div className="flex flex-col min-h-0 bg-slate-50/50 rounded-lg p-2.5">
                            <div className="flex items-center justify-between mb-2">
                              <div className="text-[11px] font-medium text-slate-500 flex items-center gap-1">
                                <Mic size={11} /> 原始转写
                              </div>
                              <button
                                onClick={(e) => handleCopyText(record.originalText, e)}
                                className="p-1.5 rounded-md bg-white border border-slate-200 hover:border-blue-400 hover:bg-blue-50 text-slate-500 hover:text-blue-600 transition-all shadow-sm hover:shadow"
                                title="复制原始文本"
                              >
                                <Copy size={13} />
                              </button>
                            </div>
                            <p className="text-xs text-slate-600 line-clamp-3 leading-relaxed">
                              {record.originalText}
                            </p>
                          </div>

                          {/* 右侧：处理后文本 */}
                          <div className="flex flex-col min-h-0 bg-violet-50/30 rounded-lg p-2.5">
                            <div className="flex items-center justify-between mb-2">
                              <div className="text-[11px] font-medium text-violet-600 flex items-center gap-1">
                                <Wand2 size={11} /> {record.presetName || '润色后'}
                              </div>
                              <button
                                onClick={(e) => handleCopyText(record.polishedText!, e)}
                                className="p-1.5 rounded-md bg-white border border-violet-200 hover:border-violet-400 hover:bg-violet-50 text-violet-500 hover:text-violet-600 transition-all shadow-sm hover:shadow"
                                title="复制处理后文本"
                              >
                                <Copy size={13} />
                              </button>
                            </div>
                            <p className="text-xs text-slate-700 line-clamp-3 leading-relaxed font-medium">
                              {record.polishedText}
                            </p>
                          </div>
                        </div>
                      ) : (
                        // 无LLM处理：单栏显示
                        <div className="flex flex-col bg-slate-50/50 rounded-lg p-3">
                          <div className="flex items-center justify-between mb-2">
                            <div className="text-[11px] font-medium text-slate-500">转写结果</div>
                            <button
                              onClick={(e) => handleCopyText(record.originalText, e)}
                              className="px-2.5 py-1.5 rounded-lg bg-white border border-slate-200 hover:border-blue-400 hover:bg-blue-50 text-slate-600 hover:text-blue-600 transition-all flex items-center gap-1.5 shadow-sm hover:shadow group"
                              title="复制文本"
                            >
                              <Copy size={14} />
                              <span className="text-xs font-medium">复制</span>
                            </button>
                          </div>
                          <p className="text-sm text-slate-700 line-clamp-3 leading-relaxed">
                            {record.originalText}
                          </p>
                        </div>
                      )
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

      {/* Close Confirmation Dialog */}
      {showCloseDialog && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 duration-200">
            {/* Dialog Header */}
            <div className="px-6 py-4 border-b border-slate-100 bg-gradient-to-r from-slate-50 to-gray-50">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-lg font-bold text-slate-900">关闭应用</h3>
                  <p className="text-xs text-slate-500">选择关闭方式</p>
                </div>
                <button
                  onClick={() => {
                    setShowCloseDialog(false);
                    setRememberChoice(false);
                  }}
                  className="p-2 hover:bg-slate-200 rounded-lg text-slate-500 hover:text-slate-700 transition-colors"
                >
                  <X size={18} />
                </button>
              </div>
            </div>

            {/* Dialog Body */}
            <div className="p-6 space-y-4">
              <p className="text-sm text-slate-600">您希望如何处理应用窗口？</p>

              <div className="space-y-3">
                <button
                  onClick={() => handleCloseAction("minimize")}
                  className="w-full p-4 bg-blue-50 hover:bg-blue-100 border border-blue-100 hover:border-blue-200 rounded-xl text-left transition-all group"
                >
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-blue-100 group-hover:bg-blue-200 rounded-lg text-blue-600 transition-colors">
                      <Minus size={18} />
                    </div>
                    <div>
                      <div className="text-sm font-medium text-slate-900">最小化到系统托盘</div>
                      <div className="text-xs text-slate-500">应用将在后台继续运行</div>
                    </div>
                  </div>
                </button>

                <button
                  onClick={() => handleCloseAction("close")}
                  className="w-full p-4 bg-slate-50 hover:bg-slate-100 border border-slate-100 hover:border-slate-200 rounded-xl text-left transition-all group"
                >
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-slate-100 group-hover:bg-slate-200 rounded-lg text-slate-600 transition-colors">
                      <XCircle size={18} />
                    </div>
                    <div>
                      <div className="text-sm font-medium text-slate-900">完全退出</div>
                      <div className="text-xs text-slate-500">关闭应用并停止所有服务</div>
                    </div>
                  </div>
                </button>
              </div>

              <label className="flex items-center gap-3 p-3 bg-slate-50/50 rounded-xl cursor-pointer hover:bg-slate-100/50 transition-colors">
                <input
                  type="checkbox"
                  checked={rememberChoice}
                  onChange={(e) => setRememberChoice(e.target.checked)}
                  className="w-4 h-4 rounded border-slate-300 text-blue-500 focus:ring-blue-500/20"
                />
                <span className="text-sm text-slate-600">记住我的选择，下次不再询问</span>
              </label>
            </div>
          </div>
        </div>
      )}

      {/* Update Modal */}
      {showUpdateModal && updateInfo && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 duration-200">
            {/* Modal Header */}
            <div className="px-6 py-4 border-b border-slate-100 bg-gradient-to-r from-green-50 to-emerald-50">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-green-100 rounded-xl text-green-600">
                    <Download size={20} />
                  </div>
                  <div>
                    <h3 className="text-lg font-bold text-slate-900">发现新版本</h3>
                    <p className="text-xs text-slate-500">v{updateInfo.version}</p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    setShowUpdateModal(false);
                    setUpdateStatus("idle");
                  }}
                  disabled={updateStatus === "downloading"}
                  className="p-2 hover:bg-slate-200 rounded-lg text-slate-500 hover:text-slate-700 transition-colors disabled:opacity-50"
                >
                  <X size={18} />
                </button>
              </div>
            </div>

            {/* Modal Body */}
            <div className="p-6 space-y-4">
              {updateInfo.notes && (
                <div className="p-4 bg-slate-50 rounded-xl border border-slate-100">
                  <h4 className="text-sm font-medium text-slate-700 mb-2">更新内容</h4>
                  <p className="text-sm text-slate-600 whitespace-pre-wrap">{updateInfo.notes}</p>
                </div>
              )}
              <p className="text-xs text-slate-400">
                查看完整更新日志：
                <button
                  onClick={() => openUrl("https://ncn18msloi7t.feishu.cn/wiki/EmTFwwtIfigqQDkXjBIc3oDonPd")}
                  className="text-blue-500 hover:text-blue-600 hover:underline ml-1"
                >
                  更新日志
                </button>
              </p>

              {updateStatus === "downloading" && (
                <div className="space-y-2">
                  <div className="flex justify-between text-sm text-slate-600">
                    <span>正在下载更新...</span>
                    <span>{downloadProgress}%</span>
                  </div>
                  <div className="w-full h-2 bg-slate-200 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-green-500 transition-all duration-300"
                      style={{ width: `${downloadProgress}%` }}
                    />
                  </div>
                </div>
              )}

              <div className="flex gap-3">
                <button
                  onClick={() => {
                    setShowUpdateModal(false);
                    setUpdateStatus("idle");
                  }}
                  disabled={updateStatus === "downloading"}
                  className="flex-1 px-4 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 bg-slate-100 hover:bg-slate-200 rounded-xl transition-colors disabled:opacity-50"
                >
                  稍后更新
                </button>
                <button
                  onClick={handleDownloadAndInstall}
                  disabled={updateStatus === "downloading"}
                  className="flex-1 px-4 py-2.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded-xl shadow-lg shadow-green-500/20 transition-all disabled:opacity-50 flex items-center justify-center gap-2"
                >
                  {updateStatus === "downloading" ? (
                    <>
                      <RefreshCw size={16} className="animate-spin" />
                      下载中...
                    </>
                  ) : (
                    <>
                      <Download size={16} />
                      立即更新
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Settings Modal - 匹配当前界面风格 */}
      {showSettingsModal && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
          {/* 增加 max-h-[85vh] 确保上下有留白，flex flex-col 支持内部滚动 */}
          <div className="bg-white rounded-2xl shadow-2xl w-full max-w-sm mx-4 max-h-[85vh] flex flex-col overflow-hidden animate-in zoom-in-95 duration-200">

            {/* Modal Header - 与其他弹窗风格一致 */}
            <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between bg-gradient-to-r from-slate-50 to-gray-50 flex-shrink-0">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-slate-100 rounded-xl text-slate-600">
                  <Settings size={20} />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-slate-900">设置</h3>
                  <p className="text-xs text-slate-500">应用偏好设置</p>
                </div>
              </div>
              <button
                onClick={() => setShowSettingsModal(false)}
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
                              showTrayIcon,
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
                      setShowSettingsModal(false);
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
                      setShowSettingsModal(false);
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
                      setShowSettingsModal(false);
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
                    <button
                      onClick={handleAutostartToggle}
                      className={`relative w-12 h-6 rounded-full transition-all duration-300 ${
                        enableAutostart ? 'bg-green-500' : 'bg-slate-300'
                      } cursor-pointer hover:opacity-90`}
                    >
                      <span
                        className="absolute top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-all duration-300"
                        style={{ left: enableAutostart ? 'calc(100% - 1.25rem - 2px)' : '2px' }}
                      />
                    </button>
                  </div>

                  {/* 系统托盘图标 */}
                  <div className="flex items-center justify-between p-3.5 hover:bg-slate-100/50 transition-colors">
                    <div className="flex items-center gap-3">
                      <div className={`p-1.5 rounded-lg transition-colors ${
                        showTrayIcon ? 'bg-blue-100 text-blue-600' : 'bg-slate-200 text-slate-500'
                      }`}>
                        <Minus size={16} />
                      </div>
                      <div>
                        <div className="text-sm font-medium text-slate-700">显示系统托盘图标</div>
                        <div className="text-[11px] text-slate-400 leading-tight">
                          {showTrayIcon ? '显示在任务栏通知区域' : '已隐藏（仍在后台运行）'}
                        </div>
                      </div>
                    </div>
                    <button
                      onClick={() => {
                        const next = !showTrayIcon;
                        setShowTrayIcon(next);
                        // 立即生效（Windows 任务栏通知区域）
                        invoke("set_tray_icon_visible", { visible: next }).catch((err) => {
                          console.error("设置托盘图标显示状态失败:", err);
                        });
                        // 同步保存到配置，避免重启后回退
                        invoke("save_config", {
                          apiKey,
                          fallbackApiKey,
                          useRealtime,
                          enablePostProcess,
                          llmConfig,
                          smartCommandConfig,
                          assistantConfig,
                          asrConfig,
                          closeAction,
                          dualHotkeyConfig,
                          showTrayIcon: next,
                          enableMuteOtherApps,
                          dictionary,
                        }).catch((err) => {
                          console.error("保存托盘图标配置失败:", err);
                        });
                      }}
                      className={`relative w-12 h-6 rounded-full transition-all duration-300 ${
                        showTrayIcon ? 'bg-blue-500' : 'bg-slate-300'
                      } cursor-pointer hover:opacity-90`}
                    >
                      <span
                        className="absolute top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-all duration-300"
                        style={{ left: showTrayIcon ? 'calc(100% - 1.25rem - 2px)' : '2px' }}
                      />
                    </button>
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
                    <button
                      onClick={() => setEnableMuteOtherApps(!enableMuteOtherApps)}
                      disabled={status === "running"}
                      className={`relative w-12 h-6 rounded-full transition-all duration-300 ${
                        enableMuteOtherApps ? 'bg-orange-500' : 'bg-slate-300'
                      } ${status === "running" ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:opacity-90'}`}
                    >
                      <span
                        className="absolute top-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-all duration-300"
                        style={{ left: enableMuteOtherApps ? 'calc(100% - 1.25rem - 2px)' : '2px' }}
                      />
                    </button>
                  </div>

                  {/* 检查更新 */}
                  <button
                    onClick={() => {
                      if (updateStatus === "available") {
                        setShowSettingsModal(false);
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
            <div className="px-4 py-3 bg-slate-50/50 border-t border-slate-100 flex-shrink-0">
              <p className="text-xs text-slate-400 text-center">
                听写: {formatHotkeyDisplay(dualHotkeyConfig.dictation)} · AI助手: {formatHotkeyDisplay(dualHotkeyConfig.assistant)}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Global Toast */}
      {copyToast && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-[100] bg-slate-900 text-white px-4 py-2 rounded-full text-sm font-medium shadow-lg animate-in fade-in zoom-in duration-200">
          {copyToast}
        </div>
      )}
    </div>
  );
}

export default App;
