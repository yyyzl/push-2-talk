// 热键类型定义
export type HotkeyKey =
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

export interface HotkeyConfig {
  keys: HotkeyKey[];
  enable_release_lock?: boolean;  // 已弃用，保留用于向后兼容
  release_mode_keys?: HotkeyKey[];  // 松手模式独立快捷键
}

// 双热键配置（听写模式 + AI助手模式）
export interface DualHotkeyConfig {
  dictation: HotkeyConfig;  // 听写模式（默认 Ctrl+Win）
  assistant: HotkeyConfig;  // AI助手模式（默认 Alt+Space）
}

// ASR 配置
export type AsrProvider = 'qwen' | 'doubao' | 'siliconflow';

export interface AsrCredentials {
  qwen_api_key: string;
  sensevoice_api_key: string;
  doubao_app_id: string;
  doubao_access_token: string;
}

export interface AsrSelection {
  active_provider: AsrProvider;
  enable_fallback: boolean;
  fallback_provider: AsrProvider | null;
}

export interface AsrConfig {
  credentials: AsrCredentials;
  selection: AsrSelection;
}

// LLM 配置
export interface LlmPreset {
  id: string;
  name: string;
  system_prompt: string;
}

export interface LlmConfig {
  endpoint: string;
  model: string;
  api_key: string;
  presets: LlmPreset[];
  active_preset_id: string;
}

// AI 助手配置（双系统提示词）
export interface AssistantConfig {
  enabled: boolean;
  endpoint: string;
  model: string;
  api_key: string;
  qa_system_prompt: string;               // 问答模式提示词（无选中文本时）
  text_processing_system_prompt: string;  // 文本处理提示词（有选中文本时）
}

// 应用配置
export interface AppConfig {
  dashscope_api_key: string;
  siliconflow_api_key: string;
  asr_config: AsrConfig;
  use_realtime_asr: boolean;
  enable_llm_post_process: boolean;
  llm_config: LlmConfig;
  assistant_config: AssistantConfig;
  close_action: "close" | "minimize" | null;
  hotkey_config: HotkeyConfig;            // 保留用于迁移
  dual_hotkey_config: DualHotkeyConfig;
  enable_mute_other_apps: boolean;
  dictionary: string[];
  theme: string;
}

// 转录结果
export interface TranscriptionResult {
  text: string;
  original_text: string | null;
  asr_time_ms: number;
  llm_time_ms: number | null;
  total_time_ms: number;
  mode?: string; // "normal" | "smartcommand"
  inserted?: boolean;
}

// 历史记录
export interface HistoryRecord {
  id: string;
  timestamp: number;
  originalText: string;
  polishedText: string | null;
  presetName: string | null;
  mode: "normal" | "assistant" | null;  // 处理模式
  asrTimeMs: number;
  llmTimeMs: number | null;
  totalTimeMs: number;
  success: boolean;
  errorMessage: string | null;
}

// ASR 服务商元数据
export interface AsrProviderMeta {
  name: string;
  model: string;
  docsUrl: string;
}

// 应用状态
export type AppStatus =
  | "idle"
  | "running"
  | "recording"
  | "transcribing"
  | "polishing"              // LLM 润色中
  | "assistant_processing";  // AI 助手处理中

// 更新状态
export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "ready";

// 使用统计
export interface UsageStats {
  totalRecordingMs: number;
  totalRecordingCount: number;
  totalRecognizedChars: number;
}

// 热键录制模式
export type HotkeyRecordingMode = 'dictation' | 'assistant' | 'release';
