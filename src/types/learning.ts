// 自动词库学习相关类型定义

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
  category: 'proper_noun' | 'term' | 'frequent';
  reason: string;
}
