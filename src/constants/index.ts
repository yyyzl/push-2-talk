import type { HotkeyKey, LlmPreset, LlmConfig, AssistantConfig, AsrProvider, AsrProviderMeta, LearningConfig, SharedLlmConfig } from '../types';

// 按键显示名称映射
export const KEY_DISPLAY_NAMES: Record<HotkeyKey, string> = {
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
    api_key: undefined
  },
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
export const ASR_PROVIDERS: Record<AsrProvider, AsrProviderMeta> = {
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

// 默认 ASR 缓存
export const DEFAULT_ASR_CACHE = {
  active_provider: 'qwen' as AsrProvider,
  qwen: { api_key: '' },
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

// 外部链接
export const EXTERNAL_LINKS = {
  tutorial: "https://ncn18msloi7t.feishu.cn/wiki/NFM3wAcWNi0IGTkUqkVckxWWntb",
  apiKeyGuide: "https://ncn18msloi7t.feishu.cn/wiki/ZnBZwSNjpisUdYkKks1cbes8nGb",
  changelog: "https://ncn18msloi7t.feishu.cn/wiki/EmTFwwtIfigqQDkXjBIc3oDonPd",
  github: "https://github.com/yyyzl/push-2-talk"
};
