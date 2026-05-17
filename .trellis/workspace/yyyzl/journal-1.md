# Journal - yyyzl (Part 1)

> AI development session journal
> Started: 2026-02-15

---



## Session 1: feat: AI 助手异步结果面板 — 完整实现 + 运行时调试

**Date**: 2026-04-11
**Task**: feat: AI 助手异步结果面板 — 完整实现 + 运行时调试
**Branch**: `feat/assistant-async-result-panel`

### Summary

(Add summary)

### Main Changes


## 完成内容

| 模块 | 变更 |
|------|------|
| **ResultPanelWindow** | 新增 Markdown 渲染浮窗（react-markdown + remark-gfm + react-syntax-highlighter） |
| **Push + Poll 双模式** | 解决隐藏 WebView 丢失 push 事件问题，300ms 自停轮询兜底 |
| **透明窗口拖动** | `data-tauri-drag-region` 在 WebView2 不工作，改用 `startDragging()` API |
| **Capabilities 权限** | `result_panel` 加入 `capabilities/default.json`，修复静默 IPC 全面失败 |
| **剪贴板即时释放** | ClipboardGuard 捕获后立即 drop，不再长期持有 |
| **Pipeline 改造** | AssistantPipeline 不再自动插入，返回结果由结果面板展示 |
| **Pending 生命周期** | 覆盖/丢弃/停止均补发 transcription_complete 事件 |
| **砍掉粘贴功能** | 用户反馈无意义，仅保留复制 + 关闭 |
| **Spec 更新** | 3 个 spec 文件记录 6 个 CRITICAL 教训 |

## 踩坑记录

1. **Capabilities `windows` 数组遗漏** — 新窗口未加入导致 ALL IPC 静默失败，无任何报错
2. **隐藏 WebView 不处理事件** — `listen()`/`emit()` 在 hidden 窗口不可靠，需 `invoke()` 轮询兜底
3. **`data-tauri-drag-region` 透明窗口失效** — Windows WebView2 下必须用 `startDragging()` API
4. **`invoke("get_config")` vs `load_config`** — Tauri IPC 命令名不匹配静默失败
5. **react-markdown v9 不支持 `className` prop** — 需外层 div 包裹

## 变更文件 (16 files, +2899/-125)

- `result-panel.html` (新增)
- `src/components/MarkdownRenderer.tsx` (新增)
- `src/types/assistant-result.ts` (新增)
- `src/windows/ResultPanelWindow.tsx` (新增)
- `src/windows/result-panel-actions.ts` (新增)
- `src/windows/result-panel-main.tsx` (新增)
- `tests/assistantResultPanel.test.ts` (新增, 16 个测试)
- `src-tauri/src/lib.rs` (核心改造)
- `src-tauri/src/pipeline/assistant.rs` (移除自动插入)
- `src-tauri/src/clipboard_manager.rs` (新增 copy_to_clipboard)
- `src-tauri/tauri.conf.json` (新增 result_panel 窗口)
- `src-tauri/capabilities/default.json` (权限声明)
- `vite.config.ts` (多页构建入口)
- `package.json` (新增 3 个 npm 依赖)


### Git Commits

| Hash | Message |
|------|---------|
| `5f2bdee` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: AI 助手多轮对话与追问功能

**Date**: 2026-04-11
**Task**: AI 助手多轮对话与追问功能
**Branch**: `feat/assistant-async-result-panel`

### Summary

(Add summary)

### Main Changes

## 完成内容

将 AI 助手模式从单轮无状态交互升级为多轮对话，支持面板内连续追问。

| 模块 | 变更 |
|------|------|
| 后端数据结构 | `ConversationSession` 替代 `PendingAssistantResult`，含 `PromptMode` 首轮锁定、20轮滑动窗口 |
| 后端 Pipeline | `handle_assistant_mode()` 分支新对话/追问路径，追问复用 `process_followup()` |
| IPC 命令 | 5 个新命令替代旧命令：`get_conversation_state` / `dismiss_conversation` / `copy_latest_reply` / `copy_full_conversation` / `paste_latest_reply` |
| IPC 事件 | 3 个新事件：`assistant_turn_complete` / `assistant_turn_pending` / `assistant_turn_error` |
| 前端类型 | `ConversationTurn` / `TurnCompletePayload` / `TurnPendingPayload` / `TurnErrorPayload` / `formatConversationForCopy()` |
| 前端 UI | `ResultPanelWindow` 重构为对话流视图，含智能滚动、浮标回底、loading/error 气泡 |

## 修复的 Bug

