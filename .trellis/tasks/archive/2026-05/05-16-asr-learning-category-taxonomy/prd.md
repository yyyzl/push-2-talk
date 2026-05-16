# Phase 5 learning category taxonomy

## Goal

让自动词库学习阶段产出的 `category` 对齐现有用户词典分类体系，而不是只停留在 `proper_noun / term / frequent` 三个旧分类。这样学习建议被接受后，后端 TNL category 路由、phrase prepass、code/email/url 保护都能吃到更精确的 metadata。

## What I already know

- 当前词库已经支持 `person / product / tool / phrase / email / url / code_symbol / domain_term / generic`。
- `dictionary_utils` 已兼容旧分类映射：`proper_noun -> product`、`term -> domain_term`、`frequent -> generic`。
- `learning::llm_judge` 的 prompt 和 sanitize 仍只允许 `proper_noun / term / frequent`。
- 前端 `VocabularyLearningSuggestion` 类型和 Toast 标签也只写了三类旧标签。
- `add_learned_word` 已经把 `category` 传给后端，接受新分类不需要改命令 shape。

## Requirements

- LLM 学习判断 prompt 改为要求输出词库完整分类。
- 后端 sanitize 允许完整词库分类，并继续接受旧分类别名。
- 旧分类别名应被规范化为词库分类后继续向前端发出：
  - `proper_noun -> product`
  - `term -> domain_term`
  - `frequent -> generic`
- invalid category 仍应保守降级到 `domain_term`，避免中断学习流程。
- 前端学习建议类型和 Toast 标签支持完整词库分类，同时保留旧分类标签兜底。
- 不改变 `add_learned_word` 的 Tauri 命令参数结构。
- 不改变 correction pair JSON schema；这里只改建议分类 metadata。

## Acceptance Criteria

- [x] 后端测试覆盖新分类 `code_symbol` 可通过 sanitize。
- [x] 后端测试覆盖旧分类 `term` 规范化到 `domain_term`。
- [x] 后端测试覆盖 invalid category 降级到 `domain_term`。
- [x] 前端 runtime/source 测试覆盖 Toast 显示完整分类标签。
- [x] `npm run test:ts` 通过。
- [x] 相关 Rust learning 测试通过。
- [x] `cargo check` 通过。
- [x] `.trellis/spec/` 与路线图同步更新 learning category taxonomy。

## Out of Scope

- 不改学习观察触发时机。
- 不引入 SQLite。
- 不改 correction pair category 历史数据。
- 不做批量重算已有词典分类。

## Technical Notes

- 预期影响文件：
  - `src-tauri/src/learning/llm_judge.rs`
  - `src-tauri/src/dictionary_utils.rs`
  - `src/types/index.ts`
  - `src/types/learning.ts`
  - `src/components/learning/VocabularyLearningToast.tsx`
  - `tests/personalizationLearningContext.test.ts`
  - `.trellis/spec/backend/event-contracts.md`
  - `.trellis/spec/backend/tnl-normalization.md`
  - `ASR_PERSONALIZATION_QUALITY_LEAP.md`
- 候选实现方向：
  - 在 `dictionary_utils` 暴露 category 标准化函数，避免 learning judge 复制分类映射。
  - `llm_judge::sanitize_result` 统一写出 canonical dictionary category。
  - TS 类型使用 `DictionaryCategory | legacy aliases`，以兼容旧事件。
