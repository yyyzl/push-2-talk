# 继续 ASR 个性化剩余阶段

## Goal

在 0-3 阶段已经闭环的基础上，继续推进不阻塞核心识别链路、但能直接提升日常体验的剩余工作。Phase 4 的口语流畅化 UI/config 闭环、Phase 5 的用户词 category metadata 小闭环、Phase 6 的 `jieba-rs` 用户词专名 span 小闭环已完成并提交；下一步推进 Phase 7 的最小可交付切片：新增 HotwordCompiler 核心，统一 Qwen/Doubao 现有 ASR 热词输入的提纯、去重、权重排序和 provider 上限，先保持现有 payload 形状兼容，不引入 app context 缓存或完整评测。

## What I Already Know

* `ASR_PERSONALIZATION_QUALITY_LEAP.md` 明确记录：Phase 0-3 已完成 MVP 垂直闭环。
* Phase 4 后端第一刀已完成：`clean_disfluency(text, mode)`、`DisfluencyMode`、`TnlConfig.disfluency_mode`、普通听写和 AI 助手路径按配置创建 `TnlEngine`。
* Phase 4 已完成：UI 三档开关、字段级 patch、前后端配置类型和最小回归测试都已闭环。
* Phase 5 文档明确：如果 P1 仍处于 JSON 旁路阶段，先扩展现有词典 entry metadata，不强行要求数据库已完成。
* 当前词库仍以 `Vec<String>` / `string[]` 存储，旧格式是 `"word"` 或 `"word|auto"`；前端 `DictionaryEntry` 只保留 `source`，没有 `category`。
* Phase 5 已完成：`DictionaryEntry.category`、`word|source|category` 兼容格式、DictionaryPage 分类微调和后端 round-trip 测试都已闭环。
* Phase 6 已完成：`jieba-rs` 注入用户词，TNL 能产出 `NamedEntity` span，且旧技术 span 优先级更高。
* Phase 7 文档要求新增 `personalization/hotword_compiler.rs`，让 ASR providers 统一消费编译后的热词 pack。
* 直接改 ASR provider 运行时路径影响较高：Doubao HTTP `transcribe_bytes` 为 CRITICAL，Qwen HTTP `transcribe_from_memory` 为 HIGH；本切片必须保持现有请求 payload 形状等价，只做编译来源统一、去重、排序和上限裁剪。
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
* `jieba-rs` 需要作为 TNL 后端依赖接入，使用默认词典并注入当前用户词。
* `TechSpanDetector` 需要能接收用户词库，并通过分词结果标记用户词专名片段。
* 新增专名 span 必须保持旧技术片段优先级：URL、邮箱、路径、文件名等既有识别不能被用户词 span 覆盖。
* 默认无词库路径仍应保持现有行为和构造 API 兼容。
* 新增 `personalization::hotword_compiler`，至少定义 `AsrHotwordPack` / `AsrHotword` / `TnlDictionaryPack` / `LlmContextPack` 的核心结构。
* HotwordCompiler 需要从现有 `Vec<String>` 词库格式读取纯词、source 和 category；旧格式 `"word"` / `"word|auto"` / 新格式 `"word|source|category"` 都要兼容。
* ASR pack 需要对同一纯词去重，手动词优先于自动词，并按权重排序后按 provider 上限截断。
* Qwen HTTP / Qwen Realtime / Doubao HTTP / Doubao Realtime 的现有热词构建逻辑统一改为消费 HotwordCompiler 输出。
* 本切片保持 provider payload 形状兼容：Qwen 仍输出顿号拼接 corpus text，Doubao 仍输出 `{"word": "..."}`
  hotwords 数组；权重先保留在 pack 中，不强行改变线上请求格式。

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
* [x] `src-tauri` 引入 `jieba-rs`，且 TNL 能在构造时把用户词注入分词器。
* [x] `TechSpanDetector` 能从用户词分词结果中产出 `NamedEntity` span。
* [x] 既有 URL/邮箱/路径等技术 span 优先级高于 `NamedEntity`。
* [x] TNL 集成测试能证明字典里的中文专名被识别为专名 span，且无字典时不新增该 span。
* [x] 目标 `tech_span` / TNL 测试和 `cargo check` 通过（本机 Cargo/libcurl schannel 无法直连 crates.io；使用临时本地 `jieba-rs` 0.9 API 兼容 stub 完成验证）。
* [x] HotwordCompiler 能把词库编译为去重、排序、截断后的 `AsrHotwordPack`。
* [x] 手动词在重复词冲突时优先于自动词，且 metadata 不进入 ASR 热词文本。
* [x] Qwen Realtime corpus 通过 HotwordCompiler 限制在 provider 上限内。
* [x] Doubao HTTP/Realtme hotwords 通过 HotwordCompiler 构建并保持旧 `{"word": ...}` 形状。
* [x] HotwordCompiler 单测和接入点最小单测通过。

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
8. Phase 6 小切片引入 `jieba-rs`，给 `TechSpanDetector` 增加用户词注入构造器和 `NamedEntity` span。
9. 在 `TnlEngine::new_with_disfluency_mode` 中把纯用户词传给 `TechSpanDetector`，并补充 span 识别回归测试。
10. Phase 7 小切片新增 `personalization/hotword_compiler.rs`，先只消费现有用户词库，不接最近 24h、当前 App context 和领域词。
11. 将 Qwen/Doubao HTTP/Realtme 热词构建替换为 HotwordCompiler helper，并补充 provider 格式单测。