| Bug | 根因 | 修复 |
|-----|------|------|
| 一次追问产生两个重复回复 (后端) | `is_assistant_processing` 用 `store(true)` 存在竞态窗口，rdev 热键双触发穿透 guard | 改为 `compare_exchange` 原子 CAS |
| 一次追问产生两个重复回复 (前端) | React 18 StrictMode 双挂载导致异步 `listen()` 注册两个 listener，累积型 state 更新被执行两次 | 使用 `cancelled` flag + deferred unsubscribe 模式 |

## 新增测试

- 5 个 Rust 单元测试：`build_followup_messages` 基本/带文本/滑动窗口/文本处理模式 + `format_conversation_for_copy`
- 7 个 TS 测试：`getKeyboardAction` (5个) + `formatConversationForCopy` (2个)

## 写入 Spec 的经验

- `frontend/component-guidelines.md`：React StrictMode + 异步 listen() 监听器泄漏模式及修复方案
- `backend/error-handling.md`：AtomicBool `store` vs `compare_exchange` 竞态窗口模式

**Modified Files**: `src-tauri/src/assistant_processor.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/openai_client.rs`, `src-tauri/src/pipeline/assistant.rs`, `src-tauri/src/pipeline/mod.rs`, `src-tauri/src/pipeline/types.rs`, `src/types/assistant-result.ts`, `src/windows/ResultPanelWindow.tsx`, `tests/assistantResultPanel.test.ts`


### Git Commits

| Hash | Message |
|------|---------|
| `630800c` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: AI 助手结果面板支持文本输入追问

**Date**: 2026-04-13
**Task**: AI 助手结果面板支持文本输入追问
**Branch**: `main`

### Summary

(Add summary)

### Main Changes

## 概述

为 AI 助手结果面板添加文本输入追问能力，作为语音追问的补充通道。用户在面板打开时可以直接打字追问，跳过录音/ASR/TNL 直接调用 LLM。

## 改动清单

| 文件 | 改动 |
|------|------|
| `src-tauri/src/lib.rs` | 新增 `send_text_question` IPC 命令（~106 行），复用 `process_followup` 逻辑 |
| `src/types/assistant-result.ts` | 新增 `formatTimingDisplay` 纯函数（19 行） |
| `src/windows/ResultPanelWindow.tsx` | 新增 `TextInputBar` 子组件 + `AssistantBubble` 耗时显示修改（~95 行） |
| `tests/assistantResultPanel.test.ts` | 新增 3 个 `formatTimingDisplay` 测试用例 |
| `CLAUDE.md` | 文档同步：新增 `send_text_question` 命令描述 |

## 设计决策

- **方案 A（仅追问）**：文本输入仅在面板已打开时可用，不引入新 UI 入口
- **耗时自适应**：`asr_time_ms = 0` 时只显示 "LLM x.xs"，语音轮次不变
- **并发保护**：文本追问和语音追问共享 `is_assistant_processing` 原子标志

## TDD 流程

- Slice 1: `formatTimingDisplay` 纯函数（RED→GREEN，3 个测试用例）
- Slice 2: 后端 `send_text_question` IPC 命令（`cargo check` 验证）
- Slice 3: 前端 `TextInputBar` 组件 + `AssistantBubble` 修改（`tsc --noEmit` + 全量测试验证）


### Git Commits

| Hash | Message |
|------|---------|
| `c8859d1` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 4: GitNexus 升级 + hook 接入；归档 TNL 候选仲裁任务

**Date**: 2026-05-09
**Task**: GitNexus 升级 + hook 接入；归档 TNL 候选仲裁任务
**Branch**: `main`

### Summary

