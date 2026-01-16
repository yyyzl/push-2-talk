// src/utils/errorParser.ts
// 错误解析工具 - 将原始错误消息转换为用户友好的信息

export type ErrorCategory = 'audio' | 'network' | 'auth' | 'service' | 'unknown';

export interface FriendlyError {
  category: ErrorCategory;
  title: string;
  suggestion: string;
  details: string;
}

export const ERROR_ICONS: Record<ErrorCategory, string> = {
  audio: '🎤',
  network: '🌐',
  auth: '🔑',
  service: '⚠️',
  unknown: '❓',
};

// 错误匹配模式配置
interface ErrorPattern {
  // 匹配函数：接收原始错误和小写版本
  match: (raw: string, lower: string) => boolean;
  category: ErrorCategory;
  title: string;
  suggestion: string;
}

// 配置驱动的错误模式列表（按优先级排序）
const ERROR_PATTERNS: ErrorPattern[] = [
  // --- 音频相关错误 ---
  {
    match: (raw) => raw.includes('录音器未初始化') || raw.includes('流式录音器未初始化'),
    category: 'audio',
    title: '音频系统未就绪',
    suggestion: '请重启应用后重试',
  },
  {
    match: (raw) => raw.includes('录音失败'),
    category: 'audio',
    title: '麦克风启动失败',
    suggestion: '请检查麦克风权限和设备连接',
  },
  {
    match: (raw) => raw.includes('停止录音失败'),
    category: 'audio',
    title: '录音停止异常',
    suggestion: '请重试',
  },
  {
    match: (raw) => raw.includes('没有录制到音频数据'),
    category: 'audio',
    title: '未检测到语音',
    suggestion: '请检查麦克风是否静音或被其他应用占用',
  },

  // --- 认证相关错误 ---
  {
    match: (raw, lower) =>
      lower.includes('401') ||
      lower.includes('403') ||
      lower.includes('unauthorized') ||
      lower.includes('forbidden') ||
      (lower.includes('invalid') && lower.includes('key')) ||  // 修复：明确括号
      raw.includes('密钥') ||
      raw.includes('认证'),
    category: 'auth',
    title: 'API 密钥无效',
    suggestion: '请检查设置中的服务密钥配置',
  },

  // --- 网络超时错误 ---
  {
    match: (raw, lower) =>
      lower.includes('timeout') ||
      raw.includes('超时') ||
      lower.includes('timed out'),
    category: 'network',
    title: '网络连接超时',
    suggestion: '请检查网络连接后重试',
  },

  // --- 服务错误 ---
  {
    match: (_, lower) =>
      lower.includes('500') ||
      lower.includes('502') ||
      lower.includes('503') ||
      lower.includes('504') ||
      (lower.includes('service') && lower.includes('unavailable')),  // 修复：明确括号
    category: 'service',
    title: '服务暂时不可用',
    suggestion: '请稍后重试',
  },

  // --- 网络连接错误 ---
  {
    match: (raw, lower) =>
      lower.includes('network') ||
      lower.includes('connection') ||
      lower.includes('fetch') ||
      lower.includes('dns') ||
      raw.includes('连接'),
    category: 'network',
    title: '网络连接失败',
    suggestion: '请检查网络设置后重试',
  },

  // --- 转录失败（通用）---
  {
    match: (raw) => raw.includes('转录失败'),
    category: 'network',
    title: '语音识别失败',
    suggestion: '请检查网络连接或稍后重试',
  },

  // --- AI 助手失败 ---
  {
    match: (raw) => raw.includes('AI 助手处理失败'),
    category: 'network',
    title: 'AI 处理失败',
    suggestion: '请检查网络连接和 AI 服务配置',
  },
];

// 默认错误（未匹配任何模式时）
const DEFAULT_ERROR: Omit<FriendlyError, 'details'> = {
  category: 'unknown',
  title: '操作失败',
  suggestion: '请重试或检查配置',
};

/**
 * 将原始错误消息解析为用户友好的错误信息
 */
export const parseError = (rawError: string | null | undefined): FriendlyError => {
  // 防御性检查
  if (!rawError) {
    return {
      ...DEFAULT_ERROR,
      details: '',
    };
  }

  const lowerError = rawError.toLowerCase();

  // 查找第一个匹配的模式
  const matchedPattern = ERROR_PATTERNS.find((pattern) =>
    pattern.match(rawError, lowerError)
  );

  if (matchedPattern) {
    return {
      category: matchedPattern.category,
      title: matchedPattern.title,
      suggestion: matchedPattern.suggestion,
      details: rawError,
    };
  }

  // 返回默认错误
  return {
    ...DEFAULT_ERROR,
    details: rawError,
  };
};
