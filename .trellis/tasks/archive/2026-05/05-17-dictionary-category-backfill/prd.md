# Phase 5 dictionary category backfill

## Goal

让已有 `AppConfig.dictionary` 里的旧 compact 词条在后端加载/保存时自动补齐可推断的 category metadata，使用户过去积累的邮箱、URL、代码符号、中文短语也能进入 Phase 5 的 category-aware TNL 路由，而不需要等用户手动编辑每个词条。

## What I already know

- Phase 5 已完成 JSON 词库 metadata、DictionaryPage 分类编辑、TNL category 路由、phrase prepass、学习 LLM 完整 taxonomy、`add_learned_word` 后端分类推断兜底。
- 当前剩余 Phase 5 小闭环之一是“已有词典批量 category 重算”。
- `AppConfig::load()` 已有迁移框架，返回 `(config, migrated)`；`load_persisted_config()` 会在 `migrated=true` 时保存迁移后的配置。
- `save_config` 当前会直接使用前端传入的 `dictionary` 数组；老客户端或脚本仍可能传入 `word` / `word|auto` compact 格式。
- `dictionary_utils` 已有 `infer_dictionary_category`、`normalize_or_infer_category`、`format_entry_with_category`、`extract_word`、`extract_category` 等可复用 helper。

## Requirements

- 新增后端批量 backfill helper，对 `Vec<String>` 词库条目逐条规范化。
- 对缺少 category 或 category 无效的条目，使用与前端一致的推断规则补 metadata。
- 推断结果为 `generic` 时继续保持旧 compact 格式：`word` 或 `word|auto`。
- 推断结果为非 `generic` 时写成 `word|source|category`。
- 已有有效 category 必须保留；旧 alias 可标准化为 canonical category：`proper_noun -> product`、`term -> domain_term`、`frequent -> generic`。
- source 继续按旧约定解析：`auto` 保持 auto，其它值视作 manual。
- `AppConfig::load()` 应在迁移阶段运行 backfill，发生变化时设置 `migrated=true`，由现有保存路径落盘。
- `save_config` 对传入或保留的 dictionary 也应执行同一 backfill，避免旧调用覆盖掉 metadata。
- ASR、TNL、LLM hotword 消费者仍必须通过 `entries_to_words` 或现有纯词边界消费纯词，不把 metadata 发给 provider。

## Acceptance Criteria

- [x] Rust 单元测试覆盖批量 backfill：compact `useState|auto` 变成 `useState|auto|code_symbol`。
- [x] Rust 单元测试覆盖中文短语 backfill：`团队约定` 变成 `团队约定|manual|phrase`。
- [x] Rust 单元测试覆盖 generic compact 保持不膨胀：`rust|auto` 仍为 `rust|auto`。
- [x] Rust 单元测试覆盖已有有效 category 保留。
- [x] Rust 单元测试覆盖旧 alias canonical 化。
- [x] `AppConfig::load()` 迁移路径在词典发生 backfill 时返回 `migrated=true`。
- [x] `save_config` 写入前复用同一 backfill helper。
- [x] 相关 dictionary/config/backend tests 通过。
- [x] `cargo check` 通过。
- [x] `.trellis/spec/` 和路线图同步记录已有词典 backfill 完成。

## Definition of Done

- Tests added/updated.
- Targeted Rust tests pass.
- `cargo check` passes.
- Docs/spec updated for the new migration/backfill behavior.
- Dirty unrelated files remain excluded from commit.

## Technical Approach

- 在 `dictionary_utils.rs` 增加 `backfill_inferred_categories(entries: &mut Vec<String>) -> bool`，返回是否发生变化。
- helper 只重写 storage string，不改变运行时纯词提取逻辑。
- 在 `AppConfig::load()` 的迁移逻辑末尾调用该 helper。
- 在 `save_config` 计算 `final_dictionary` 后调用同一 helper，确保 IPC 写入也走后端兜底。

## Decision (ADR-lite)

**Context**: 现有用户词典仍是 `Vec<String>`，Phase 5 category metadata 已能提升 TNL 路由质量，但旧 compact 词条不会自动享受这些保护。

**Decision**: 先做后端配置级 backfill，不引入 SQLite，不批量跑 LLM。使用确定性规则补可推断 category，并保持 `generic` compact 兼容格式。

**Consequences**: 老词典第一次加载后会自动获得非 generic metadata，TNL 保护更快生效；对无法可靠推断的 `generic` 词条不膨胀，也不引入误分类成本。更细的人名/产品/工具判别仍留给后续 LLM/SQLite 迁移。

## Out of Scope

- 不引入 SQLite `user_terms`。
- 不新增 `en_phonetic_key` / `zh_pinyin_fuzzy_key` 索引列。
- 不引入 LLM 批量分类。
- 不做持久化 phrase trie/index。
- 不改前端 UI。
- 不改 ASR provider payload。

## Technical Notes

- 预期影响文件：
  - `src-tauri/src/dictionary_utils.rs`
  - `src-tauri/src/config.rs`
  - `src-tauri/src/lib.rs`
  - `.trellis/spec/backend/tnl-normalization.md`
  - `ASR_PERSONALIZATION_QUALITY_LEAP.md`
- 相关契约：
  - `.trellis/spec/backend/tnl-normalization.md` → “User Dictionary Category Metadata Uses Backward-Compatible Storage”
- 现有无关脏文件继续排除：
  - `package.json`
  - `src-tauri/Cargo.toml`
  - `src-tauri/tauri.conf.json`
  - `.trellis/.runtime/`
