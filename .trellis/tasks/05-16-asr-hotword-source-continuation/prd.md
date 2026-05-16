# 继续 ASR 个性化热词来源闭环

## Goal

让 Phase 7 HotwordCompiler 已经支持的来源权重真正进入运行时链路：前端启动/热更新服务时保留用户词、自动词、内置领域词的来源元数据；后端 ASR 热词按来源排序，同时 TNL/LLM 继续只消费纯词，避免内置领域词被误当成最高优先级手动词。

## What I Already Know

* `ASR_PERSONALIZATION_QUALITY_LEAP.md` 的 Phase 7 要求热词来源按手动用户词、纠错对、最近词、App context、活跃领域词、内置词库排序。
* `src-tauri/src/personalization/hotword_compiler.rs` 已有 `HotwordSource::{Recent, AppContext, Domain, Builtin}` 枚举和权重，但当前只从词典条目的 `auto`/manual 与 correction pairs 生成实际来源。
* 前端 `buildRuntimeDictionary` 会把已选择的内置领域词合并进传给 `start_app` / `update_runtime_config` 的 dictionary。
* 当前前端合并时使用纯词，后端 `HotwordCompiler` 会把没有元数据的词条视为 `ManualUser`，导致内置领域词获得最高权重。
* Rust LLM 后处理会调用 `dictionary_utils::entries_to_words` 提纯词库；TNL 当前构造函数注释要求“已提纯的词库”，但入口没有自我提纯。

## Assumptions

* 本任务只修“活跃领域词/内置词来源权重”这条运行时链路，不新增最近 24h 用词存储，也不接 App context。
* 运行时 dictionary 可以携带 `"word|builtin|domain_term"` 这类元数据，但持久化配置仍只保存用户词，不把内置词写入用户词库。
* 后端 TNL 应在边界处自我提纯，保证传入带元数据的运行时 dictionary 不影响现有匹配行为。

## Requirements

* 前端运行时 dictionary 保留用户词条的 source/category 元数据。
* 前端将内置领域词追加为低权重来源条目，不能持久化进用户词典。
* 后端 `HotwordCompiler` 能识别 `builtin` / `domain` 来源，并按低于 `auto` 的权重排序。
* 后端 TNL 构造入口能安全接收带元数据的词库，并内部转换为纯词。
* Provider outbound payload 继续只包含纯词，不暴露 metadata。

## Acceptance Criteria

* [ ] 单元测试证明运行时 dictionary 会把内置领域词标记为 builtin/domain 来源。
* [ ] 单元测试证明 HotwordCompiler 中 correction pair > auto user > domain/builtin，且 provider payload 不含 metadata。
* [ ] 单元测试证明 TNL 接收 `"Claude Code|manual|product"` 仍能按 `Claude Code` 匹配。
* [ ] 相关 TypeScript runtime tests 通过。
* [ ] 相关 Rust hotword/TNL tests 通过，`cargo check` 通过。

## Definition of Done

* 测试先补，再实现。
* 只改 ASR 热词来源元数据闭环相关文件。
* 更新 `.trellis/spec/backend/asr-hotword-compilation.md` 中的来源契约。
* 提交前运行 GitNexus detect changes，确认影响范围符合预期。

## Out of Scope

* 最近 24h 用词存储与提取。
* 当前 App context 词提取。
* Hotword pack 缓存复用。
* Doubao weighted hotword JSON 新格式兼容验证。
* Phase 8 本地 reranker。

## Technical Notes

* 主要候选文件：
  * `src/hooks/useAppServiceController.ts`
  * `src/utils/dictionaryUtils.ts`
  * `src/utils/builtinDictionary.ts`
  * `src-tauri/src/personalization/hotword_compiler.rs`
  * `src-tauri/src/tnl/engine.rs`
  * `.trellis/spec/backend/asr-hotword-compilation.md`
* 当前工作区已有无关版本号改动：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`，本任务不触碰、不提交。
