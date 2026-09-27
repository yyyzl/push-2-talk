//! TNL 类型定义

use serde::{Deserialize, Serialize};

/// 替换记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replacement {
    /// 原始文本
    pub original: String,
    /// 替换后文本
    pub replaced: String,
    /// 起始位置（字节偏移）
    pub start: usize,
    /// 结束位置（字节偏移）
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

/// TNL 候选风险等级
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TnlCandidateRisk {
    /// 低风险：本地规则可直接应用
    Low,
    /// 中风险：需要 LLM 候选仲裁
    Medium,
    /// 高风险：本地保守拒绝
    High,
}

/// TNL 候选来源
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TnlCandidateSource {
    /// 口语符号映射
    SpokenSymbol,
    /// 词库精确匹配
    DictionaryExact,
    /// 词库模糊匹配
    DictionaryFuzzy,
    /// 中文拼音匹配
    DictionaryPinyin,
    /// 英文音近匹配
    DictionaryPhonetic,
    /// 连续单字母合并
    LetterMerge,
    /// 本地个性化纠错对候选
    PersonalizationCorrectionPair,
}

impl From<&ReplacementReason> for TnlCandidateSource {
    fn from(reason: &ReplacementReason) -> Self {
        match reason {
            ReplacementReason::SpokenSymbol => Self::SpokenSymbol,
            ReplacementReason::DictionaryExact => Self::DictionaryExact,
            ReplacementReason::DictionaryFuzzy => Self::DictionaryFuzzy,
            ReplacementReason::DictionaryPinyin => Self::DictionaryPinyin,
            ReplacementReason::DictionaryPhonetic => Self::DictionaryPhonetic,
            ReplacementReason::LetterMerge => Self::LetterMerge,
        }
    }
}

/// TNL 候选决策
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TnlCandidateDecision {
    /// 本地高置信规则已应用
    AppliedLocal,
    /// 等待 LLM 候选仲裁
    PendingLlm,
    /// LLM 仲裁后应用
    AppliedLlm,
    /// 本地低置信拒绝
    RejectedLocal,
    /// LLM 仲裁后拒绝
    RejectedLlm,
    /// 词库增强未开启，跳过仲裁
    SkippedDisabled,
    /// 未配置 LLM，跳过仲裁
    SkippedNoProcessor,
    /// LLM 仲裁超时，跳过仲裁
    SkippedTimeout,
    /// LLM 仲裁失败，跳过仲裁
    SkippedError,
    /// 超过单次仲裁候选数量上限，跳过仲裁
    SkippedLimit,
}

/// TNL 候选记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TnlCandidate {
    /// 候选唯一 ID
    pub id: String,
    /// 原始片段
    pub original: String,
    /// 目标热词
    pub target: String,
    /// 起始位置（UTF-8 字节偏移）
    pub start: usize,
    /// 结束位置（UTF-8 字节偏移）
    pub end: usize,
    /// 置信分数 (0.0 - 1.0)
    pub score: f32,
    /// 候选风险等级
    pub risk: TnlCandidateRisk,
    /// 候选来源
    pub source: TnlCandidateSource,
    /// 决策证据
    pub evidence: Vec<String>,
    /// 当前决策
    pub decision: TnlCandidateDecision,
}

impl TnlCandidate {
    /// 从本地替换记录构建已应用候选
    pub fn from_replacement(index: usize, replacement: &Replacement) -> Self {
        Self {
            id: format!("local-{}-{}-{}", replacement.start, replacement.end, index),
            original: replacement.original.clone(),
            target: replacement.replaced.clone(),
            start: replacement.start,
            end: replacement.end,
            score: replacement.confidence,
            risk: TnlCandidateRisk::Low,
            source: TnlCandidateSource::from(&replacement.reason),
            evidence: vec![format!("{:?}", replacement.reason)],
            decision: TnlCandidateDecision::AppliedLocal,
        }
    }
}

/// TNL 候选仲裁摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TnlArbitrationSummary {
    /// 是否实际尝试过 LLM 仲裁
    pub attempted: bool,
    /// 进入仲裁的候选数量
    pub candidate_count: usize,
    /// 仲裁应用数量
    pub applied_count: usize,
    /// 仲裁拒绝数量
    pub rejected_count: usize,
    /// 跳过数量
    pub skipped_count: usize,
    /// 仲裁耗时（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    /// 跳过或失败原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// TNL 诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TnlDiagnostics {
    /// 候选与决策列表
    pub candidates: Vec<TnlCandidate>,
    /// LLM 仲裁摘要
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arbitration: Option<TnlArbitrationSummary>,
}

impl TnlDiagnostics {
    /// 从候选列表构建诊断；空列表返回 None，便于兼容旧事件 payload
    pub fn from_candidates(candidates: Vec<TnlCandidate>) -> Option<Self> {
        if candidates.is_empty() {
            None
        } else {
            Some(Self {
                candidates,
                arbitration: None,
            })
        }
    }

    /// 是否存在等待 LLM 仲裁的候选
    pub fn has_pending_llm(&self) -> bool {
        self.candidates
            .iter()
            .any(|c| c.decision == TnlCandidateDecision::PendingLlm)
    }

    /// 等待 LLM 仲裁的候选数量
    pub fn pending_llm_count(&self) -> usize {
        self.candidates
            .iter()
            .filter(|c| c.decision == TnlCandidateDecision::PendingLlm)
            .count()
    }

    /// 将全部待仲裁候选标记为跳过
    pub fn mark_pending_skipped(
        &mut self,
        decision: TnlCandidateDecision,
        reason: impl Into<String>,
        elapsed_ms: Option<u64>,
    ) {
        let reason = reason.into();
        let mut skipped_count = 0usize;
        for candidate in &mut self.candidates {
            if candidate.decision == TnlCandidateDecision::PendingLlm {
                candidate.decision = decision.clone();
                candidate.evidence.push(reason.clone());
                skipped_count += 1;
            }
        }

        if skipped_count > 0 {
            self.arbitration = Some(TnlArbitrationSummary {
                attempted: false,
                candidate_count: skipped_count,
                applied_count: 0,
                rejected_count: 0,
                skipped_count,
                elapsed_ms,
                reason: Some(reason),
            });
        }
    }
}

/// TNL 候选仲裁结果
#[derive(Debug, Clone)]
pub struct TnlCandidateArbitrationResult {
    /// 仲裁后的文本
    pub text: String,
    /// 更新后的诊断信息
    pub diagnostics: TnlDiagnostics,
    /// LLM 仲裁耗时（毫秒）
    pub elapsed_ms: u64,
}

/// 技术片段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// 片段文本
    pub text: String,
    /// 起始位置（字节偏移）
    pub start: usize,
    /// 结束位置（字节偏移）
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
    /// URL/域名（如 https://example.com, github.com）
    Url,
    /// CLI flag（如 --verbose, -p）
    CliFlag,
    /// 标识符（如 getElementById, TypeScript）
    Identifier,
    /// 版本号（如 1.2.3）
    Version,
    /// 邮箱地址（如 test@example.com）
    Email,
    /// 用户词典专名（如产品、人名、术语）
    NamedEntity,
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
    /// 可选 TNL 诊断信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<TnlDiagnostics>,
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
            diagnostics: None,
        }
    }
}
