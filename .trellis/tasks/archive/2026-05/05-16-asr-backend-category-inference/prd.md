# Phase 5 backend dictionary category inference

## Goal

让后端 `add_learned_word` 在调用方没有传 category、或传入旧/无效 category 时，仍能按 Phase 5 规则推断并持久化词库 metadata，避免旧前端、脚本、自动化调用写入的词条退化为无分类 generic。

## What I already know

- 前端 `inferDictionaryCategory` 已覆盖 email/url/code_symbol/中文 phrase/generic。
- 后端 `dictionary_utils` 目前只标准化已有 category，不做推断。
- `add_learned_word` 直接把 `category.as_deref()` 传给 `upsert_entry_with_category`；缺省时会保存成旧 compact generic。
- Phase 5 的 TNL category 路由已经依赖 metadata：`email/url/code_symbol/phrase` 的行为差异明显。
- SQLite `user_terms` 仍是稳定后迁移目标，本任务只补 JSON/config 后端兜底。

## Requirements

- 在 Rust `dictionary_utils` 中新增与前端一致的 category 推断 helper。
- 推断规则：
  - email：非空白邮箱形态，包含 `@` 和点号。
  - url：`http://` / `https://` / `www.` 开头。
  - code_symbol：全 ASCII 且驼峰、含 `_`、含 `/` 或 `\`、或字母数字间含 `-`。
  - phrase：至少两个 CJK 字符且不含空白。
  - 其他：`generic`。
- `add_learned_word` 在 category 缺省或无效时使用推断 category。
- 旧 category alias 继续标准化：`proper_noun -> product`、`term -> domain_term`、`frequent -> generic`。
- generic 仍保持 compact 旧格式。
- 不改变 ASR provider payload、TNL 纯词输入、前端 UI。

## Acceptance Criteria

- [x] Rust 单元测试覆盖 backend category 推断规则。
- [x] Rust 单元测试覆盖 category 缺省时 upsert 使用推断结果。
- [x] Rust 单元测试覆盖无效 category 降级为推断结果。
- [x] `add_learned_word` 使用同一 helper 推断 category。
- [x] 相关 dictionary/backend tests 通过。
- [x] `cargo check` 通过。
- [x] `.trellis/spec/` 和路线图同步记录 backend 推断兜底。

## Out of Scope

- 不引入 SQLite。
- 不批量迁移已有 compact 词条。
- 不改前端推断规则。
- 不改 correction pair schema。

## Technical Notes

- 预期影响文件：
  - `src-tauri/src/dictionary_utils.rs`
  - `src-tauri/src/lib.rs`
  - `.trellis/spec/backend/tnl-normalization.md`
  - `ASR_PERSONALIZATION_QUALITY_LEAP.md`
- 候选实现方向：
  - 暴露 `infer_dictionary_category(word) -> &'static str`。
  - 暴露 `normalize_or_infer_category(word, category) -> &'static str`。
  - `upsert_entry_with_category` 可使用这个 helper，对已有有效 category 保持原行为，对 None/invalid 走推断。
