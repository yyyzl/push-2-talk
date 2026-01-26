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

// LLM 提供商配置
export interface LlmProvider {
  id: string;  // 唯一标识，如 "zhipu", "openai"
  name: string;  // 显示名称，如 "智谱AI", "OpenAI"
  endpoint: string;
  api_key: string;
  default_model: string;
}

// 共享 LLM 配置（重构：支持多提供商）
export interface SharedLlmConfig {
  providers: LlmProvider[];  // 提供商列表
  default_provider_id: string;  // 默认提供商 ID
  // 功能默认绑定（可选，留空则使用 default_provider_id）
  polishing_provider_id?: string;
  polishing_model?: string;
  assistant_provider_id?: string;
  assistant_model?: string;
  learning_provider_id?: string;
  learning_model?: string;

  // 向后兼容字段（用于迁移）
  endpoint?: string;
  api_key?: string;
  default_model?: string;
}

// 功能特定 LLM 配置
export interface LlmFeatureConfig {
  use_shared: boolean;
  // 如果 use_shared=true，可选覆盖
  provider_id?: string;  // 覆盖提供商
  model?: string;  // 覆盖模型
  // 如果 use_shared=false，完全独立配置
  endpoint?: string;
  api_key?: string;
}

export interface LlmConfig {
  shared: SharedLlmConfig;
  feature_override: LlmFeatureConfig;
  presets: LlmPreset[];
  active_preset_id: string;
}

// AI 助手配置（双系统提示词）
export interface AssistantConfig {
  enabled: boolean;
  llm: LlmFeatureConfig;
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
  learning_config: LearningConfig;
  close_action: "close" | "minimize" | null;
  hotkey_config: HotkeyConfig;            // 保留用于迁移
  dual_hotkey_config: DualHotkeyConfig;
  enable_mute_other_apps: boolean;
  dictionary: string[];  // 简化格式："word" 或 "word|auto"
  theme: string;
}

// 词库条目
export interface DictionaryEntry {
  id: string;
  word: string;
  source: "manual" | "auto";
  added_at: number;  // Unix timestamp (seconds)
  frequency: number;
  last_used_at: number | null;  // Unix timestamp (seconds)
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

// ============================================================================
// 自动词库学习相关类型
// ============================================================================

/** 学习配置 */
export interface LearningConfig {
  enabled: boolean;
  observation_duration_secs: number;
  llm_endpoint: string | null;  // 保留用于向后兼容
  feature_override: LlmFeatureConfig;
}

/** 词库学习建议 */
export interface VocabularyLearningSuggestion {
  id: string;
  word: string;
  original: string;
  corrected: string;
  context: string;
  category: 'proper_noun' | 'term' | 'frequent';
  reason: string;
}
