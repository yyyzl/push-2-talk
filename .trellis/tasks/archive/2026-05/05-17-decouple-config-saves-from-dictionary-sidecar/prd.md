# Decouple Config Saves From Dictionary Sidecar Snapshots

## Goal

让普通配置保存路径不再把 `AppConfig.dictionary` 当作用户词库的真实写入源，避免 sidecar-first 迁移后被旧配置快照反向水合覆盖。词库真实写入应由 `get_dictionary_entries` / `add_learned_word` / `delete_dictionary_entries` 和 `user_terms.db` 负责；`AppConfig.dictionary` 只保留兼容快照和显式迁移入口。

## What I Already Know

- `user_terms.db` 已经是词库管理命令的主要持久化源。
- `start_app` 已经能合并 enabled sidecar 用户词和运行时动态热词。
- `save_config` 当前每次普通配置保存都会持久化 `dictionary: resolved.storageDictionary`。
- 后端 `save_persisted_config_without_emit` 当前每次保存配置后都会调用 `sync_user_terms_sidecar_from_dictionary_or_warn(&config.dictionary, "配置保存")`。
- 这意味着即使词库命令已 sidecar-first，普通设置保存仍可能把旧 `AppConfig.dictionary` 快照重新水合进 sidecar。
- 前端 `loadConfig` 当前从 `config.dictionary` 初始化 dictionary state；现在可以改为调用 `get_dictionary_entries`，该命令已经 sidecar-first，并有旧 config bootstrap fallback。

## Assumptions

- `AppConfig.dictionary` 字段本任务不删除，仍作为兼容快照存在。
- localStorage 迁移或其它明确提供 `dictionary` / `storageDictionary` 的路径仍可同步 sidecar。
- 普通设置保存、ASR fallback 配置修复、主题/模型/助手配置保存不应默认触碰 sidecar。
- `get_dictionary_entries` 是前端初始化用户词库 state 的新来源。

## Requirements

- 前端 `loadConfig`：
  - 在后端 `load_config` 后调用 `get_dictionary_entries` 读取用户词条。
  - 用 `get_dictionary_entries` 的结果初始化 dictionary state 和启动 runtime dictionary。
  - 保持旧 `config.dictionary` 作为读取失败时的兜底。
- 前端 `saveConfigThroughGateway`：
  - 默认不向 `save_config` payload 传 `dictionary` 字段。
  - 只有 `overrides.dictionaryEntries` 或 `overrides.storageDictionary` 显式出现时，才传 `dictionary: resolved.storageDictionary`。
  - runtime dictionary 构建仍使用当前 dictionary state + recent + builtin，不改变动态热词 runtime-only 语义。
- 后端配置保存：
  - `save_persisted_config_without_emit` 只保存配置文件，不默认同步 `user_terms` sidecar。
  - `save_config` 只有收到显式 `dictionary` payload 时才 warning-only 同步 sidecar。
  - `load_persisted_config` 仍可在配置加载时从 `AppConfig.dictionary` bootstrap sidecar，以保护老配置。
- 测试更新：
  - TS runtime test 覆盖普通 `save_config` payload 不含 `dictionary`。
  - TS runtime test 覆盖显式 dictionary override 仍会传 `dictionary`。
  - Rust/source test 覆盖 `save_persisted_config_without_emit` 不再默认同步 sidecar，`save_config` 只在 dictionary payload 存在时同步。

## Acceptance Criteria

- [ ] 普通 `saveConfigThroughGateway()` 生成的 `save_config` payload 不包含 `dictionary`。
- [ ] `saveConfigThroughGateway({ dictionaryEntries })` 或 `{ storageDictionary }` 仍包含 `dictionary`。
- [ ] `loadConfig` 的 dictionary state 来源改为 `get_dictionary_entries`，失败才回退 `config.dictionary`。
- [ ] 后端普通 `mutate_persisted_config` 保存不再触发配置快照到 sidecar 的同步。
- [ ] `save_config` 显式收到 dictionary 时仍同步 sidecar，兼容迁移/导入路径。
- [ ] `npm run test:ts`、`cargo test --lib`、`cargo check` 通过。

## Definition Of Done

- 代码和测试落地。
- `ASR_PERSONALIZATION_QUALITY_LEAP.md` 和 backend database spec 更新。
- 工作提交、Trellis task 归档、session journal 记录。

## Out Of Scope

- 删除 `AppConfig.dictionary` 字段。
- 删除所有前端 dictionary state。
- 改造 DictionaryPage UI。
- 持久化 phrase trie/索引结构。
- Phase 6/7 功能。

## Technical Notes

- 主要前端文件：`src/hooks/useAppServiceController.ts`。
- 主要后端文件：`src-tauri/src/lib.rs`。
- 相关测试：`tests/recentHotwordRuntimeFlow.test.ts`、`tests/defaultDoubaoImeFallback.test.ts`、`tests/configSaveReviewFixes.test.ts`、可新增 sidecar config save flow 测试。
- 相关 specs：`.trellis/spec/backend/database-guidelines.md`、`.trellis/spec/backend/event-contracts.md`、`.trellis/spec/backend/asr-hotword-compilation.md`、`.trellis/spec/frontend/hook-guidelines.md`、`.trellis/spec/frontend/state-management.md`。
