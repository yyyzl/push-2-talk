# Phase 6 NER-aware Tech Spans

## Goal

完成 Phase 6 的最小闭环：在现有 `TechSpanDetector` 已经使用 jieba 注入用户词的基础上，进一步利用 jieba 词性标签，把内置词典识别出的 `nr/ns/nt/nz` 专名也标记为 `NamedEntity` span。这样用户词以外的常见中文人名、地名、机构名和专名也能进入 TNL 的保护层，减少后续音化/符号处理误切专名的概率。

## What I Already Know

- `src-tauri/src/tnl/tech_span.rs` 已经持有 `jieba_rs::Jieba`，并通过 `add_word(..., Some("nz"))` 注入用户词。
- 当前 `detect_user_named_entities` 使用 `jieba.cut(text, false)`，只会把 `self.user_terms` 命中的词标成 `NamedEntity`。
- `jieba-rs 0.9.0` 提供 `Jieba::tag(sentence, hmm) -> Vec<Tag { word, tag }>`，可读取默认词典或用户词的词性标签。
- `Tag` 不带 offset，但当前实现已经用 cursor + `text[cursor..].find(word)` 计算字节范围，可复用。
- `SpanType::NamedEntity` 优先级低于 URL/email/path/file/identifier 等技术 span，已有 merge 逻辑会让强技术 span 胜出。
- `TnlEngine::new_with_disfluency_mode` 已经把 category 路由后的 `named_entity_words` 传入 `TechSpanDetector::new_with_user_dictionary`。
- `src-tauri/Cargo.toml` 当前工作区有既有 dirty version bump，不能把它混进本任务提交；所需 `jieba-rs` 依赖本身已存在于当前文件内容。

## Requirements

- `TechSpanDetector` 的专名检测应从 `jieba.cut` 升级为 `jieba.tag`。
- 继续保留用户词命中行为：注入的用户词即使默认词典不认识，也应作为 `NamedEntity`。
- 新增内置 POS 专名识别：`nr`、`ns`、`nt`、`nz` 标签可形成 `NamedEntity`。
- 专名 span 仍需满足现有保守过滤：短词、纯空白/纯标点不进入。
- `Email` / `Url` / `Path` / `Identifier` 等强技术 span 优先级不能被 `NamedEntity` 覆盖。
- 不引入 ONNX NER 或新模型，不修改前端 UI。

## Acceptance Criteria

- [ ] 不提供用户词时，jieba 默认词典中的地名/机构名/专名可产生 `NamedEntity` span。
- [ ] 用户词注入行为仍通过原有测试，metadata 纯化路径不退化。
- [ ] `Email` 与 `NamedEntity` 重叠时仍保留 `Email`。
- [ ] `TnlEngine` 的 `technical_spans` 能暴露新的 POS 专名 span。
- [ ] `cargo test tech_span --lib` 通过。
- [ ] `cargo test tnl::engine --lib` 或相关 TNL 子集通过。
- [ ] `cargo check` 通过。

## Definition Of Done

- 代码和回归测试落地。
- 如行为契约有变化，更新 `.trellis/spec/backend/tnl-normalization.md` 和 `ASR_PERSONALIZATION_QUALITY_LEAP.md`。
- 工作提交、Trellis task 归档、session journal 记录。

## Technical Approach

- 在 `TechSpanDetector` 中新增/调整专名检测逻辑：
  - 使用 `self.jieba.tag(text, false)` 获取分词和 POS 标签，避免 HMM 带来的 hot path 延迟回归。
  - 复用 cursor-find 计算字节 offset。
  - 命中 `self.user_terms` 或 POS 属于 `nr/ns/nt/nz` 时输出 `SpanType::NamedEntity`。
  - 用统一 helper 判断候选词和标签，避免重复条件。
- 保持 `merge_overlapping` 和 span priority 不变。
- 先写/调整 Rust 单元测试，再改实现。

## Decision (ADR-lite)

**Context**: Phase 6 目标是用 jieba + 用户词 + 启发式覆盖 80% 专名保护场景；当前实现只使用了用户词注入，还没消费默认词典 POS 标签。

**Decision**: 采用 `Jieba::tag` 的轻量 POS 路径，不引入额外模型或异步服务。

**Consequences**: 这是低成本本地能力，但 jieba POS 标签不是强 NER 模型；因此只把结果作为低优先级 `NamedEntity` span，不让它覆盖 email/url/path 等强技术 span。

## Out Of Scope

- 引入 ONNX NER、本地 reranker 或 LLM 判断。
- 改 `SyllableMatchPass` 权重逻辑。
- 改前端词库 UI。
- 删除或重构 `TechSpanDetector` 现有规则。
- 处理 `src-tauri/Cargo.toml` 既有 version dirty。

## Technical Notes

- 主要文件：`src-tauri/src/tnl/tech_span.rs`、`src-tauri/src/tnl/engine.rs`。
- 相关 spec：`.trellis/spec/backend/tnl-normalization.md`、`.trellis/spec/backend/asr-hotword-compilation.md`、`.trellis/spec/backend/database-guidelines.md`。
- Research: `research/jieba-rs-pos-tagging.md`。
