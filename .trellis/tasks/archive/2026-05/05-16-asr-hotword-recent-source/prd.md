# 继续 ASR 热词最近用词来源

## Goal

把 Phase 7 里的“最近 24h 用过的词”先接入 ASR 运行时热词来源，让正式识别请求能利用用户刚刚说过、刚刚成功转写过的产品名、工具名和代码词。这个任务只做最小闭环：从前端本地历史提取候选词，作为 `recent` 来源进入运行时 dictionary，后端继续使用现有 HotwordCompiler 权重排序。

## What I Already Know

- ASR 运行时 dictionary 已经支持 `word|source|category` 元数据格式。
- 后端 HotwordCompiler 已识别 `recent` 来源，并按 `manual > correction_pair > recent > app_context > auto > domain > builtin` 排序。
- 前端成功转写历史记录已经保存在 `HistoryRecord[]` / localStorage 中，包含 `timestamp`、`originalText`、`polishedText`、`success`、`mode` 等字段。
- `useAppServiceController` 是运行时 dictionary 的组装入口，目前合并用户词库和已选内置领域词。

## Requirements

- 从成功历史记录中提取最近 24 小时的保守热词候选。
- 只使用成功记录；失败、空文本、超过 24 小时的记录不能进入热词。
- 优先使用最终用户可见文本：`polishedText ?? originalText`。
- 候选词必须保守，优先覆盖英文产品名、工具名、代码符号、带数字/连字符/大小写特征的技术词，避免把普通句子碎片塞进 ASR。
- 生成的运行时条目格式为 `word|recent|category`，并且不写回后端持久化配置。
- 运行时 dictionary 合并顺序必须保持：用户词库 > 最近词 > 内置领域词；按纯词去重，前者优先。
- 当历史记录新增后，running 状态下应触发 `update_runtime_config` 刷新 ASR 热词。
- 不改变已有 provider payload 形状：Qwen 仍是纯词 corpus；Doubao 仍是 `{"word": "<pure word>"}`。

## Acceptance Criteria

- [ ] 新增/更新前端测试，证明最近热词只来自 24h 内成功历史，输出 `recent` metadata，并去重/过滤噪声。
- [ ] 新增/更新流程测试，证明 `App` 把最近热词纳入运行时配置 hash，并传给 `useAppServiceController`。
- [ ] 更新 HotwordCompiler 测试，证明 `recent` 排名在 correction pair 之后、auto/domain 之前。
- [ ] `npm run test:ts` 通过。
- [ ] `npm run build` 通过。
- [ ] 相关 Rust hotword 测试通过，且 `cargo check` 通过。

## Out of Scope

- 不新增后端数据库/文件存储。
- 不新增最近热词 UI 管理页。
- 不做 app_context 当前窗口上下文来源。
- 不做 4~8 阶段的长线自动学习、纠错确认和 eval 面板。

## Technical Notes

- 前端数据流：`HistoryRecord[]` → recent hotword utility → `recentHotwordEntries` → `buildRuntimeDictionary` → `start_app/update_runtime_config` dictionary。
- 后端边界：HotwordCompiler 和 TNL 均应只消费纯词；metadata 只用于来源排序。
- 风险点：历史变化不应触发持久化保存配置，只应触发运行时热更新。
