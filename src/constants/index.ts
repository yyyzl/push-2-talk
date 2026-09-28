import { desktopOs, platformKeyLabels } from "../utils/platform";
import type {
  AssistantConfig,
  AsrProvider,
  AsrProviderMeta,
  QwenAsrProfile,
  DisfluencyMode,
  HotkeyKey,
  LearningConfig,
  LlmConfig,
  LlmPreset,
  SearchConfig,
  SharedLlmConfig,
  TnlConfig,
} from '../types';

// 按键显示名称映射
export const KEY_DISPLAY_NAMES: Record<HotkeyKey, string> = {
  control_left: 'Ctrl(左)', control_right: 'Ctrl(右)',
  shift_left: 'Shift(左)', shift_right: 'Shift(右)',
  ...platformKeyLabels(desktopOs),
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

// 历史记录
export const HISTORY_KEY = 'pushtotalk_history';
export const MAX_HISTORY = 50;

// 使用统计
export const USAGE_STATS_KEY = 'pushtotalk_usage_stats_v1';

// ASR 缓存 key
export const ASR_CACHE_STORAGE_KEY = 'pushtotalk_asr_cache';

// 默认 LLM 预设
export const DEFAULT_PRESETS: LlmPreset[] = [
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

// 默认共享 LLM 配置
export const DEFAULT_SHARED_LLM_CONFIG: SharedLlmConfig = {
  providers: [],
  default_provider_id: "",
  polishing_provider_id: undefined,
  polishing_model: undefined,
  assistant_provider_id: undefined,
  assistant_model: undefined,
  learning_provider_id: undefined,
  learning_model: undefined
};

// 默认 LLM 配置
export const DEFAULT_LLM_CONFIG: LlmConfig = {
  shared: DEFAULT_SHARED_LLM_CONFIG,
  feature_override: {
    use_shared: true,
    provider_id: undefined,
    model: undefined,
    endpoint: undefined,
    api_key: undefined
  },
  presets: DEFAULT_PRESETS,
  active_preset_id: "polishing"
};

// AI 助手默认配置
export const DEFAULT_ASSISTANT_CONFIG: AssistantConfig = {
  enabled: false,
  llm: {
    use_shared: true,
    provider_id: undefined,
    model: undefined,
    endpoint: undefined,
    api_key: undefined,
    reasoning: undefined,
    custom_body: undefined
  },
  qa_llm: undefined,
  text_processing_llm: undefined,
  qa_system_prompt: `你是一个智能语音助手。用户会通过语音向你提问，你需要：
1. 理解用户的问题
2. 给出简洁、准确、有用的回答
3. 如果问题不够明确，给出最可能的解答
注意：
- 回答要简洁明了，适合直接粘贴使用
- 避免过多的解释和废话
- 如果是代码相关问题，直接给出代码`,
  text_processing_system_prompt: `你是一个选区上下文助手。用户会选中一段文本，然后用语音或文字提出问题/指令。你需要：
1. 始终先阅读【本轮选中文本（主要上下文）】，把它作为本轮回答的主要依据。
2. 判断用户意图：是编辑类任务，还是基于选中文本回答问题、解释、分析、提建议。
3. 编辑类任务（润色、翻译、总结、改写、扩写、修复语法等）：直接输出处理后的文本，不要添加“这是修改后的版本”等前缀。
4. 问答/解释/分析类任务：围绕选中文本给出清晰回答，可以引用关键点，但不要脱离选区泛泛回答。
5. 保持原文格式和结构，除非用户明确要求改变。

如果指令不明确，优先根据选中文本给出最可能有用的处理结果；确实无法判断时，只提出一个简短澄清问题。`,
  enable_web_search: false,
  web_search_max_loops: 3,
  web_search_in_text_mode: false
};

export const DEFAULT_SEARCH_CONFIG: SearchConfig = {
  providers: [],
  default_provider_id: null,
  max_results: 5,
  timeout_secs: 6,
  enable_fallback: true
};

export const DEFAULT_TNL_CONFIG: TnlConfig = {
  enabled: true,
  disfluency_mode: "conservative",
  enable_personalization_exact_text_pass: true,
  enable_personalization_syllable_match_pass: true,
  enable_personalization_hotwords: true,
  enable_context_hotwords: true,
  personalization_max_window_tokens: 5,
  personalization_apply_threshold: 0.88
};

const isDisfluencyMode = (mode: unknown): mode is DisfluencyMode =>
  mode === "off" || mode === "conservative" || mode === "aggressive";

export function normalizeTnlConfig(
  tnlConfig: Partial<TnlConfig> | null | undefined,
): TnlConfig {
  tnlConfig = tnlConfig ?? {};

  const maxWindowTokens = Number(tnlConfig.personalization_max_window_tokens);
  const applyThreshold = Number(tnlConfig.personalization_apply_threshold);

  return {
    ...DEFAULT_TNL_CONFIG,
    ...tnlConfig,
    enable_personalization_exact_text_pass: tnlConfig.enable_personalization_exact_text_pass ?? false,
    enable_personalization_syllable_match_pass: tnlConfig.enable_personalization_syllable_match_pass ?? false,
    enable_personalization_hotwords: tnlConfig.enable_personalization_hotwords ?? false,
    enable_context_hotwords: tnlConfig.enable_context_hotwords ?? false,
    disfluency_mode: isDisfluencyMode(tnlConfig.disfluency_mode)
      ? tnlConfig.disfluency_mode
      : tnlConfig.disfluency_mode == null ? "off" : DEFAULT_TNL_CONFIG.disfluency_mode,
    personalization_max_window_tokens:
      Number.isFinite(maxWindowTokens) && maxWindowTokens > 0
        ? maxWindowTokens
        : DEFAULT_TNL_CONFIG.personalization_max_window_tokens,
    personalization_apply_threshold:
      Number.isFinite(applyThreshold) && applyThreshold > 0 && applyThreshold <= 1
        ? applyThreshold
        : DEFAULT_TNL_CONFIG.personalization_apply_threshold,
  };
}

// ASR 服务商元数据
export const DEFAULT_QWEN_ASR_PROFILE: QwenAsrProfile = 'qwen_audio_3_1';

export const QWEN_ASR_PROFILES: Record<
  QwenAsrProfile,
  {
    name: string;
    httpModel: string;
    realtimeModel: string;
    description: string;
  }
> = {
  qwen_audio_3_1: {
    name: 'Qwen Audio 3.1 ASR（默认）',
    httpModel: 'qwen-audio-3.1-asr-flash',
    realtimeModel: 'qwen-audio-3.1-asr-flash-streaming',
    description: '新版多语种与方言识别，支持热词增强',
  },
  qwen_audio_3: {
    name: 'Qwen Audio 3.0 ASR',
    httpModel: 'qwen-audio-3.0-asr-flash',
    realtimeModel: 'qwen-audio-3.0-asr-flash-streaming',
    description: '多语种与中文方言能力更强，支持新版即时热词协议',
  },
  qwen3_legacy: {
    name: 'Qwen3 ASR Flash（旧版兼容）',
    httpModel: 'qwen3-asr-flash',
    realtimeModel: 'qwen3-asr-flash-realtime',
    description: '保留原有 HTTP 与 Realtime 协议，便于兼容旧环境',
  },
};

/** 保留明确选择的版本，并兼容修复前落盘的 3.0 名称。 */
export function normalizeQwenAsrProfile(profile: unknown): QwenAsrProfile {
  if (profile == null) return 'qwen3_legacy';
  if (profile === 'qwen_audio3') return 'qwen_audio_3';
  return typeof profile === 'string' && Object.prototype.hasOwnProperty.call(QWEN_ASR_PROFILES, profile)
    ? profile as QwenAsrProfile
    : DEFAULT_QWEN_ASR_PROFILE;
}

export const ASR_PROVIDERS: Record<AsrProvider, AsrProviderMeta> = {
  qwen: {
    name: '阿里千问',
    model: QWEN_ASR_PROFILES[DEFAULT_QWEN_ASR_PROFILE].httpModel,
    docsUrl: 'https://help.aliyun.com/zh/model-studio/asr-model',
  },
  doubao: {
    name: '豆包',
    model: 'Doubao-Seed-ASR-2.0',
    docsUrl: 'https://www.volcengine.com/docs/6561',
  },
  doubao_ime: {
    name: '豆包输入法',
    model: '免费 (自动注册)',
    docsUrl: '',
  },
  siliconflow: {
    name: '硅基移动',
    model: 'SenseVoiceSmall',
    docsUrl: 'https://cloud.siliconflow.cn/',
  },
};

// 默认双热键配置
export const DEFAULT_DUAL_HOTKEY_CONFIG = {
  dictation: {
    keys: ['control_left', 'meta_left'] as HotkeyKey[],
    release_mode_keys: ['f2'] as HotkeyKey[],
  },
  assistant: { keys: ['alt_left', 'space'] as HotkeyKey[] }
};

// Key 缺失时自动回退的 ASR Provider
export const FALLBACK_ASR_PROVIDER = 'doubao_ime' as AsrProvider;

// 合法的 ASR Provider 列表（用于 localStorage 迁移校验等）
export const VALID_ASR_PROVIDERS: AsrProvider[] = ['qwen', 'doubao', 'doubao_ime', 'siliconflow'];

// 默认 ASR 缓存
export const DEFAULT_ASR_CACHE = {
  active_provider: 'doubao_ime' as AsrProvider,
  qwen: { api_key: '', profile: DEFAULT_QWEN_ASR_PROFILE },
  doubao: { app_id: '', access_token: '' },
  doubao_ime: { device_id: '', token: '', cdid: '' },
  siliconflow: { api_key: '' }
};

// 默认自动学习配置
export const DEFAULT_LEARNING_CONFIG: LearningConfig = {
  enabled: false,
  observation_duration_secs: 15,
  llm_endpoint: null,
  feature_override: {
    use_shared: true,
    provider_id: undefined,
    model: undefined,
    endpoint: undefined,
    api_key: undefined
  }
};

export function normalizeLearningConfig(
  learningConfig: Partial<LearningConfig> | null | undefined,
): LearningConfig {
  if (!learningConfig) return DEFAULT_LEARNING_CONFIG;

  return {
    ...DEFAULT_LEARNING_CONFIG,
    ...learningConfig,
    feature_override: {
      ...DEFAULT_LEARNING_CONFIG.feature_override,
      ...(learningConfig.feature_override || {}),
    },
  };
}

// 外部链接
export const EXTERNAL_LINKS = {
  tutorial: "https://ncn18msloi7t.feishu.cn/wiki/NFM3wAcWNi0IGTkUqkVckxWWntb",
  apiKeyGuide: "https://ncn18msloi7t.feishu.cn/wiki/ZnBZwSNjpisUdYkKks1cbes8nGb",
  changelog: "https://ncn18msloi7t.feishu.cn/wiki/EmTFwwtIfigqQDkXjBIc3oDonPd",
  github: "https://github.com/yyyzl/push-2-talk"
};
