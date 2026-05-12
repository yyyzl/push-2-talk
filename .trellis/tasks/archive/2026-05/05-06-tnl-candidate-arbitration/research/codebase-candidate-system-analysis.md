# Codebase Research: TNL 候选仲裁系统

## 当前链路

普通听写链路大致是：

1. `handle_transcription_result` 从 `AppState` 读取运行时词库、润色开关、词库增强开关。
2. `NormalPipeline.process` 接收 ASR 文本。
3. `TnlEngine::normalize` 同步执行本地规范化，返回 `NormalizationResult`。
4. `NormalPipeline::maybe_polish` 在启用语句润色或词库增强时调用 `LlmPostProcessor::polish_transcript`。
5. `PipelineResult` 被 `handle_transcription_result` 转成 `TranscriptionResult`，通过 `transcription_complete` 事件给前端。
6. `useTauriEventListeners` 消费 `TranscriptionResult`，更新当前结果状态，并写入 `HistoryRecord`。

## 现有能力

后端已有：

* ASR hotwords/context 注入：豆包 HTTP 和实时路径都会把运行时词库注入 ASR 请求。
* TNL 本地处理：字母合并、技术片段、口语符号、拼音替换、连字符词库重写、英文音近替换。
* TNL 误伤保护：常见英文词保护、动态阈值、编辑距离硬阈值、歧义 margin suppression。
* LLM 词库增强：目前给模型全文和词库，让模型直接输出最终文本。
* 自动词库学习：用户修改插入后的文本后，后台观察 diff，并用 LLM 判断是否加入词库。

前端已有：

* `TranscriptionResult` 当前结果类型。
* `HistoryRecord` 本地历史记录类型。
* `useTauriEventListeners` 统一消费后端事件并写入历史。
* `HistoryPage` / `HistoryDrawer` 显示原始转写、润色后文本、耗时和模式标签。

## 缺口

1. **没有候选输出通道**

   `NormalizationResult` 只有 `applied`，没有 `candidates`、`rejected`、`ambiguous`。因此中置信候选无法进入后续决策，也无法展示给前端。

2. **TNL 当前是命中即改**

   拼音、连字符、英文音近路径都会在 `normalize` 内直接返回替换文本。适合高置信规则，但不适合边缘候选的语义判定。

3. **LLM 当前粒度太粗**

   `LlmPostProcessor` 的词库增强是全文改写。候选系统需要的是短 JSON：只判断候选是否替换，不自由改写全文。

4. **前后端类型没有诊断字段**

   `PipelineResult` 和 `TranscriptionResult` 没有携带 TNL 决策摘要。前端历史也不能保存“为什么替换/为什么没替换”。

5. **词库结构表达力不足**

   运行时词库仍是 `Vec<String>`，无法表达 `word/aliases/lang/risk/domain`。MVP 可以先从词条自动派生候选特征，但长期应演进成结构化词库。

## GitNexus 影响分析

* `TnlEngine.normalize` 上游影响为 CRITICAL：影响普通听写、助手模式、`start_app`、`handle_realtime_stop` 和大量 TNL 回归测试。
* 后端 `TranscriptionResult` 影响为 HIGH：影响 `handle_transcription_result`、对话历史事件等链路。
* 前端 `TranscriptionResult` 类型影响为 CRITICAL：被大量页面、窗口、hooks 和测试引用。
* `PipelineResult` 结构本身在索引中的直接影响低，但它是后端转前端事件的自然扩展点。

结论：设计必须以“新增可选字段 + 保持原行为兼容”为优先。不要改变现有 `text/original_text/asr_time_ms/llm_time_ms` 语义。

## 推荐后端架构

### 数据结构

新增 TNL 候选相关类型，建议放在 `src-tauri/src/tnl/types.rs`：

```rust
pub struct TnlCandidate {
    pub id: String,
    pub original: String,
    pub target: String,
    pub start: usize,
    pub end: usize,
    pub score: f32,
    pub risk: CandidateRisk,
    pub source: CandidateSource,
    pub evidence: Vec<String>,
    pub decision: CandidateDecision,
}

pub enum CandidateDecision {
    AppliedLocal,
    PendingLlm,
    AppliedLlm,
    RejectedLocal,
    RejectedLlm,
    SkippedTimeout,
}
```

