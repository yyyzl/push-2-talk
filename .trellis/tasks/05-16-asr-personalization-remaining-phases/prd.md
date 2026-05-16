# 继续 ASR 个性化剩余阶段

## Goal

在 0-3 阶段已经闭环的基础上，继续推进不阻塞核心识别链路、但能直接提升日常体验的剩余工作。Phase 4 的口语流畅化 UI/config 闭环已完成并提交；下一步推进 Phase 5 的最小可交付切片：在现有 JSON/string 词库形态上扩展用户词 category metadata，先打通保存、读取、展示、人工微调闭环，不强行引入 SQLite、分词索引或 HotwordCompiler。

## What I Already Know

* `ASR_PERSONALIZATION_QUALITY_LEAP.md` 明确记录：Phase 0-3 已完成 MVP 垂直闭环。
* Phase 4 后端第一刀已完成：`clean_disfluency(text, mode)`、`DisfluencyMode`、`TnlConfig.disfluency_mode`、普通听写和 AI 助手路径按配置创建 `TnlEngine`。
* Phase 4 已完成：UI 三档开关、字段级 patch、前后端配置类型和最小回归测试都已闭环。
* Phase 5 文档明确：如果 P1 仍处于 JSON 旁路阶段，先扩展现有词典 entry metadata，不强行要求数据库已完成。
* 当前词库仍以 `Vec<String>` / `string[]` 存储，旧格式是 `"word"` 或 `"word|auto"`；前端 `DictionaryEntry` 只保留 `source`，没有 `category`。
* 当前工作区已有版本号 `1.6.2 -> 1.6.3` 的未提交变更，本任务不把它作为功能改动处理。

## Requirements

* 前端配置类型要包含 `tnl_config` 与 `disfluency_mode`，与后端 serde 字段保持一致。
* 在合适的设置页面提供口语流畅化三档切换：
  * `off`：完全关闭清洗。
  * `conservative`：默认保守模式，仅清理明确句首/独立填充词。
  * `aggressive`：更激进地处理重复字、拖长音和 false start。
* 配置加载时使用后端默认值兜底；旧配置文件没有字段时不影响启动。
* 保存配置时通过字段级 patch 更新 `tnl_config.disfluency_mode`，不得破坏现有 TNL、词库、后处理、学习等配置。
* 文案要让用户能理解模式差异，但界面保持项目当前设置页风格。
* 词库 category 要兼容旧数据：
  * `"word"` 解析为手动来源和默认/推断 category。
  * `"word|auto"` 解析为自动来源和默认/推断 category。
  * 新格式可保存非默认 category，且 `entries_to_words` / ASR 热词输入仍只拿纯词。
* Category 候选至少覆盖 `person / product / tool / phrase / email / url / code_symbol / domain_term / generic`。
* 先实现确定性推断：email、url、code symbol、中文短语、兜底 generic；LLM 批量判断、SQLite 索引、phrase trie 留给后续切片。
* DictionaryPage 需要展示 category，并允许用户在不改词面的情况下手动调整 category。

## Acceptance Criteria

* [x] TypeScript 类型覆盖 `tnl_config.disfluency_mode`。
* [x] 前端能展示并切换 `关闭 / 保守 / 强力` 三档。
* [x] 保存后 `patch_config_fields` payload 包含更新后的 `tnlConfig.disfluencyMode`。
* [x] 配置加载和即时保存不会丢失既有字段。
* [x] 至少补充一个 TypeScript runtime regression test 覆盖配置字段保存或类型/源码契约。
* [x] `npm run test:ts` 通过；Rust 配置 patch 目标单测和 `cargo check` 通过。
* [x] `DictionaryEntry` 类型包含 category，并有统一的 category label/option 定义。
* [x] 前端解析旧词库字符串时能推断 category，新格式保存时能保留非默认 category。
* [x] 后端词库工具能读写 `word|source|category` 形态，同时 `entries_to_words` 仍输出纯词。
* [x] `add_learned_word` 传入 category 时能持久化 metadata；旧调用不传 category 时行为不变。
* [x] DictionaryPage 展示 category，且用户可手动修改 category。
* [x] 补充前端和后端最小回归测试，覆盖 category round-trip 与旧格式兼容。

## Definition Of Done

* 测试已新增或更新。
* 相关最小测试通过。
* 代码风格与现有页面、hook、保存逻辑一致。
* 不提交与本任务无关的版本号变更。

## Technical Approach

1. 先定位现有配置加载/保存链路和 `PreferencesPage`/右侧快捷设置模式。
2. 扩展前端 `AppConfig`/`TnlConfig` 类型与默认归一化逻辑。
3. 在设置页增加三档分段/按钮式控件，并通过现有 `saveImmediately` 或同等保存入口持久化。
4. 补充源码级 runtime test，防止后续删除 `tnl_config.disfluency_mode` 的保存链路。
5. 跑前端 runtime tests，并视改动范围跑 Rust 配置测试。
6. Phase 5 小切片先扩展 `dictionaryUtils` / `dictionary_utils.rs` 的兼容格式，增加 category 推断与 round-trip 测试。
7. 在 DictionaryPage 复用现有个人词库 UI，增加 category badge/select，并通过现有 `add_learned_word` 持久化微调。

## Decision (ADR-lite)

**Context**: 剩余阶段 4-8 范围很大，Phase 5-8 分别涉及词典分类、分词/NER、HotwordCompiler、本地 reranker，适合作为独立任务。Phase 4 已有后端基础，只差 UI/config，能用最小风险把“文本更干净”能力交给用户。Phase 4 已在本任务内完成并提交。

**Decision**: 本任务先完成 Phase 4 UI/config 闭环；随后继续 Phase 5 的 JSON metadata 最小闭环，只做 category 的类型、存储兼容、页面展示和手动微调。

**Consequences**: 可以快速交付一个可感知的质量提升，并为后续 phrase trie、不同 category lookup、HotwordCompiler 和 SQLite 迁移留下稳定的 metadata 基础，同时避免在一个任务里同时引入 ASR provider hotword 编译和本地模型评估等高风险变化。

## Out Of Scope

* Phase 0B 的 80-120 条真实评测集扩展。
* 完整 ConvertPipeline trait 化。
* 助手路径中置信候选独立云端仲裁。
* Phase 5 的 SQLite 分表、索引、phrase trie、不同 lookup path。
* Phase 6 jieba/NER。
* Phase 7 HotwordCompiler。
* Phase 8 本地 reranker。
* 当前未提交的 `1.6.3` 版本号变更。

## Technical Notes

* 主要入口：
  * `src-tauri/src/config.rs`：`TnlConfig.disfluency_mode` 已存在。
  * `src-tauri/src/tnl/disfluency.rs`：后端清洗规则与测试。
  * `src/types/index.ts`：前端配置类型目前缺少 `tnl_config`。
  * `src/hooks/useAppServiceController.ts`：配置加载、保存与 runtime config 构建。
  * `src/pages/PreferencesPage.tsx`：更适合承载全局输入/规范化偏好。
* Codex dispatch mode 为 inline，本任务 Phase 2 直接加载 `trellis-before-dev` 后在主会话实现。