升级 gitnexus 到 1.6.3；新增 PreToolUse(Grep|Glob|Bash)+PostToolUse(Bash) gitnexus-hook.cjs 钩子；归档已实现的 tnl-candidate-arbitration 任务（实现见 50ef68a 与 446eb19，所有 AC 已满足）

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `50ef68a` | (see git log) |
| `446eb19` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 5: per-preset LLM 模型选择 (issue #12)

**Date**: 2026-05-10
**Task**: per-preset LLM 模型选择 (issue #12)
**Branch**: `main`

### Summary

实现每个润色预设独立选择 LLM Provider/模型；经过 v1→v4 设计演进，最终采用 inline 模型下拉而非覆盖+徽章方案；后端 11 单测、前端 14 单测全过；附带 Trellis 0.5.9 平台接入整理

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `199f34a` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 6: 修复 AI 助手联网搜索状态与取消流程

**Date**: 2026-05-12
**Task**: 修复 AI 助手联网搜索状态与取消流程
**Branch**: `main`

### Summary

修复联网搜索结果面板恢复、取消/重试、文本追问后台化、搜索配置短路、SSE CRLF 与历史工具上下文等问题，并通过 cargo/npm 定向验证。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `65e5d32` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 7: ASR personalization MVP closure

**Date**: 2026-05-15
**Task**: ASR personalization MVP closure
**Branch**: `main`

### Summary

完成 ASR 个性化 0-3 MVP 闭环并提交 Phase 4 后端配置入口：观察/撤销/LLM 仲裁反馈回写、中置信仲裁、去口癖、运行时 pass 配置、disfluency 配置；验证 cargo test/check 与 ASR eval 通过。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `0fea5b4` | (see git log) |
| `4d0b77c` | (see git log) |
| `24a409b` | (see git log) |
| `fcb80fa` | (see git log) |
| `075a403` | (see git log) |
| `6aebf90` | (see git log) |
| `58cd0a2` | (see git log) |
| `96e1ce3` | (see git log) |
| `26703c7` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 8: ASR 个性化剩余阶段闭环

**Date**: 2026-05-16
**Task**: ASR 个性化剩余阶段闭环
**Branch**: `main`

### Summary

完成 Phase 4 前端配置闭环、Phase 5 词库 category metadata、Phase 6 jieba 用户词专名 span、Phase 7 HotwordCompiler 与 provider 等价接入，并补齐 correction pair 编译 API；相关目标测试和 cargo check 已通过。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `b2ca8db` | (see git log) |
| `a239488` | (see git log) |
| `a4cc196` | (see git log) |
| `da480e5` | (see git log) |
| `c8d149b` | (see git log) |
| `76b8033` | (see git log) |
| `efe2735` | (see git log) |
| `a0b8783` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 9: ASR 热词运行时 correction pairs 来源

**Date**: 2026-05-16
**Task**: ASR 热词运行时 correction pairs 来源
**Branch**: `main`

### Summary

让 Qwen/Doubao HTTP 与 realtime ASR 热词编译消费运行时 correction-pair 快照，服务启动与接受学习词后刷新缓存，保持 provider payload 兼容；hotword 相关目标测试和 cargo check 通过。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `870b19f` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 10: ASR hotword source metadata

**Date**: 2026-05-16
**Task**: ASR hotword source metadata
**Branch**: `main`

### Summary

Preserved runtime dictionary source metadata so selected builtin domain words rank as ASR domain hotwords instead of manual user words; added TNL metadata purification and updated tests/specs.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `f50558d` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 11: ASR recent hotword runtime source

**Date**: 2026-05-16
**Task**: ASR recent hotword runtime source
**Branch**: `main`

### Summary

Implemented runtime-only recent ASR hotwords from successful 24h history, wired them into runtime dictionary refresh, added frontend/runtime flow tests, backend source-priority coverage, and updated ASR hotword spec.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `c896d72` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 12: ASR 当前应用上下文热词

**Date**: 2026-05-16
**Task**: ASR 当前应用上下文热词
**Branch**: `main`

### Summary

完成录音开始前的当前 App UIA 上下文热词提取与 runtime-only dictionary 追加，覆盖保守提取、URL/email 过滤、上限和 hotword 编译链路验证。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `c969d34` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 13: 确认口语流畅化 UI 状态

**Date**: 2026-05-16
**Task**: 确认口语流畅化 UI 状态
**Branch**: `main`

### Summary

验证 Phase 4 口语流畅化三档 UI 与 tnlConfig.disfluencyMode 字段级 patch 已完成，并同步 ASR 个性化路线图状态。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `43ffbba` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 14: 助手语音候选仲裁

**Date**: 2026-05-16
**Task**: 助手语音候选仲裁
**Branch**: `main`

### Summary

接入 AI 助手语音指令的中置信 TNL/个性化候选仲裁，复用既有 bounded LLM candidate arbiter，并将真实 LLM apply/reject 弱反馈写回 correction pair。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `38df5df` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 15: Phase 5 TNL category routing

**Date**: 2026-05-16
**Task**: Phase 5 TNL category routing
**Branch**: `main`

### Summary

接入 TNL 词库 category 路由：email/url 跳过字典改写路径，code_symbol 跳过 phonetic/fuzzy，产品/术语类继续保留音近修正；同步 Phase 5 路线图和 TNL spec。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `78af2e2` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 16: Phase 5 phrase dictionary prepass

**Date**: 2026-05-16
**Task**: Phase 5 phrase dictionary prepass
**Branch**: `main`

### Summary

为 phrase category 接入 TNL 轻量短语优先匹配：ASCII 短语大小写规范化、中文短语吞字间空白、不跨标点，并同步 Phase 5 路线图和 TNL spec。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `a87384f` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 17: Phase 5 learning category taxonomy

**Date**: 2026-05-16
**Task**: Phase 5 learning category taxonomy
**Branch**: `main`

### Summary

将自动词库学习的 LLM 分类升级为完整词库 taxonomy，兼容旧 proper_noun/term/frequent 别名，更新 Toast 标签、TS 类型、事件契约和 Phase 5 路线图。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `b4127b8` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 18: Phase 5 backend category inference

**Date**: 2026-05-16
**Task**: Phase 5 backend category inference
**Branch**: `main`

### Summary

Added backend dictionary category inference helpers aligned with frontend rules, routed add_learned_word through inferred-category upsert, covered missing/invalid/existing metadata cases with Rust tests, and synced the TNL spec plus ASR roadmap.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `5f3cad7` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 19: Phase 5 dictionary category backfill

**Date**: 2026-05-17
**Task**: Phase 5 dictionary category backfill
**Branch**: `main`

### Summary

Added deterministic backend dictionary category backfill for config load/save, preserving generic compact storage and existing metadata while canonicalizing legacy aliases. Covered dictionary, AppConfig, and save_config paths with Rust tests and synced the TNL spec plus ASR roadmap.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `08a44e0` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 20: Phase 5 phrase dictionary runtime index

**Date**: 2026-05-17
**Task**: Phase 5 phrase dictionary runtime index
**Branch**: `main`

### Summary

Added a runtime first-segment/first-character index for TNL phrase dictionary rules, preserved phrase matching behavior with regression tests, and updated the TNL spec plus ASR personalization roadmap.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `61b8ab2` | (see git log) |
| `c6a5ff5` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 21: Phase 5 user_terms SQLite store

**Date**: 2026-05-17
**Task**: Phase 5 user_terms SQLite store
**Branch**: `main`

### Summary

Added a rusqlite-backed user_terms sidecar store with schema/index creation, dictionary metadata hydration, reopen/idempotency tests, and updated database/roadmap docs while leaving production runtime paths on AppConfig.dictionary.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `021b2da` | (see git log) |
| `23f3b5b` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 22: Phase 5 user term phonetic keys

**Date**: 2026-05-17
**Task**: Phase 5 user term phonetic keys
**Branch**: `main`

### Summary

Hydrated user_terms SQLite rows with existing phonetic key generation, added regression assertions for English and CJK key columns, and updated database/roadmap docs.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `a23d9f3` | (see git log) |
| `c3122cb` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 23: Phase 5 user term key queries

**Date**: 2026-05-17
**Task**: Phase 5 user term key queries
**Branch**: `main`

### Summary

Added enabled-row lookup APIs for user_terms by English phonetic and Chinese fuzzy pinyin keys, with deterministic manual-first ordering, empty-key handling, disabled-row filtering, tests, and docs.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7f0c5bc` | (see git log) |
| `0b53a4d` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 24: Phase 5 user terms sidecar sync

**Date**: 2026-05-17
**Task**: Phase 5 user terms sidecar sync
**Branch**: `main`

### Summary

Synced AppConfig.dictionary into the user_terms SQLite sidecar during persisted config load/save, warning-only on sidecar failures; hydration now treats the current config as a snapshot and disables missing enabled rows while leaving ASR/TNL runtime consumers on AppConfig.dictionary.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `48e89fc` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 25: Fix dictionary category metadata test

**Date**: 2026-05-17
**Task**: Fix dictionary category metadata test
**Branch**: `main`

### Summary

Updated the stale TypeScript regression assertion to match the current add_learned_word backend path through upsert_entry_with_inferred_category while still verifying category.as_deref() is passed; restored npm run test:ts to 120/120 passing.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `56f4a3c` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 26: Phase 5 runtime user terms read path

**Date**: 2026-05-17
**Task**: Phase 5 runtime user terms read path
**Branch**: `main`

### Summary

Added enabled user_terms export preserving source/category metadata and made backend-controlled service restarts prefer user_terms.db runtime entries with warning-only fallback to normalized AppConfig.dictionary; start_app frontend runtime merge remains unchanged.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `0a92137` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 27: Phase 5 runtime dictionary sidecar merge

**Date**: 2026-05-17
**Task**: Phase 5 runtime dictionary sidecar merge
**Branch**: `main`

### Summary

Merged enabled user_terms.db entries into start_app runtime dictionaries while preserving domain, recent, builtin, and app_context runtime sources. Sidecar user metadata now wins duplicate words, with warning-only fallback to normalized input entries when the sidecar is missing, unreadable, or empty.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `cb73915` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 28: Phase 5 dictionary command sidecar persistence

**Date**: 2026-05-17
**Task**: Phase 5 dictionary command sidecar persistence
**Branch**: `main`

### Summary

Moved dictionary management commands to user_terms.db sidecar-first persistence. get_dictionary_entries now reads enabled sidecar terms with config bootstrap fallback; add_learned_word and delete_dictionary_entries upsert/disable sidecar entries, mirror enabled entries back to AppConfig.dictionary as a compatibility snapshot, and keep runtime dictionary metadata for ASR/TNL consumers.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `9353696` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete
