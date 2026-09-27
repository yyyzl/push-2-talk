// 自动词库学习相关类型定义

import type { DictionaryCategory } from "./index";

export type LearningSuggestionCategory =
  | DictionaryCategory
  | "proper_noun"
  | "term"
  | "frequent";

/** 学习配置 */
export interface LearningConfig {
  enabled: boolean;
  observation_duration_secs: number;
  llm_endpoint: string | null;
}

/** 词库学习建议 */
export interface VocabularyLearningSuggestion {
  id: string;
  word: string;
  original: string;
  corrected: string;
  context: string;
  category: LearningSuggestionCategory;
  reason: string;
  already_in_dictionary?: boolean;
}
