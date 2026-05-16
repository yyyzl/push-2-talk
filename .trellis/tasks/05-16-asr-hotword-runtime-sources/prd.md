# 继续 ASR 热词运行时个性化来源

## Goal

在 Phase 7 HotwordCompiler 已经统一 provider 热词格式的基础上，补齐一个小而直接的运行时闭环：让 ASR 热词 pack 在录音请求前安全消费已存在的 `correction_pairs.corrected_text`，把用户已经确认过的个性化纠错经验提前作为云端 ASR bias，而不是只在 ASR 文本返回之后再二次解码。

## What I Already Know

* 上一个任务已完成并归档：Phase 4 UI/config、Phase 5 category metadata、Phase 6 jieba 用户词专名 span、Phase 7 HotwordCompiler 核心与 provider 等价接入都已提交。
* `ASR_PERSONALIZATION_QUALITY_LEAP.md` 的 Phase 7 编译依据包含 `correction_pairs.corrected_text`，权重低于手动用户词、高于自动用户词。
* `src-tauri/src/personalization/hotword_compiler.rs` 已有 `compile_asr_pack_with_correction_pairs`，单测覆盖排序、去重、禁用/空值过滤、TNL/LLM pack hints。
* 当前四个 ASR provider helper 仍只调用 `compile_user_dictionary_asr_pack(dictionary, limit)`。
* `correction_pairs.json` 路径由 `personalization::default_correction_pairs_path()` 解析，普通听写和 AI 助手的本地二次解码会在 ASR 文本返回后加载它。
* `AppState` 里已有运行时 `dictionary: Arc<Mutex<Vec<String>>>`，`start_app` / 热更新 / `add_learned_word` / `delete_dictionary_entries` 会维护词库并更新 HTTP 客户端。
* 直接在 provider request builder 或音频 chunk 路径同步读取 `correction_pairs.json` 是 spec 明确禁止的坏例子。
* 当前工作区仍有未提交版本号变更：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`，本任务不处理它们。

## Assumptions

* 本切片优先做 correction pair 运行时来源，不进入 Phase 8 本地 reranker。
* 本切片保持 provider payload 形状兼容：Qwen 仍是顿号拼接 corpus text，Doubao 仍是 `{"word": "..."}` 热词数组。
* 本切片不要求完整“最近 24h / 当前 App context / 活跃领域词”编译来源；它们后续再拆小任务。

## Requirements

* 运行时必须有一个可复用的 ASR 热词来源快照，至少包含：
  * 当前纯用户词 `Vec<String>`。
  * 已加载的 `CorrectionPair` 列表。
* 该快照必须在服务启动、词库变更、接受学习词/纠错对后刷新。
* provider request builder 不得在每次构建请求、每个音频 chunk 或 realtime send loop 中同步读取 `correction_pairs.json`。
* 当 `correction_pairs.json` 不存在或加载失败时，ASR provider 应保守降级为仅用户词热词，不阻断录音。
* Qwen HTTP / Qwen Realtime / Doubao HTTP / Doubao Realtime 应能消费同一套包含 correction pairs 的 HotwordCompiler API。
* correction pair 热词仍只使用 `corrected_text` 进入 provider payload；`original_text` / `alias_keys` 仅作为 pack alias/hint，不进入当前 provider payload。
* 手动用户词与 correction pair 纠正文重复时，手动用户词继续胜出。

## Acceptance Criteria

* [x] 新增或扩展运行时结构，能缓存 dictionary + correction pairs 的 ASR 热词输入。
* [x] 服务启动时加载 correction pairs；文件缺失时得到空列表且不报错。
* [x] `add_learned_word` 保存 correction pair 后，运行时 ASR 热词输入同步刷新。
* [x] `delete_dictionary_entries` 仍只删除 dictionary，不破坏 correction pair cache。
* [x] 四个 provider helper 使用 `compile_asr_pack_with_correction_pairs` 或等价封装，并保留旧 payload 形状。
* [x] 最小单测覆盖：correction pair 出现在 Qwen corpus / Doubao hotwords，缺失文件降级，correction pair store 加载。
* [x] 目标 Rust 测试通过；至少 `cargo check` 通过（若 Cargo TLS 仍失败，使用本机临时 `jieba-rs` stub 的离线验证并记录限制）。

## Definition Of Done

* 测试已新增或更新。
* 相关最小测试通过。
* Provider payload 形状兼容性保持不变。
* 不提交与本任务无关的版本号变更。
* 如果学到新的运行时热词缓存约定，同步更新 `.trellis/spec/backend/asr-hotword-compilation.md`。

## Technical Approach

1. 先给 provider helper 增加传入 correction pairs 的测试，证明 corpus/hotwords 会包含 `corrected_text` 且不包含 metadata/alias。
2. 抽一个小的运行时热词输入结构或 helper，集中负责从 `default_correction_pairs_path()` 加载 correction pairs，缺失时返回空列表。
3. 在 `AppState` 中增加可锁定的 correction pair cache 或 ASR hotword source snapshot。
4. 在 `start_app` 初始化、`update_runtime_config` 词库热更新、`add_learned_word` 保存 correction pair 后刷新 cache。
5. 将 Qwen/Doubao HTTP 客户端和 realtime 录音开始时读取的词库来源扩展为 dictionary + correction pairs。
6. 保持旧的单参数 provider helper 或测试 helper 兼容，内部委托到新 helper，降低高风险调用点改动面。
7. 跑 HotwordCompiler/provider 目标测试和 `cargo check`。

## Decision (ADR-lite)

**Context**: Phase 7 的下一步有两个方向：继续补 ASR 上游热词来源，或进入 Phase 8 本地 reranker。Phase 8 工程量大、默认关闭且需要评测集证明残留问题；而 correction pairs 已经存在、HotwordCompiler API 已准备好，运行时接入能直接提升下一次 ASR 首次识别命中率。

**Decision**: 本任务先做 correction pairs 的运行时热词来源接入，并用缓存/快照避免 provider 请求路径反复读文件；最近 24h、App context、领域词和本地 reranker 继续延后。

**Consequences**: 可以把已学习的“cloud code -> Claude Code”等纠错经验提前喂给云端 ASR，同时保持当前 provider payload 兼容。代价是运行时状态多一份 cache，需要在保存/删除/启动路径维护一致性。

## Out Of Scope

* Phase 8 本地 reranker。
* 最近 24h 用词、当前 App context、活跃领域词的热词来源。
* provider payload 升级为带权重对象。
* SQLite/索引化 correction pair store。
* 完整 ASR 首次识别命中率离线评测集扩展。
* 当前未提交的 `1.6.3` 版本号变更。

## Technical Notes

* 主要代码入口：
  * `src-tauri/src/personalization/hotword_compiler.rs`
  * `src-tauri/src/personalization/correction_pair_store.rs`
  * `src-tauri/src/asr/http/qwen.rs`
  * `src-tauri/src/asr/http/doubao.rs`
  * `src-tauri/src/asr/realtime/qwen.rs`
  * `src-tauri/src/asr/realtime/doubao.rs`
  * `src-tauri/src/lib.rs`
* 需要遵守 `.trellis/spec/backend/asr-hotword-compilation.md`：不得在 provider runtime paths 里同步加载 `correction_pairs.json`，不得改变 Doubao legacy hotwords shape。
* 2026-05-16 本地验证：使用临时本地 `jieba-rs` 0.9 API 兼容 stub，跑过 `runtime_correction_pairs`、`asr_hotword_runtime_tests` 和 `cargo check --offline`。
