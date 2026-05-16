# Phase 5 category-aware TNL lookup

## Goal

把用户词库的 `category` metadata 真正接入 TNL 本地二次解码路径，避免邮箱、链接、代码符号这类“应保护/精确匹配”的词条进入音化模糊改写，同时保留产品名、工具名、术语等对 ASR 错音的修正能力。

## What I already know

- `ASR_PERSONALIZATION_QUALITY_LEAP.md` 的 Phase 5 要求不同 category 走不同 lookup 路径。
- 前端和配置层已经支持 `word|source|category`，`DictionaryPage` 也已经有 category 编辑入口。
- ASR hotword pack 会读取 metadata 并按 source/category 保留排序信息，但 provider payload 只发送 pure word。
- 当前 `TnlEngine::new_with_disfluency_mode` 会先调用 `purify_dictionary_entries`，只保留 pure word，导致 category 在 `TechSpanDetector`、hyphen rewrite、`FuzzyMatcher` 之前丢失。
- `.trellis/spec/backend/tnl-normalization.md` 已要求 metadata 不能泄漏到 TNL，但 Phase 5 的 category-aware 路由还没有完整落地。

## Requirements

- `email` / `url` 词条不得进入 TNL 的 fuzzy / phonetic / hyphen rewrite / named-entity 用户词注入路径。
- `code_symbol` 词条不得进入 fuzzy / phonetic 路径，避免把普通英语误改成代码符号。
- `code_symbol` 可以保留现有精确/分隔符重写能力，用于 `GPT 5.3 Codex -> GPT-5.3-Codex` 这类低风险规范化。
- `person` / `product` / `tool` / `phrase` / `domain_term` / `generic` / legacy 无 category 词条保持现有 TNL 行为。
- correction pair 产生的 TNL 词库仍按现有 pure word 方式进入 TNL，避免本任务扩大到 correction-pair schema。
- 现有 ASR provider hotword payload、前端词库 UI、学习存储格式不被改动。

## Acceptance Criteria

- [x] 新增后端回归测试：`code_symbol` 不参与 phonetic/fuzzy 替换。
- [x] 新增后端回归测试：`product` 等可音化 category 仍能触发既有 phonetic/fuzzy 修正。
- [x] 新增后端回归测试：`email` / `url` category 不会被注入为 named entity 或进入 fuzzy/hyphen。
- [x] 相关 TNL / dictionary 工具测试通过。
- [x] `cargo check` 通过。
- [x] Phase 5 路线图和 `.trellis/spec/` 同步说明本次落地范围。

## Out of Scope

- 不新建 SQLite `user_terms` 表。
- 不做 phrase trie 的全量重写。
- 不调整前端 category UI。
- 不改 ASR provider hotword 协议。
- 不改变 correction pair 文件格式。

## Technical Notes

- 预期影响文件：
  - `src-tauri/src/dictionary_utils.rs`
  - `src-tauri/src/tnl/engine.rs`
  - `.trellis/spec/backend/tnl-normalization.md`
  - `ASR_PERSONALIZATION_QUALITY_LEAP.md`
- 候选实现方向：
  - 在 backend 暴露 category 解析辅助函数。
  - TNL 构造阶段把 dictionary entries 分成 technical-span / fuzzy / hyphen 三个纯词列表。
  - 对 `email` / `url` 返回空路由；对 `code_symbol` 只进入 exact/technical-safe 路径；其余 category 沿用现有路径。
