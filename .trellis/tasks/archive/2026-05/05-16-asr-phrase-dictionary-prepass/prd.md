# Phase 5 phrase dictionary prepass

## Goal

把 `phrase` category 词条接入 TNL 的本地短语优先匹配路径，让用户手动标注的固定短语可以先于 phonetic/fuzzy 候选被规范化，补上 Phase 5 中“短语独立 lookup 路径”的后端切片。

## What I already know

- 前一个任务已让 TNL 按 category 路由词库：`email/url` 跳过字典改写路径，`code_symbol` 跳过 phonetic/fuzzy。
- 当前 `phrase` category 仍沿用普通 named-entity / fuzzy / hyphen 路径，没有独立的 phrase prepass。
- `Tokenizer` 只按字符类型切分，不做中文分词；中文短语被 ASR 插入空格时，现有路径很难把 `团队 约定` 收回 `团队约定`。
- 个性化 correction pair 已有 phrase/window 逻辑，但它依赖已学习的 correction pair；本任务只处理用户词库 `phrase` category 的本地词典路径。

## Requirements

- 仅 `category=phrase` 的词条进入 phrase prepass。
- phrase prepass 在 pinyin / hyphen / phonetic 字典改写之前运行。
- 对空格分隔短语做大小写不敏感匹配并规范化为词库原始大小写，例如 `claude code -> Claude Code`。
- 对无空格中文短语允许 ASR 在字之间插入空白并收回，例如 `团队 约定 -> 团队约定`。
- 不跨句号、逗号等标点匹配短语。
- 非 `phrase` category 的产品名/术语不因为本任务额外触发大小写规范化。
- 不改变 ASR hotword payload、frontend category UI、correction pair schema。

## Acceptance Criteria

- [x] 新增后端测试：`phrase` ASCII 短语可做大小写规范化。
- [x] 新增后端测试：`phrase` 中文短语可吞掉 ASR 插入的空白。
- [x] 新增后端测试：短语 prepass 不跨标点匹配。
- [x] 新增后端测试：非 `phrase` category 不走该 prepass。
- [x] 相关 TNL category/phrase 测试通过。
- [x] `cargo check` 通过。
- [x] 路线图和 `.trellis/spec/` 同步记录 phrase prepass 的已落地范围。

## Out of Scope

- 不实现 SQLite `user_terms` 表。
- 不实现完整持久化 trie 索引或数据库索引列。
- 不改 learning 阶段的 LLM category 判断。
- 不改变 personalization correction pair 的匹配算法。

## Technical Notes

- 预期影响文件：
  - `src-tauri/src/tnl/engine.rs`
  - `.trellis/spec/backend/tnl-normalization.md`
  - `ASR_PERSONALIZATION_QUALITY_LEAP.md`
- 候选实现方向：
  - 在 `RoutedDictionary` 中增加 `phrase_words`。
  - 构建轻量 `PhraseDictionaryRule`，对 ASCII 空格短语按 segment 匹配，对中文无空格短语按字符匹配并允许字间空白。
  - 复用 `ReplacementReason::DictionaryExact`，让诊断面保持兼容。
