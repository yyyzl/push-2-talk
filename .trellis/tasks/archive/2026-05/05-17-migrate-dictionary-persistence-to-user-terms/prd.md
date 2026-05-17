# Migrate Dictionary Persistence To User Terms Sidecar

## Goal

把用户词管理入口的主要持久化源从 `AppConfig.dictionary` 迁到 SQLite `user_terms.db` sidecar，让“词库页新增/编辑/删除”和“自动学习接受词条”直接更新带 category/source/phonetic index 的用户词表。`AppConfig.dictionary` 在本任务中继续作为兼容快照保留，避免一次性破坏旧配置、前端加载和启动参数。

## What I Already Know

- Phase 5 已完成 `user_terms` sidecar schema、水合、phonetic key 查询、运行时词库读取与 `start_app` merge。
- `get_dictionary_entries` 当前读取 `AppConfig.dictionary`。
- `add_learned_word` 当前通过 `upsert_entry_with_inferred_category` 更新 `AppConfig.dictionary`，再把纯词更新到运行时 state/ASR client。
- `delete_dictionary_entries` 当前通过 `remove_entries` 删除 `AppConfig.dictionary`，再热更新运行时 state/ASR client。
- 前端 `useDictionary` 已通过 Tauri 命令管理词库；它不直接写配置文件。
- 前端 `useAppServiceController` 仍从 `load_config.dictionary` 初始化 dictionary state，并在保存配置时把 dictionary 快照写回 `save_config`。
- `save_config` / `load_persisted_config` 已 warning-only 水合 sidecar，因此兼容快照仍能保持 sidecar 可恢复。

## Assumptions

- 本任务不移除 `AppConfig.dictionary` 字段，只把词库命令优先读写 `user_terms.db`。
- `AppConfig.dictionary` 会在词库命令成功后同步成 enabled sidecar entries，继续服务旧路径和前端启动初始化。
- sidecar 打不开时，词库命令应该返回错误，而不是静默写旧配置；否则会产生两个真实持久化源。
- 动态 runtime-only entries（`domain` / `recent` / `builtin` / `app_context`）不进入 sidecar，也不通过词库命令写入配置快照。

## Requirements

- 为 `UserTermStore` 增加命令级 API：
  - upsert 单个词条，保留 manual 优先级、规范化 category，并刷新 phonetic index。
  - disable/delete 一组词条，按 term 大小写不敏感匹配。
  - list enabled entries 继续输出 `word|source|category` storage-compatible 字符串。
- `add_learned_word`：
  - 继续保存 correction pair。
  - 直接 upsert 默认 sidecar。
  - 将 enabled sidecar entries 同步回 `AppConfig.dictionary` 兼容快照。
  - 热更新运行时 state/ASR client 时使用 enabled sidecar entries 的 metadata 字符串；ASR provider 编译层继续提纯为 provider payload，TNL 继续使用 category/source 路由。
  - 继续发送 `config_updated` 和 `dictionary_updated`。
- `delete_dictionary_entries`：
  - 直接 disable 默认 sidecar 中的词。
  - 将 enabled sidecar entries 同步回 `AppConfig.dictionary` 兼容快照。
  - 用 enabled sidecar entries 热更新运行时 state/ASR client。
  - 继续发送 `config_updated` 和 `dictionary_updated`。
- `get_dictionary_entries`：
  - 优先读取 enabled sidecar entries。
  - 如果 sidecar 为空但 config 有旧词典，先水合 sidecar，再返回 enabled entries。
  - sidecar 读取失败时 warning-only 回退 `AppConfig.dictionary`，用于保护老用户启动。
- 保持现有前端 `useDictionary` 调用方式不变。

## Acceptance Criteria

- [ ] `add_learned_word` 写入 sidecar，并同步 `AppConfig.dictionary` 兼容快照。
- [ ] `delete_dictionary_entries` disable sidecar entries，并同步 `AppConfig.dictionary` 兼容快照。
- [ ] `get_dictionary_entries` 可从 sidecar 返回 enabled entries，并在 sidecar 空时从旧 config 水合。
- [ ] 词库命令继续触发 `dictionary_updated` / `config_updated`。
- [ ] 动态 runtime-only entries 不被写入 sidecar。
- [ ] Rust 单测覆盖 upsert/delete/list 与命令 helper fallback。
- [ ] 相关 TypeScript runtime tests 更新后通过。
- [ ] `cargo check` 通过。

## Definition Of Done

- 后端命令持久化源完成 sidecar-first。
- 兼容 `AppConfig.dictionary` 快照仍可被旧路径读取。
- 路线图和必要 spec 更新。
- 工作提交后再归档任务、写 session journal。

## Out Of Scope

- 删除 `AppConfig.dictionary`。
- 改造前端 `useAppServiceController` 的配置初始化来源。
- 把 builtin/recent/app-context 动态热词持久化到 sidecar。
- 持久化 phrase trie/索引结构。
- Phase 6 分词/NER 或 Phase 7 HotwordCompiler 升级。

## Technical Notes

- 主要后端文件：`src-tauri/src/lib.rs`、`src-tauri/src/personalization/user_terms_store.rs`、`src-tauri/src/dictionary_utils.rs`。
- 主要前端接入点：`src/hooks/useDictionary.ts`、`src/hooks/useAppServiceController.ts`。
- 相关测试：`tests/dictionaryCategoryMetadata.test.ts`、`tests/recentHotwordRuntimeFlow.test.ts`、Rust `user_terms_store` 与词库命令 helper tests。
- 相关 specs：`.trellis/spec/backend/database-guidelines.md`、`.trellis/spec/backend/event-contracts.md`、`.trellis/spec/backend/asr-hotword-compilation.md`、`.trellis/spec/backend/tnl-normalization.md`、`.trellis/spec/frontend/hook-guidelines.md`、`.trellis/spec/frontend/state-management.md`。
