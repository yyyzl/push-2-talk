//! TNL 类型定义

use serde::{Deserialize, Serialize};

/// 替换记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replacement {
    /// 原始文本
    pub original: String,
    /// 替换后文本
    pub replaced: String,
    /// 起始位置（字符索引）
    pub start: usize,
    /// 结束位置（字符索引）
    pub end: usize,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
    /// 替换原因
    pub reason: ReplacementReason,
}

/// 替换原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplacementReason {
    /// 口语符号映射（如 "点" → "."）
    SpokenSymbol,
    /// 词库精确匹配
    DictionaryExact,
    /// 词库模糊匹配（编辑距离）
    DictionaryFuzzy,
    /// 词库拼音匹配（中文）
    DictionaryPinyin,
    /// 词库音标匹配（英文）
    DictionaryPhonetic,
    /// 连续单字母合并（如 "T N L" → "TNL"）
    LetterMerge,
}

/// 技术片段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// 片段文本
    pub text: String,
    /// 起始位置（字符索引）
    pub start: usize,
    /// 结束位置（字符索引）
    pub end: usize,
    /// 片段类型
    pub span_type: SpanType,
}

/// 片段类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpanType {
    /// 文件名（如 readme.md）
    FileName,
    /// 路径（如 src/lib.rs）
    Path,
    /// 版本号（如 1.2.3）
    Version,
    /// 邮箱地址（如 test@example.com）
    Email,
    /// 通用技术串
    Technical,
}

/// 规范化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationResult {
    /// 规范化后的文本
    pub text: String,
    /// 是否有改动
    pub changed: bool,
    /// 高置信自动替换记录
    pub applied: Vec<Replacement>,
    /// 识别到的技术片段
    pub technical_spans: Vec<Span>,
    /// 处理耗时（微秒）
    pub elapsed_us: u64,
}

impl NormalizationResult {
    /// 创建无修改的结果
    pub fn unchanged(text: String, elapsed_us: u64) -> Self {
        Self {
            text,
            changed: false,
            applied: Vec::new(),
            technical_spans: Vec::new(),
            elapsed_us,
        }
    }
}