`NormalizationResult` 增加：

```rust
pub candidates: Vec<TnlCandidate>,
pub arbitration: Option<TnlArbitrationSummary>,
```

这些字段只做附加信息，不改变 `text/changed/applied` 原语义。

### 执行分层

推荐分成三层：

1. **Local normalizer**

   保持现在高置信规则继续直接应用，例如口语符号、连字符精确重写、声调完全一致且无冲突的中文拼音、明确的字母合并。

2. **Candidate collector/scorer**

   对音近英文、多 token 英文、中文同音/近音、上下文强相关热词生成候选。分数分为：

   * `>= 0.88`：本地自动应用。
   * `0.68..0.88`：中置信候选，进入 LLM 仲裁。
   * `< 0.68`：保留为 rejected/diagnostic，不参与替换。

   阈值应先作为常量，后续再暴露配置。

3. **Candidate arbitrator**

   新增独立轻量组件，例如 `candidate_arbitrator.rs` 或 `tnl/arbitrator.rs`。它接收：

   * TNL 后文本。
   * 候选列表。
   * 每个候选的短上下文。
   * 用户词库目标词。

   返回 JSON 决策，只允许替换候选 span，不允许全文润色。

### 与 LLM 润色的关系

推荐顺序：

```text
ASR 原文
  -> TNL 本地高置信规范化 + 中置信候选召回
  -> 候选 LLM 仲裁（短超时，可跳过）
  -> 可选全文润色/词库增强
  -> 插入文本
  -> transcription_complete 携带诊断摘要
```

需要注意：如果已经开启全文润色，候选仲裁仍然有价值，因为它在润色前把关键热词先纠正，降低全文模型跑偏概率。

## 推荐前后端交互

### 后端事件 payload

在后端 `TranscriptionResult` 增加可选字段：

```rust
#[serde(skip_serializing_if = "Option::is_none")]
tnl_diagnostics: Option<TnlDiagnostics>,
```

前端 `TranscriptionResult` 增加同名可选字段：

```ts
tnl_diagnostics?: TnlDiagnostics;
```

`HistoryRecord` 也增加可选字段：

```ts
tnlDiagnostics?: TnlDiagnostics;
```

旧历史记录没有该字段时正常渲染。

### 前端展示

MVP 推荐先放在历史详情，不做录音过程实时弹层：

* 历史记录卡片上显示一个小标签，例如 `热词修正 2`。
* 展开或详情区域显示：
  * `Cloud Code -> Claude Code`
  * 来源：`音近匹配 + 开发上下文`
  * 决策：`本地应用` / `模型确认` / `模型拒绝` / `超时保留`
  * 耗时：候选仲裁耗时

这样能满足可观察和调试，不打断按键说话的主流程。

## 延迟预算

建议预算：

* TNL 本地候选召回：目标 `< 10ms`，沿用现有性能测试风格。
* 候选数量上限：默认最多 5 个进入 LLM。
* LLM 仲裁超时：建议 500-800ms。
* 无候选：绝不调用 LLM。
* 只有高置信本地替换：绝不调用 LLM。
* 超时/失败：保留本地高置信结果，中置信候选不替换。

## MVP 范围建议

推荐 MVP 不做大型词库 schema 迁移，而是：

1. 复用现有词库 `Vec<String>`。
2. 在 TNL 内从词条自动派生英文音近、多 token、连字符、中文拼音候选。
3. 增加 `TnlDiagnostics` 前后端可选字段。
4. 新增候选仲裁轻量 LLM 调用，和全文润色分离。
5. 历史记录展示候选摘要。

## 风险

* 改 `normalize` 风险高，必须保留现有回归测试，并先加候选系统测试。
* 字节索引和字符索引混用已有历史，新增候选 span 必须明确使用 UTF-8 byte offset 或 char offset，并保持前后端展示不依赖直接切片。
* LLM 仲裁如果放在插入前，会影响延迟；因此必须有超时和开关。
* 候选和全文润色同时开启时，`original_text` 的语义要清楚：历史原文应能表达 ASR 原文、TNL 后文本、最终文本之间的关系。