## Decision (ADR-lite)

**Context**: 剩余阶段 4-8 范围很大，Phase 5-8 分别涉及词典分类、分词/NER、HotwordCompiler、本地 reranker，适合作为独立任务。Phase 4 已有后端基础，只差 UI/config，能用最小风险把“文本更干净”能力交给用户。Phase 4 已在本任务内完成并提交。

**Decision**: 本任务先完成 Phase 4 UI/config 闭环；随后继续 Phase 5 的 JSON metadata 最小闭环，只做 category 的类型、存储兼容、页面展示和手动微调；再推进 Phase 6 的 `jieba-rs` 用户词注入与专名 span 保护第一刀；Phase 7 先做 HotwordCompiler 核心与 provider 现有热词构建的等价接入。

**Consequences**: 可以快速交付一个可感知的质量提升，并为后续 phrase trie、不同 category lookup、HotwordCompiler app-context 加权、SQLite 迁移和 SyllableMatchPass 权重调优留下稳定基础，同时避免在一个任务里同时引入本地模型评估等高风险变化。

## Out Of Scope

* Phase 0B 的 80-120 条真实评测集扩展。
* 完整 ConvertPipeline trait 化。
* 助手路径中置信候选独立云端仲裁。
* Phase 5 的 SQLite 分表、索引、phrase trie、不同 lookup path。
* Phase 6 的 ONNX NER、完整词性权重、SyllableMatchPass 分数调参和离线质量评测。
* Phase 7 的最近 24h 用词、当前 App context、活跃领域词、缓存复用和完整首次识别命中率评测。
* Phase 8 本地 reranker。
* 当前未提交的 `1.6.3` 版本号变更。

## Technical Notes

* 主要入口：
  * `src-tauri/src/config.rs`：`TnlConfig.disfluency_mode` 已存在。
  * `src-tauri/src/tnl/disfluency.rs`：后端清洗规则与测试。
  * `src/types/index.ts`：前端配置类型目前缺少 `tnl_config`。
  * `src/hooks/useAppServiceController.ts`：配置加载、保存与 runtime config 构建。
  * `src/pages/PreferencesPage.tsx`：更适合承载全局输入/规范化偏好。
  * `src-tauri/src/tnl/tech_span.rs`：Phase 6 用户词专名 span 检测入口。
  * `src-tauri/src/tnl/engine.rs`：把纯用户词注入 `TechSpanDetector`。
  * `src-tauri/src/personalization/hotword_compiler.rs`：Phase 7 热词编译核心。
  * `src-tauri/src/asr/http/*.rs` 和 `src-tauri/src/asr/realtime/*.rs`：Phase 7 provider 热词格式接入点。
* Codex dispatch mode 为 inline，本任务 Phase 2 直接加载 `trellis-before-dev` 后在主会话实现。
* 2026-05-16 Phase 6 本地验证：真实 `cargo check` 被 Cargo/libcurl schannel 访问 crates.io 的 TLS 握手失败阻断；使用临时本地 `jieba-rs` 0.9 API 兼容 stub 验证新增 `tech_span` / `engine` 目标测试和 `cargo check` 通过，stub 未写入仓库。
* 2026-05-16 Phase 7 本地验证：使用同一个临时 `jieba-rs` 0.9 API 兼容 stub 跑过 HotwordCompiler 单测、Qwen/Doubao provider 接入单测和 `cargo check`；stub 未写入仓库。
