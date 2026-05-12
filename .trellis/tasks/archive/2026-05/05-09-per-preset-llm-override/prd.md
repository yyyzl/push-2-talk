# 润色预设支持独立 LLM 模型覆盖

源自 [issue #12](https://github.com/yyyzl/push-2-talk/issues/12) 诉求 A。诉求 B（AI 助手 Q&A vs 文本处理拆分）暂不在本任务范围。

## Goal

让用户在"语句润色"的多个预设之间，可选地为单个预设覆盖默认 LLM provider/model。常见场景：翻译预设用便宜小模型，润色预设用强模型。**95% 单 provider 用户应完全感知不到这个功能存在**；只有配了多 provider 的高级用户才看得到入口。

## What I already know

### 现有架构（已确认）

- `LlmConfig.presets: Vec<LlmPreset>` 仅含 `id / name / system_prompt`，**所有 preset 共用** `polishing_provider_id` 这一个绑定 ([config.rs:689-693](../../../src-tauri/src/config.rs))
- Preset 切换的热生效依赖 `LlmPostProcessor::compute_config_hash` ([llm_post_processor.rs:104](../../../src-tauri/src/llm_post_processor.rs))，当前 hash 输入：`resolved.endpoint/api_key/model + active_preset_id + 当前 preset 的 system_prompt`
- 资源解析链：`LlmConfig::resolve_polishing()` → `LlmFeatureConfig::resolve_with_feature(&shared, "polishing")` → 优先取 `feature.provider_id` → 回落 `shared.polishing_provider_id` → 回落 `shared.default_provider_id` → 最末降级到 `shared.providers.first()` ([config.rs:850-925](../../../src-tauri/src/config.rs))
- 配置保存路径：前端 `saveConfigThroughGateway` 整体传 `llmConfig` 给 `save_config` ([useAppServiceController.ts:342-368](../../../src/hooks/useAppServiceController.ts) / [lib.rs:763](../../../src-tauri/src/lib.rs))，Rust serde 整体反序列化
- 热生效路径：保存后前端 useEffect 检测 `JSON.stringify({llmConfig,...})` 变化 → 调 `update_runtime_config` → 后端 `existing.config_changed(cfg)` 决定是否重建 `LlmPostProcessor` ([App.tsx:582-628](../../../src/App.tsx) / [lib.rs:3970](../../../src-tauri/src/lib.rs))
- `patch_config_fields` 是白名单 patch，**完全不动 LlmConfig**（[lib.rs:967-1004](../../../src-tauri/src/lib.rs)）。preset 改动必走 `save_config`
- ModelsPage 已有"双向可见"机制：Section 1 功能默认绑定卡片 + Section 2 Provider 卡片右上角"绑定到本 provider 的 feature 图标组" ([ModelsPage.tsx:232-360](../../../src/pages/ModelsPage.tsx))
- ModelsPage 已有删除 provider 的 confirm Modal 模式（`deleteConfirm` state，[ModelsPage.tsx:65-138](../../../src/pages/ModelsPage.tsx)），R6 复用扩展即可
- App.tsx 路由是 `useState<AppPage>` + setter callback 模式（**无 React Router**），跨页面跳转走 props/callback 透传（[App.tsx:127, 702, 760](../../../src/App.tsx)）
- LlmPostProcessor 存储为 `Arc<Mutex<Option<LlmPostProcessor>>>`，使用时 `lock + clone + unlock` 释放锁后调用（[lib.rs:3358](../../../src-tauri/src/lib.rs)）。**重建对正在 in-flight 的请求安全隔离**
- `AppConfig::load()` 返回 `(Self, bool)`，`migrated=true` 时调用方 `load_persisted_config` 自动 save（[lib.rs:307-312](../../../src-tauri/src/lib.rs)）；其他直接读取 load 的路径丢弃 migrated 标志，不写盘
- LlmProvider 数据结构**只有 `default_model: string`，无可用模型列表**（[types/index.ts:62-69](../../../src/types/index.ts)）

### 升级兼容性

- 项目 CLAUDE.md 明确"无需考虑版本向后兼容"，仅配置文件需要兼容旧 JSON
- 新增字段为 `Option<...>`，serde 默认 `None` → 旧配置自动升级，**零 migration 风险**（仅新增**迁移 9** 用于清理理论上不应存在的 state ④）
- 旧用户 `shared.polishing_model` 显式值（迁移 7 第 1605 行保留）在新 resolve 链下仍生效（preset 不覆盖 provider 时走 feature 默认链）
- 新 → 旧回滚：`skip_serializing_if = "Option::is_none"` 让无覆盖的 preset JSON 与旧版一致；有覆盖的 preset 在旧版加载时未知字段被忽略，行为退化为「无覆盖」（与改造前一致）

## Requirements

### R1 — 数据结构

- `LlmPreset` 新增两个可选字段：
  - `provider_id: Option<String>` — 覆盖 polishing 默认 provider 的 ID；`None` 表示沿用 feature 默认
  - `model: Option<String>` — 覆盖 model；`None` 表示沿用 provider 的 `default_model`
- 字段使用 `#[serde(default, skip_serializing_if = "Option::is_none")]` — 旧配置兼容，新配置紧凑
- 前端 `LlmPreset` interface 同步加可选字段
- **Invariant**：`preset.model.is_some() ⇒ preset.provider_id.is_some()`（state ④ 禁止）。前端 popover 强制门控；后端 resolve 时若读到 state ④ 视为状态 ①（fallback feature 默认）+ trace warn；load 时由迁移 9 一次性清理写盘

### R2 — 后端 resolve 链合入 preset 覆盖

- **改造范围限定在 `LlmConfig::resolve_polishing()` 内**（`config.rs:979-983`，当前仅 5 行）
- **不要修改 `LlmFeatureConfig::resolve_with_feature` 的签名** —— 该方法被 polishing / assistant / learning 三个 feature 共用（`config.rs:684, 1179, 981`），preset 是 polishing 独有概念，加 preset 参数会污染通用方法、强迫 assistant/learning 调用方传 None
- **实施模式**：在 `resolve_polishing()` 内查找 active preset；若 `preset.provider_id` 非空且指向有效 provider → 短路返回（按下方 model 优先级解析）；否则 fallthrough 到原 `feature_override.resolve_with_feature(&shared, "polishing")` 保持现状
- **Provider 优先级链**（仅当 active_preset.provider_id 非空时短路）：
  1. `active_preset.provider_id` 指向的 provider（命中即短路）
  2. 若 provider_id 失效（指向已删除 provider）→ trace warn + fallthrough 到下面默认链
  3. `feature_override.provider_id`（来自 `feature_override.resolve_with_feature`）
  4. `shared.polishing_provider_id`
  5. `shared.default_provider_id`
  6. `shared.providers.first()`（最末兜底）
- **Model 优先级链**（**关键：preset 覆盖 provider 时跳过 polishing_model**）：
  - 当 `preset.provider_id.is_some() && provider 仍存在` 时（preset 主动覆盖且未失效）：
    1. `preset.model`
    2. **跳过 `shared.polishing_model`**（避免徽章文案 vs 实际行为错位）
    3. 上一步选中 provider 的 `default_model`
  - 其他所有情况（preset.provider_id 为 None 或失效）：
    1. 走 `feature_override.resolve_with_feature(&shared, "polishing")` 原链路
    2. 等价于 `shared.polishing_model > provider.default_model`
- **`compute_config_hash` 现状**（[llm_post_processor.rs:105-122](../../../src-tauri/src/llm_post_processor.rs)）已 hash `resolved.endpoint/api_key/model + active_preset_id + preset.system_prompt`：
  - R2 改造后切换 preset / 修改 preset.provider_id / 修改 preset.model 都会让 `resolved.*` 三元组变 → hash 变 → 自动重建
  - **澄清**：「切 preset 触发重建」是**现状已有行为**（active_preset_id 已在 hash 中），不是 R2 引入的新特性；R2 只是让重建用上正确的 provider/key/model

### R3 — UI：LlmPage preset 卡片右侧徽章入口

#### R3.1 徽章

- 每个 preset 卡片右侧追加一个状态徽章
  - 默认（preset.provider_id 为空）：`[🔗 默认 GLM-4-Flash]` 灰色边框，显示**实际生效的模型名**
  - 覆盖中：`[🎯 GPT-4-turbo]` 蓝/紫品牌色
  - Provider 失效（指向的 provider 已被删除）：`[⚠️ 已失效，使用 GLM-4]` 红色
- 徽章本身**不加 ✕ 等动作按钮**（保持纯展示），动作集中在 popover 内

#### R3.1.1 Preset row 布局协调

- 当前 preset row（[LlmPage.tsx:50-92](../../../src/pages/LlmPage.tsx)）右侧已有 hover-only `Trash2` 删除按钮
- 加徽章后的 row 布局**自左向右**：`[icon] [preset.name (truncate)]  ←spacer→  [徽章 常驻]  [删除按钮 hover-only]`
- 徽章常驻显示，删除按钮保持 hover-only 显隐策略
- 徽章本身是可点击 `<button>`（点开 popover），与其他 preset 卡片的 `onClick={切 active}` 用 `e.stopPropagation()` 隔离
- 当 preset.name 较长时通过 `truncate` 让出空间给徽章，确保徽章不被挤出

#### R3.2 popover 状态机（事务边界）

- popover 用 **local state**（snapshot 自打开瞬间的 preset 值）；编辑过程中**不触全局** `setLlmConfig`
- 「确定」时单次 patch 全局 `llmConfig`（一次 setState → 一次 useEffect → 一次 update_runtime_config）
- 「取消」直接 close popover，全局 state 完全没动过
- 这是与全局 dirty save 隔离的**事务边界**，避免污染 useEffect 链触发误重建

#### R3.2.1 Commit 路径实现细节（绕过 helper）

- **不要扩展** `useLlmPresets.ts` 的 `handleUpdateActivePreset(key, value)` helper
  - 当前签名 `(key: keyof LlmPreset, value: string)` 不能携带 `undefined` 值（用于"清除覆盖"）
  - 新字段 `provider_id?: string | null` 类型与 `value: string` 不兼容
- popover 「确定」时**直接调** `setLlmConfig`，按 `id` 匹配做 spread 更新：
  ```typescript
  setLlmConfig(prev => ({
    ...prev,
    presets: prev.presets.map(p =>
      p.id === presetId ? { ...p, provider_id: stagedProviderId, model: stagedModel } : p
    ),
  }));
  ```
- 必须按 id 匹配 + spread map，**不要整体替换 presets 数组**——后端 `save_config` 有"presets 为空保留旧值"兜底（[lib.rs:788-797](../../../src-tauri/src/lib.rs)），但不能依赖兜底掩盖前端 bug

#### R3.3 popover 控件

- **Provider 下拉**：
  - 第一项 `[使用默认 ▼]`（值为 `null`）+ 全部 provider 列表
  - API key 为空的 provider **可选但下拉项尾缀「（缺 API Key）」标识**
- **Model 控件**（不提供下拉）：
  - 「☑ 继承 Provider 默认」复选框
  - TextInput（勾选时禁用并 placeholder 显示 provider.default_model；取消勾选时手动输入）
- **底部按钮**：`[取消] [确定]`
- 「确定」时若 model TextInput trim 后为空，提交 `model: undefined`（视为「继承」）
- **Invariant 强制**：当 staged `provider_id === null` 时，model 控件强制锁为「继承」状态（禁用 TextInput，确定时强制提交 `model: undefined`），从前端守护 state ④ 不出现

#### R3.4 失效态特殊 UX（边界防御）

> ⚠️ **方案 A 下的现状**：R5.3 已让删除 provider 时同步清空 preset.provider_id，正常路径**不会**产生失效 preset。本节仅作为**手工编辑 config.json 的边界防御**保留。

- 当 popover 打开时 staged `provider_id` 指向的 provider 不存在（仅手工编辑 config 才能触发）：
  - popover 顶部显示横幅条：
    ```
    ⚠️ Provider 不存在：abc123
    当前降级到「智谱AI」。请重新选择，或 [清除覆盖]
    ```
  - 「[清除覆盖]」点击后立即把 staged 状态置为 `{provider_id: null, model: null}` 并自动 commit + 关 popover（不需要再点确定）
- Provider 下拉中「使用默认 ▼」选项在失效态默认 highlight
- **不需要做** GlobalNoticeHost 横幅级别提示（方案 A 下失效路径稀有，popover 内联横幅已足够）

#### R3.5 inline warning（API key 空）

- popover 中部 inline 黄色文字提示，仅在选中 API key 为空的 provider 时显示：
  ```
  ⚠️ 该 Provider 缺少 API Key，保存后此预设将无法工作。
     请先到「模型」页配置。
  ```
- 不阻塞「确定」按钮（保留用户主权）；后端 R6.1 的 fail-safe 是最后一道防线

#### R3.6 输入处理

- Model TextInput onChange 时 trim 末尾空格（保留中间字符）
- 「确定」时若 trim 后为空，提交 `model: undefined`（视为「继承 Provider 默认」）
- 不做正则校验（中间空格、特殊字符等不主动拒绝）；服务端 400 报错时 trace 暴露

### R4 — 隐式门控

- 当 `shared.providers.length < 2` 时：**徽章默认完全不渲染**
- **历史例外**（兼容数据）：若任一 preset 已有非空 `provider_id`（来自历史 ≥2 providers 时的设置），LlmPage 中**仍渲染**该 preset 的徽章（通常为失效态红色）
- 这是为了让 95% 单 provider 用户的 UI 与现状完全一致，同时保证遗留数据可视

### R5 — UI：ModelsPage 双向可见

#### R5.1 "语句润色 · 词库增强"卡片下方提示行

- 当存在任意 preset 设置了非空 `provider_id` 时，卡片底部加一行：
  - `ⓘ N 个预设覆盖了此默认  [查看 →]`
- **可见性条件 = 「存在任意 preset.provider_id 非空」，不受 providers.length < 2 门控影响**（与 R4 LlmPage 历史例外对齐，保证双向可见性）
- 点击 `[查看 →]` 打开覆盖列表（popover/Drawer/Sheet 任选）：
  - 默认行：`默认（语句润色）：Provider A`
  - 覆盖行：`· 中译英 → Provider B (GPT-4)` `[↗ 跳转编辑]` （触发 `onNavigateToPreset(presetId, 'open-popover')`）
  - 跟随默认的 preset 在列表中以灰字呈现（"跟随默认"），不算覆盖

#### R5.2 Provider 卡片徽章组扩展

- 在现有"默认"金标 + feature 图标组旁（[ModelsPage.tsx:318-360](../../../src/pages/ModelsPage.tsx)），追加 preset 引用徽章
  - 1 个 preset 引用：`[🎯 中译英]`
  - 2 个 preset 引用：`[🎯 中译英] [🎯 邮件]`
  - ≥3 个：折叠为 `[🎯 +N]`，hover Tooltip 列全名（**复用 [Tooltip](../../../src/components/common/Tooltip.tsx) 组件，content 用 `\n` 分隔多行**）
  - 点击徽章触发 `onNavigateToPreset(presetId, 'open-popover')`

##### R5.2.1 交互密度协调（与现有徽章区分）

- 当前徽章组内的"默认"金标 + feature icon 都是 **`<span>` 非交互展示**
- 新增 `🎯` preset 徽章是**唯一可点击**元素，必须在视觉上明确区分：
  - DOM：用 `<button type="button">` 而非 `<span>`
  - 样式：加 `hover:bg-stone-200` / `cursor-pointer` / 微弱阴影
  - 必须 `e.stopPropagation()` 避免触发卡片其他 onClick 行为

#### R5.3 删除被引用 provider 的 confirm 扩展（方案 A：同步清空）

- 复用现有 `deleteConfirm` Modal（[ModelsPage.tsx:65-138, 570-610](../../../src/pages/ModelsPage.tsx)）
- 复用方式：在 `deleteConfirm` state 中新增 `referencedPresetNames: string[]` 字段（confirmDelete 触发前由 ModelsPage 派生计算），Modal 渲染时根据这个数组决定是否显示扩展文案
- 当被删 provider 在 preset.provider_id 中被引用时，confirm 文案扩展为：
  ```
  N 个预设引用此 Provider（中译英、邮件）。
  删除后这些预设的覆盖将被一并清空，回退到默认 Provider。
  仍要删除？  [取消] [删除]
  ```
- 用户确认删除后，confirmDelete 行为：
  1. 从 `providers` 数组移除被删 provider（现状）
  2. 清空指向该 ID 的 `polishing_provider_id` / `assistant_provider_id` / `learning_provider_id`（现状）
  3. **新增**：`presets.map(p => p.provider_id === id ? { ...p, provider_id: undefined, model: undefined } : p)`（同步清空 preset 层覆盖，与 shared 层行为一致）
- **关键决策**：与 shared 层删除清理策略保持一致，符合用户"删 provider = 切断所有引用"的直觉，避免悬挂引用

### R6 — 后端边界处理（防隐性 bug）

- **R6.1 API key 校验对 preset 覆盖感知**
  - 当前 [lib.rs:564](../../../src-tauri/src/lib.rs) 和 [lib.rs:3945](../../../src-tauri/src/lib.rs) 用 `resolve_polishing()` 拿到的 key 校验非空。R2 改造后这条天然变正确（因为 `resolve_polishing` 现在按 active preset 解析）
  - 验收点：用户为 preset 覆盖到一个 API key 为空的 provider → 切到该 preset → 校验失败 → 跳过 LlmPostProcessor 创建，trace warn

- **R6.2 失效检测（前端纯派生，作为边界防御）**
  - **不引入新 Tauri 事件**——失效检测完全是前端的 derived state
  - 方案 A 下正常路径不会出现失效（R5.3 已同步清空），但**保留**派生集合作为手工编辑 config 的边界防御
  - 前端 LlmPage useMemo：
    ```typescript
    const invalidPresetIds = useMemo(() =>
      llmConfig.presets
        .filter(p => p.provider_id && !llmConfig.shared.providers.find(x => x.id === p.provider_id))
        .map(p => p.id),
      [llmConfig.presets, llmConfig.shared.providers]
    );
    ```
  - 用途：R3.1 徽章红色态、R3.4 popover 顶部边界横幅
  - **不再用于** GlobalNoticeHost 横幅触发（R8.1 已砍）
  - 后端 `resolve_polishing` 在 fallback 时 `tracing::warn!("preset {id} 指向不存在的 provider {pid}，降级到 {fallback_id}")` 满足诊断需求
  - 不需要事件去重、payload 设计、契约维护

### R7 — 测试矩阵

#### Rust 单元测试（10 个，在 `config.rs` + `llm_post_processor.rs`）

| ID | 测试名 | 决策来源 |
|---|---|---|
| T1 | `test_preset_provider_override_resolves` | Q1 基础 |
| T2 | `test_preset_model_override_resolves` | Q1 优先级 |
| T3 | `test_preset_provider_missing_falls_back` | **边界防御**：手工编辑 config 写入悬挂 provider_id 时，`resolve_polishing` 仍能 fallback + trace warn（不 panic） |
| T4 | `test_compute_config_hash_changes_on_preset_switch_with_override` | hash 切换：preset A（覆盖 providerB）↔ preset B（默认）切换时 hash 变化（resolved.endpoint/api_key/model 变） |
| T5 | `test_compute_config_hash_stable_when_only_unrelated_field_changes` | **回归测试**：仅修改不在 hash 内的字段（如 preset.name）时 hash 不变。**不再断言"两个默认 preset 切换 hash 稳定"**——现状下 active_preset_id 已在 hash 内，切 preset 必然变 hash（这是现状行为，不是 R2 引入） |
| T6 | `test_resolve_skips_shared_polishing_model_when_preset_overrides_provider` | **Q1**: 跳过 polishing_model 核心契约 |
| T7 | `test_load_migration_9_cleans_state_invariant_violation` | **Q11**: state ④ load 自动清理 |
| T8 | `test_load_migration_9_emits_migrated_true` | **Q11**: 触发自动 save |
| T9 | `test_load_legacy_config_without_preset_fields_unchanged` | **升级兼容**: 老 JSON 加载零变化。Fixture 实施：用 inline JSON literal `r#"{...}"#` 喂 `serde_json::from_str::<AppConfig>(...)`，断言所有 `preset.provider_id == None && preset.model == None` |
| T10 | `test_save_preset_skips_serializing_none_fields` | **R1**: skip_serializing_if 生效。实施：构造 `LlmPreset { provider_id: None, model: None, .. }` → `serde_json::to_string` → 断言序列化结果**不包含** `"provider_id"` / `"model"` 子串 |

#### 前端单元测试（5 个，在 `tests/*.test.ts`）

| ID | 测试名 | 决策来源 |
|---|---|---|
| T11 | `popover_cancel_does_not_pollute_global_llmConfig` | **Q6**: 事务边界 |
| T12 | `popover_invariant_blocks_state_4` | **Q2**: 前端禁 state ④ |
| T13 | `model_input_trim_empty_to_undefined` | **Q12**: trim + 空 = 继承 |
| T14 | `derived_invalidation_set_includes_dangling_provider_id` | **Q4**: 前端纯派生失效集（仅边界防御场景，断言 useMemo 输出） |
| T15 | `gating_threshold_exception_when_legacy_provider_id_present` | **Q5**: < 2 providers 但有遗留覆盖时 UI 仍显示 |
| T16 | `delete_provider_clears_referencing_preset_overrides` | **方案 A 核心**：confirmDelete 后 `presets.find(p => p.provider_id === deletedId)` 为空，且对应 preset.model 也变 undefined |

#### 手工集成验证（4 项，方案 A 简化）

| 场景 | 决策来源 |
|---|---|
| 删除被引用 provider 弹 confirm 列出影响 preset，**确认后被引用 preset 的覆盖被同步清空，徽章变回默认态** | Q3 + 方案 A |
| Popover 顶部横幅在手工编辑 config 制造的失效 preset 上显示 + 「清除覆盖」立即生效（边界场景） | Q8 + R3.4 |
| 选 API key 为空的 provider 时 popover 显示黄色 inline warning | Q10 |
| ModelsPage 🎯 徽章点击跳 LlmPage 并 scroll + 高亮 + 自动展开 popover | Q13 |
| ~~切到失效 preset 出现 GlobalNoticeHost 横幅 + 「打开修复 →」~~ | ~~v3 砍：方案 A 下正常路径不存在失效~~ |

#### 不做 e2e（Playwright 等）

- 项目目前无 e2e 框架（`npm run test:ts` 实际是 **Node `node:test` runner via `tsx`**——见 [package.json:5](../../../package.json) `"test:ts": "tsx --test tests/*.test.ts"`，**非 Bun**）
- 跨页面跳转行为靠 T13 useEffect 单元测试 + 手工验证覆盖

### R8 — 跨页面跳转协议

> **方案 A 下的简化（v3 修订）**：R5.3 删除 provider 时同步清空 preset.provider_id 后，正常路径不会出现"激活的 preset 失效"场景，**砍掉 GlobalNoticeHost 失效横幅 + 通知层 action 扩展**。仅保留跨页面跳转协议（用于 ModelsPage 🎯 徽章 / ⓘ 提示行点击跳 LlmPage）。

#### R8.1 (已砍) GlobalNoticeHost 失效横幅

- 砍掉理由：方案 A 下正常路径不会有失效 preset；手工编辑 config 的边界场景靠 R3.4 popover 顶部横幅（用户主动打开 popover 时才看到）已足够，不必占用顶级通知层
- 通知层（`globalNotice.ts` / `NoticeCapsule.tsx` / `GlobalNoticeHost.tsx`）**保持现状不动**

<details>
<summary>原 R8.1 / R8.1.1 / R8.1.2 详细方案（保留作为方案 B 历史参考）</summary>

> ⚠️ **GlobalNoticeHost 现状不支持 action 按钮**：当前 `GlobalNoticePayload = { message, loading, tone }`（[globalNotice.ts:8](../../../src/utils/globalNotice.ts)）纯展示，host 用 `pointerEvents: "none"`（[GlobalNoticeHost.tsx:37](../../../src/components/notice/GlobalNoticeHost.tsx)）整体不接收点击，NoticeCapsule 也无按钮 slot。本节落地需要**先扩展通知层**才能渲染「打开修复 →」。

- 当 active_preset_id 切换到 provider_id 失效的 preset 时，前端通过 GlobalNoticeHost 显示横幅：
  ```
  ⚠️ 当前预设「翻译」的 Provider 已失效，正在使用降级 Provider「智谱AI」。  [打开修复 →]
  ```
- **生命周期 = 纯派生 state**：`shouldShowBanner = activePresetId 指向的 preset.provider_id ∈ shared.providers.map(p => p.id) === false`
- **不加 ✕ 关闭按钮**，无 sessionStorage / dismissed 持久状态
- 自动消失触发器：
  1. 用户切到非失效 preset → 派生重算 → 自动消失
  2. 用户在 popover 修复（选新 provider 或清除覆盖）→ 派生重算 → 自动消失
  3. providers 列表变化（用户重新创建配 + 手工去 popover「清除覆盖」后重新选）→ 派生重算 → 自动消失（注意：provider 重建 ID 必然不同，**不存在自动恢复**，必须手工触发）
- 「[打开修复 →]」点击触发 `navigateToPreset(activePresetId, 'open-popover')`

##### R8.1.1 通知层扩展（PR4 必做）

PR4 须先做以下基础改造，再实现 R8.1 横幅：

1. **扩展 `GlobalNoticePayload`**（[globalNotice.ts:8](../../../src/utils/globalNotice.ts)）：
   ```typescript
   export type GlobalNoticeAction = {
     label: string;          // 「打开修复 →」
     onClick: () => void;    // navigateToPreset(presetId, 'open-popover')
   };
   export type GlobalNoticePayload = {
     message: string;
     loading: boolean;
     tone: NoticeTone;
     action?: GlobalNoticeAction;   // 新增
   };
   ```

2. **扩展 `NoticeCapsule`**（[NoticeCapsule.tsx](../../../src/components/notice/NoticeCapsule.tsx)）：
   - 当 `payload.action` 存在时，在 message 右侧渲染 `<button>` 显示 `action.label`
   - 按钮颜色用当前 tone 的 color 反相 / 加边框，保证 affordance

3. **扩展 `GlobalNoticeHost`**（[GlobalNoticeHost.tsx:37](../../../src/components/notice/GlobalNoticeHost.tsx)）：
   - 外层 `pointerEvents: "none"` 保留（确保不挡下层）
   - 内层 capsule div 改为 `pointerEvents: "auto"`，让按钮可点
   - 按钮自身带 `e.stopPropagation()`

4. **扩展 `resolveGlobalNotice`**（[globalNotice.ts:27](../../../src/utils/globalNotice.ts)）：
   - 加入第 4 个优先级源：`presetInvalidation`（preset 失效）
   - 新参数：`presetInvalidation?: { presetName: string; fallbackProviderName: string; onFix: () => void } | null`
   - 优先级：放在 `syncStatus !== syncing` 之后（不打断保存进度），但**优先于** updateStatus（失效是更紧急的提示）
   - 输出 `{ message, loading: false, tone: "warning", action: { label: "打开修复 →", onClick: onFix } }`

5. **App.tsx 装配**：在调用 `resolveGlobalNotice` 时传入派生的 presetInvalidation（若 `invalidPresetIds.includes(activePresetId)` 则构造，否则 null）

##### R8.1.2 不在通知层做的事

- ❌ 不直接 `window.open` / 路由跳转（`onFix` callback 全权由 App.tsx 提供，保持通知层无业务耦合）
- ❌ 不持久 dismiss state（纯派生）
- ❌ 不加多个 action 按钮（单按钮够用，避免 capsule 横向膨胀）

</details>

#### R8.2 跨页面跳转协议（App.tsx pendingPresetFocus state）

```typescript
// App.tsx
const [pendingPresetFocus, setPendingPresetFocus] = useState<
  { presetId: string; action?: 'open-popover' } | null
>(null);

const navigateToPreset = (presetId: string, action?: 'open-popover') => {
  setPendingPresetFocus({ presetId, action });
  setActivePage('llm');
};

// 传给 ModelsPage：onNavigateToPreset={navigateToPreset}
// 传给 LlmPage：pendingFocus / onFocusConsumed
// （v3 修订后不再传给 GlobalNoticeHost）

// LlmPage useEffect 消费即清空
useEffect(() => {
  if (!pendingFocus) return;
  const node = presetRefs.current[pendingFocus.presetId];
  if (node) {
    node.scrollIntoView({ behavior: 'smooth', block: 'center' });
    if (pendingFocus.action === 'open-popover') {
      setOpenPopoverPresetId(pendingFocus.presetId);
    }
  }
  onFocusConsumed();
}, [pendingFocus]);
```

## Acceptance Criteria

### 功能正确性

- [ ] 单 provider 配置下，LlmPage 和 ModelsPage 视觉上与改造前**完全一致**（无新增元素）
- [ ] 配置 ≥2 个 provider 后，LlmPage preset 卡片出现徽章，显示当前实际使用的模型名
- [ ] 在徽章 popover 中给 preset 选定一个非默认 provider，保存后切换该 preset，**实际 ASR 后处理走新 provider**（trace 日志确认或抓包确认 endpoint）
- [ ] popover 点击"取消"后，前端 llmConfig 状态保持原值（不污染 dirty）
- [ ] popover 中 provider 选「使用默认 ▼」时，model TextInput **强制锁为「继承」状态**（前端 invariant 守护）
- [ ] popover 中选 API key 为空的 provider 时显示 yellow inline warning，但「确定」按钮仍可用
- [ ] ~~切换 active_preset 到失效 preset 时，**GlobalNoticeHost 顶部出现横幅**~~ —— v3 砍（方案 A 下正常路径无失效）
- [ ] Popover 打开时若 preset.provider_id 失效（**仅手工编辑 config 才能触发**），popover 顶部出现红色横幅含「清除覆盖」内嵌链接
- [ ] ModelsPage 在存在覆盖时，"语句润色"卡片下方出现 `ⓘ N 个预设覆盖了此默认`（不受 < 2 providers 门控）
- [ ] ModelsPage Provider 卡片在被 preset 引用时显示 `🎯 <preset name>` 徽章
- [ ] **删除被引用的 provider** 弹 confirm 列出受影响的 preset 名单 + 文案明示"覆盖将被清空"
- [ ] **方案 A 关键**：删除 provider 后，被引用 preset 的 `provider_id` 和 `model` **同步清空为 undefined**，徽章变回默认态（`[🔗 默认 GLM-4-Flash]`）
- [ ] 手工编辑 config.json 写入悬挂 provider_id 后，启动 app 不崩溃，`resolve_polishing` 走 fallback 链 + trace warn

### 升级兼容性

- [ ] 旧版（无 provider_id 字段）的 `config.json` 加载后，所有 preset 的 `provider_id` 为 `None`，行为与改造前完全一致
- [ ] 新版保存后的 `config.json` 中 `presets[].provider_id` 仅在用户显式设置时才出现（`skip_serializing_if`）
- [ ] 旧用户 `shared.polishing_model` 显式值在 preset **不覆盖 provider** 的情况下仍生效
- [ ] 手工编辑 config.json 出 state ④（model 有 / provider_id 无）时，load 时迁移 9 自动清理 model 字段并 trace warn
- [ ] 新 → 旧版本回滚（用户降级）时，新增的 preset.provider_id 字段被旧版本忽略，行为退化为「无覆盖」（不报错、不丢其他字段）

### 热生效

- [ ] 在 LlmPage 修改 preset 的 provider_id → 保存 → 后端 trace 日志出现 `LLM 处理器已重新初始化（配置变更）`
- [ ] 切换 active_preset_id 到另一个有覆盖的 preset → 同样触发重建（hash 因 resolved.endpoint 变化而变）

## Definition of Done

- 单元测试覆盖 R7 列举的 15 个用例（10 Rust + 5 TS），`cargo test` + `npm run test:ts` 全绿
- `cargo build --release` + `npm run tauri build` 成功（Windows NSIS 安装器产出）
- 手工验收覆盖上述 Acceptance Criteria 所有项 + R7 手工验证 5 项
- README 不强制更新（issue 关闭时在 issue 里贴出新功能截图即可）

## Out of Scope（明确排除）

- ❌ **诉求 B：AI 助手 Q&A vs 文本处理拆分独立 LLM**——后续单独立任务
- ❌ 在 ModelsPage 把每个 preset 单独列卡片（会让列表无限膨胀）
- ❌ 在 ModelsPage 直接编辑 preset 的 provider（避免双入口编辑同一字段引发的状态同步坑）
- ❌ "高级模式"显式开关（用 provider 数量做隐式门控，无显式 toggle）
- ❌ Preset 的 `provider_id / model` 字段加 patch_config_fields 路由（不需要，走 save_config 整体保存即可）
- ❌ AssistantConfig 任何改动
- ❌ **后端 Tauri 事件契约**（`preset_provider_invalidated` 等）——失效检测前端纯派生
- ❌ Provider 数据结构新增 `available_models: Vec<String>` 字段——model 输入用 TextInput 而非下拉
- ❌ Popover 加 ✕ / 24h 静音等持久 dismissed 状态——失效横幅纯派生
- ❌ React Router 引入——保持现有 `useState<AppPage>` + callback 透传模式
- ❌ E2E 测试框架引入——靠单元测试 + 手工验证覆盖

## Decision (ADR-lite)

### Context

Issue #12 用户 @xiong9151 反馈：润色多个预设共用一个模型不灵活——翻译可以用便宜模型而润色需要强模型。但 95% 用户只配单 provider，对此功能无感知。

### 16 项关键决策（grill-me 闭环）

| # | 决策点 | 摘要 |
|---|---|---|
| Q1 | model resolve 链 | preset 覆盖 provider 时**跳过** `shared.polishing_model`（避免徽章文案 vs 实际行为错位） |
| Q2 | provider_id/model 耦合 | **state ④ banned**：`model.is_some() ⇒ provider_id.is_some()` |
| Q3 | 删除 provider cascade | **方案 A**（v3 修订）：confirm 弹窗 + 同步清空 preset.provider_id（与 shared 层一致） |
| Q4 | 失效检测 | **前端纯派生**，不引入后端事件契约 |
| Q5 | 门控阈值历史例外 | LlmPage 与 ModelsPage **双向可见**，不受 < 2 providers 影响 |
| Q6 | popover 状态机 | local state 事务边界，确定时一次性 commit |
| Q7 | model 输入控件 | 继承复选框 + TextInput，**不做下拉** |
| Q8 | 失效态 popover UX | 顶部横幅 + 「清除覆盖」内嵌链接 |
| Q9 | 失效 preset 激活提示 | ~~GlobalNoticeHost 横幅~~（v3 砍，方案 A 下正常路径不会有失效） |
| Q10 | 空 API key provider | 下拉可选 + inline yellow warning（不阻断） |
| Q11 | state ④ 清理时机 | load 时**迁移 9** 自动清理 + trace warn |
| Q12 | model 输入校验 | onChange trim 末尾，空 = 继承 |
| Q13 | 跨页面跳转协议 | App.tsx `pendingPresetFocus` state + callback 透传 |
| Q14 | 横幅生命周期 | 无 ✕，纯 useMemo 派生 |
| Q15 | 测试矩阵 | T1-T16（11 Rust + 5 TS）+ 4 项手工验证（v3 加 T16，砍 1 项手工） |
| Q16 | PR 分批 | 4 PRs：后端 / LlmPage 核心 / ModelsPage（含 confirmDelete 同步清空） / 跨页面跳转（v3 砍通知层改造） |

### Decision Highlights

1. **粒度下沉**：模型绑定从 feature 级新增 preset 级覆盖能力，**作为可选层**，feature 级仍是默认
2. **隐式门控 + 历史例外**：`shared.providers.length < 2` 时所有相关 UI 元素默认隐藏；但若有遗留 `provider_id` 仍显示徽章/提示行（Q5 双向可见原则）
3. **配置就近**：preset 模型选择放在 LlmPage 的 preset 卡片内（不在 ModelsPage 列预设）
4. **统一收口**：ModelsPage 仍作为"模型分配总览"，通过 ⓘ 提示行 + Provider 卡片 🎯 徽章双向反映 preset 覆盖关系
5. **零事件契约**：失效检测完全是前端 derived state，**不引入** `preset_provider_invalidated` 之类的 Tauri 事件
6. **零 migration 风险**：新字段为 `Option`，旧配置 serde 默认 `None`；新增**迁移 9** 仅清理理论上不应存在的 state ④
7. **删除时同步清空（方案 A，v3 修订）**：删除 provider 时同步清空 `preset.provider_id` 和 `preset.model`，与 shared 层（polishing_provider_id 等）行为一致，符合"删 = 切断引用"的用户直觉。失效检测代码作为手工编辑 config 的边界防御保留（R6.2 useMemo + R3.4 popover 顶部横幅 + 后端 trace warn），但**砍掉 GlobalNoticeHost 横幅**（正常路径不会触发）

### Consequences

- ✅ 非破坏性变更，旧用户升级无感知，新→旧回滚也无破坏
- ✅ 新功能对功能不需求的用户零打扰
- ✅ 后端复用现有 `compute_config_hash` 自动检测变化机制（前提是 R2 实施正确）
- ✅ in-flight LLM 请求隔离（lock + clone + unlock 模式）
- ⚠️ Popover 状态管理用 local state（Q6 事务边界），不能让 popover 编辑直接 setLlmConfig，否则触发 useEffect 误重建
- ⚠️ ModelsPage 的"覆盖列表" Drawer 需要新组件，是本次最大的前端工作量
- ⚠️ 跨页面跳转 `pendingPresetFocus` state 需要在 App.tsx + LlmPage + ModelsPage 串联（PR4 集中装配；v3 修订后 GlobalNoticeHost 不再涉及）
- ✅ **方案 A** 让 shared 层和 preset 层删除清理策略统一（都是"删除 = 切断引用"），符合用户直觉
- ⚠️ **方案 A 的代价**：用户误删 provider 后无法撤销 — 必须重新到 LlmPage 给每个 preset 重新选 provider/model（与现状 shared 层删除行为一致，可接受）
- ⚠️ 失效检测代码（R6.2 useMemo + R3.4 popover 顶部横幅）保留作为手工编辑 config 的边界防御，正常路径不触发

## Technical Approach

### 实施顺序（4 PR，方案 B 风险隔离）

#### PR1：后端基础

- `LlmPreset` 加 `provider_id` / `model` 字段（含 `skip_serializing_if`）
- `resolve_polishing` / `resolve_with_feature` 接入 preset 覆盖（含 Q1 跳过 polishing_model 规则）
- `LlmPostProcessor::compute_config_hash` 自动反映 preset 覆盖（验证已 hash `resolved.*`，无需改动）
- 新增**迁移 9**（清理 state ④）+ 写入 `migrated=true`
- 单元测试 T1-T10
- 手工验证：通过 `config.json` 直接编辑写入 `provider_id`，启动后 trace 日志确认 resolve 走新 provider

#### PR2：前端 LlmPage 核心

- `LlmPreset` interface 加字段
- preset 卡片右侧加徽章（R3.1）
- popover：事务边界 local state（R3.2）+ 控件（R3.3）+ 失效顶部横幅（R3.4）+ inline warning（R3.5）+ 输入处理（R3.6）
- 失效检测前端纯派生 useMemo（R6.2）
- 隐式门控 + 历史例外（R4）
- 单元测试 T11-T14
- **不含**跨页面跳转 props 接收（PR4 加）

#### PR3：ModelsPage 双向反映

- "语句润色"卡片下方 ⓘ 提示行 + 覆盖列表 popover/Drawer（R5.1，含历史例外）
- Provider 卡片 🎯 徽章组（R5.2）
- 删除被引用 provider 的 confirm 扩展（R5.3，复用现有 deleteConfirm Modal）
- 单元测试 T15
- **不含**跳转 callback（PR4 加）

#### PR4：跨页面装配（方案 A 下大幅缩窄）

- App.tsx 加 `pendingPresetFocus` state + `navigateToPreset` callback（R8.2）
- LlmPage 接收 `pendingFocus` props + useEffect 消费即清空
- ModelsPage 接收 `onNavigateToPreset` callback，串联到 R5.1 「[↗ 跳转编辑]」+ R5.2 「🎯」徽章点击
- 手工验证 R7 列出的剩余项（GlobalNoticeHost 横幅相关验证项已砍）
- ⚠️ **不再做**：通知层扩展（GlobalNoticePayload action / NoticeCapsule 按钮 / GlobalNoticeHost pointerEvents / resolveGlobalNotice 第 4 源）—— v3 修订砍掉

### 关键代码位点

| 文件 | 位置 | 改动类型 | PR |
|---|---|---|---|
| `src-tauri/src/config.rs` | LlmPreset 结构 (689-693) | 新增 2 字段 + skip_serializing_if | PR1 |
| `src-tauri/src/config.rs` | LlmConfig::resolve_polishing (979-983) | **仅在此处加 preset 短路逻辑**（不动 resolve_with_feature） | PR1 |
| `src-tauri/src/config.rs` | LlmFeatureConfig::resolve_with_feature (862-950) | **不改签名**（被 polishing/assistant/learning 共用） | - |
| `src-tauri/src/config.rs` | AppConfig::load 迁移段 (~1700) | 新增迁移 9（清理 state ④） | PR1 |
| `src-tauri/src/llm_post_processor.rs` | tests (500+) | 加 T1-T10 测试 | PR1 |
| `src/types/index.ts` | LlmPreset (~58-60) | 新增 2 字段 | PR2 |
| `src/pages/LlmPage.tsx` | preset 卡片渲染 (50-92) | 加徽章 + popover + useMemo 失效集（含 R3.1.1 row layout 协调） | PR2 |
| `src/pages/LlmPage.tsx` | useEffect 消费 pendingFocus | 跨页面跳转接收 | PR4 |
| `src/pages/ModelsPage.tsx` | "语句润色"卡片 + provider 卡片 | 加 ⓘ 提示行 + 🎯 徽章 | PR3 |
| `src/pages/ModelsPage.tsx` | deleteConfirm state + Modal (65-138, 570-610) | state 加 referencedPresetNames + Modal 扩展文案 + **confirmDelete 同步清空 preset.provider_id** | PR3 |
| `src/pages/ModelsPage.tsx` | 🎯 徽章 + 跳转编辑 | 接 onNavigateToPreset callback | PR4 |
| `src/App.tsx` | pendingPresetFocus state + navigateToPreset | 跨页面装配 | PR4 |
| `src/hooks/useLlmPresets.ts` | 无需改 | （新建 preset 默认 undefined；popover commit 直接走 setLlmConfig 不经此 helper） | - |
| ~~`src/utils/globalNotice.ts`~~ | ~~GlobalNoticePayload + resolveGlobalNotice~~ | ~~v3 修订砍：方案 A 下不需要通知层扩展~~ | ~~PR4~~ |
| ~~`src/components/notice/NoticeCapsule.tsx`~~ | ~~按钮 slot~~ | ~~v3 修订砍~~ | ~~PR4~~ |
| ~~`src/components/notice/GlobalNoticeHost.tsx`~~ | ~~pointerEvents~~ | ~~v3 修订砍~~ | ~~PR4~~ |
| ~~`tests/globalNotice.test.ts`~~ | ~~action 字段回归~~ | ~~v3 修订砍~~ | ~~PR4~~ |
| `tests/*.test.ts` | T11-T15 | 前端单元测试 | PR2/PR3 |

## Technical Notes

### GitNexus 影响分析（已跑）

- `resolve_polishing` upstream impact = HIGH (8 symbols, 3 processes, 4 modules)
- 直接调用方共 6 处：`refresh_post_processor_after_toggle`、`start_app`、`update_runtime_config`、`switch_asr_provider_from_tray_inner`、以及 `llm_post_processor.rs::new` / `compute_config_hash`
- `LlmPreset` upstream impact = CRITICAL (46 importers) 但都是 IMPORTS 关系，加 Option 字段不破坏任何 importer
- 关键 process：`save_config (proc_109)` 3 步链 / `applyRuntimeConfig (proc_134/135)` 是热生效必经路径
- `AppConfig::load` 8 步迁移逻辑已通读（[config.rs:1239-1714](../../../src-tauri/src/config.rs)）；新增**迁移 9** 不影响前 8 步语义
- ⚠️ `LlmFeatureConfig::resolve_with_feature` 被 polishing/assistant/learning **3 个 feature 共用**（`config.rs:684, 1179, 981`）——R2 的改造**必须**约束在 `LlmConfig::resolve_polishing` 这一层，**不能修改 `resolve_with_feature` 签名**，否则会污染 assistant/learning 路径

### 已确认无需改动的"隐藏面"

- `patch_config_fields` 是 5 字段白名单，不接 LlmConfig
- 前端 `lastAppliedConfigHashRef` 用 JSON.stringify deep diff，新字段自动纳入
- 后端无任何"托盘/外部"切 preset 入口（grep 全文确认），单一保存路径
- `save_persisted_config_without_emit` 两处调用都不动 LlmConfig
- `refresh_post_processor_after_toggle` 只在懒加载时执行，不在运行中切 preset 路径
- `LlmPostProcessor` 的 `Arc<Mutex<Option<LlmPostProcessor>>>` + `lock + clone + unlock` 模式确保**重建对正在 in-flight 的请求安全隔离**（[lib.rs:3358](../../../src-tauri/src/lib.rs)）
- `AppConfig::load()` 返回 `(Self, bool)` 中 `migrated=true` 时由 `load_persisted_config` 自动 save（[lib.rs:307-312](../../../src-tauri/src/lib.rs)），R1 加 Option 字段不会让 serde 反序列化失败因此**不会触发额外写盘**

### 已知约束

- 项目专为 Windows，可放心使用 Win32 API；无需考虑跨平台
- 配置文件路径：`%APPDATA%\PushToTalk\config.json`
- 单实例桌面应用，无版本向后兼容需求
- 默认 LLM 模型 fallback：`glm-4-flash-250414` ([config.rs:1130](../../../src-tauri/src/config.rs))
- 项目无 React Router，App.tsx 路由是 `useState<AppPage>` + callback 透传

### 关联资源

- Issue: https://github.com/yyyzl/push-2-talk/issues/12
- 现有 LlmConfig schema: `src-tauri/src/config.rs:962-983`
- 现有 ModelsPage 双向可见机制: `src/pages/ModelsPage.tsx:232-360`
- 现有 deleteConfirm Modal: `src/pages/ModelsPage.tsx:65-138, 570-610`
- 现有通知层: `src/utils/globalNotice.ts` + `src/components/notice/{GlobalNoticeHost,NoticeCapsule}.tsx`
- 现有 Tooltip 组件: `src/components/common/Tooltip.tsx`（接受 `content: string`，支持 `\n` 多行）
- 热生效链路文档化已在 `CLAUDE.md` 的 "Critical Event Flow" 段落

---

## Decision Pending（2026-05-09 修订引入）

### ✅ DP1（已解决，2026-05-09 by yyyzl）：选择 **方案 A — 删除 provider 时同步清空 preset.provider_id**

**最终决策**：方案 A。删除 provider 时同步清空所有引用此 provider 的 preset 的 `provider_id` 和 `model` 字段，与 shared 层行为一致。

**采纳理由**（同 v2 修订时的推荐）：
1. P2 已证实"自动恢复"是空中楼阁，方案 B 软引用的核心收益（误删恢复）**完全不成立**
2. 行为一致性收益大：用户对"删除 = 切断引用"的直觉不被破坏
3. 工作量显著降低
4. 失效检测代码（R6.2 useMemo）仍保留作为手工编辑 config 的边界防御

**已落地的 v3 裁剪**（详见 Revision Log）：
- R5.3 confirm 文案改为"将清空 N 个预设的覆盖"+ confirmDelete 加同步清空逻辑
- R8.1 GlobalNoticeHost 横幅 → 砍（折叠为 details 段保留方案 B 历史参考）
- R8.1.1 通知层 action 扩展 → 砍
- R3.4 popover 顶部红色横幅 → 保留作为手工编辑 config 边界防御
- R6.2 失效检测 useMemo → 保留作为边界防御
- 关键代码位点表通知层 4 个文件 → 划线标注砍
- T3 测试改写为边界防御断言；T16 新增方案 A 核心断言
- 手工集成验证从 5 项简化为 4 项（GlobalNoticeHost 验证项砍）
- Acceptance Criteria 调整失效相关项

---

## Revision Log

### 2026-05-09 — Independent Review 修订（v2）

针对 `bmad-review` 模式的代码核对发现的 12 个问题进行修订。**实施前必读**。

**🚨 硬伤修复（3 项，已落地）**

- **P1 GlobalNoticeHost 不支持 action 按钮** → 新增 R8.1.1 通知层扩展子节，明确 PR4 必做的 4 处改造（GlobalNoticePayload / NoticeCapsule / GlobalNoticeHost / resolveGlobalNotice）；关键代码位点表新增 4 个文件
- **P2 "同 SHA256 ID 恢复"是空中楼阁** → R5.3 / Acceptance Criteria / Decision Highlights / Consequences / R8.1 全部删除"自动恢复"承诺；Provider ID 实际是 `randomUUID().substring(0, 12)`，与内容无关
- **P3 改 `resolve_with_feature` 会污染通用方法** → R2 改写实施模式，限定改造范围在 `LlmConfig::resolve_polishing()` 内；关键代码位点表显式标注 `resolve_with_feature` **不改签名**；Technical Notes 加警示

**⚠️ 中等问题修复（5 项，已落地）**

- **P4 T5 测试假设错误** → T5 重写为"仅修改不在 hash 内字段时 hash 不变"回归测试；R2 段补充澄清"切 preset 触发重建是现状已有行为"
- **P5 Preset row 布局** → 新增 R3.1.1 子节明确 row 自左向右布局：icon / name / spacer / 徽章常驻 / 删除按钮 hover-only
- **P6 删除 provider 时清理策略不一致** → 在 R5.3 顶部加 ⚠️ 待决策提示；末尾增加 Decision Pending DP1 段，列出方案 A/B 对比 + 推荐方案 A
- **P7 测试基础设施事实错误** → R7 修正"Bun runtime"→"Node node:test runner via tsx"
- **P8 handleUpdateActivePreset helper 不能管新字段** → 新增 R3.2.1 子节明确 popover commit 直接走 `setLlmConfig`（按 id map + spread），绕过 helper

**💡 小提醒（4 项，已落地或注释）**

- **P9 save_config presets 兜底** → R3.2.1 中提及，强调不可整体替换 presets 数组
- **P10 T9 fixture 实施** → T9/T10 测试条目补充 inline JSON literal + serde_json 实施细节
- **P11 R5.2 按钮 vs 展示元素** → 新增 R5.2.1 子节，明确 `<button>` vs `<span>` 样式分离 + e.stopPropagation
- **P12 Tooltip 组件已存在** → R5.2 显式标注复用 `src/components/common/Tooltip.tsx`

**待用户拍板**

- ~~DP1（P6）：方案 A vs 方案 B~~ → ✅ **2026-05-09 决策选 A，已在 v3 修订中落地**

---

### 2026-05-10 — UX 大重构：Inline 模型选择（v4）

#### 触发原因

dev 模式自测发现严重 UX 问题：
- "LLM 连接配置"卡片的「更换」按钮跳转到 ModelsPage tab，徽章点击却弹 popover —— **同一面板里两个看似一样的入口行为完全不同**
- 徽章 popover 在测试环境下用户感觉"没有保存按钮"（实际有但被忽略）
- 用户视角整体感受："非常的混乱"

#### 根因复盘

PRD v1-v3 把后端的"覆盖（override）"概念**直接暴露给 UX**：
- 4 种入口（徽章 / 连接配置 / ModelsPage 卡片 / Provider 列表）
- 3 种态（默认 / 覆盖 / 失效）
- 隐式门控、事务边界 popover、state ④ 守护、迁移 9、失效横幅……一堆为了支撑"覆盖"心智模型而生的复杂度

但用户的真实心智只有：「**这个预设用哪个模型**」—— 一个普通的属性，不需要"覆盖"概念。

PRD v1-v3 grill-me 的 16 个决策都在为"覆盖+徽章"这个错误前提服务，没有人质疑前提本身。Independent review 也只审视了实现细节，未质疑核心比喻。

#### v4 核心思路：Inline 模型选择

**把"模型选择"做成预设的一个普通属性**，与 name / system_prompt 平级摊在 preset 编辑面板里：

```
┌─ 文本润色 ────────────────────┐
│ 预设名称 [文本润色      ]     │
│ System Prompt [...]           │
│ ──────────────────────────    │
│ LLM 模型                      │
│ [ ▼ 跟随润色默认（智谱 GLM-4） ] │  ← 下拉
│ ⚙️ 在「LLM 模型配置」改默认 →  │
│                               │
│ [选了具体 provider 后展开]    │
│ 模型 ID（可选，留空用默认）   │
│ [glm-4-flash       ]          │
└───────────────────────────────┘
```

#### 砍掉的（v3 引入但 v4 移除）

| 内容 | 理由 |
|---|---|
| **徽章组件 + 三态视觉** | 没了"覆盖"概念，徽章无意义 |
| **PresetOverrideEditor popover + 事务边界** | inline 编辑直接 commit，无需事务 |
| **R3.4 失效态特殊横幅** | 简化为下拉里的 disabled 选项「(已删除) <id>」 |
| **R4 隐式门控（< 2 providers 隐藏）** | 下拉只有一个选项时用户不会主动操作 = 自然门控 |
| **R5.2.1 按钮 vs span 样式分离** | ModelsPage 的 🎯 引用徽章保留（仍是反向显示），但 LlmPage 内徽章全删 |
| **LlmPage 右下「LLM 连接配置」section** | polishing 默认改在 ModelsPage 唯一管理；通过组件内的「⚙️ 管理 Provider」链接跳转 |
| **R8.2 navigateToPreset 的 'open-popover' action** | 没了 popover，跳转 = scroll + 激活 preset |
| **shouldShowPresetBadges 函数 + T15 测试** | 不需要门控判断 |
| **T11 popover_cancel 测试** | 无 popover |
| **T12 popover_invariant 测试** | 改为 inline `computeOverrideCommit` 的 invariant 单测 |

#### 保留的（v3 已落地继续可用）

- ✅ **PR1 后端全部**：LlmPreset 加 provider_id/model 字段、resolve_polishing 短路、迁移 9、T1-T10 测试 — 后端契约完全不变
- ✅ **方案 A cascade**：ModelsPage delete confirm 同步清空 preset 覆盖（T16 测试）
- ✅ **ModelsPage 双向反映**：「N 个预设使用了独立模型」提示行 + Provider 卡片 🎯 引用徽章组（文案改"独立模型"，去掉"覆盖"术语）
- ✅ **PR4 跨页面跳转**：pendingPresetFocus state（简化无 action 字段）
- ✅ **utility 函数**：deriveInvalidPresetIds / clearPresetOverridesForProvider / computeOverrideCommit（签名简化为 2 参数）

#### 新增（v4）

- ✨ **`PresetModelSelect.tsx`** 组件（约 120 行）— 下拉 + 可选 model ID input + dangling 提示
- ✨ **`resolvePresetEffectiveModel`** util（替代旧 `resolvePresetBadge`）— 三态返回 `inherit / direct / dangling`
- ✨ **`buildModelSelectOptions`** util — 生成下拉选项（含失效项作为 disabled）
- ✨ **新测试**：
  - 5 个 `resolvePresetEffectiveModel` 测试（三态 + polishing_model 优先级 + 默认 fallback）
  - 3 个 `buildModelSelectOptions` 测试（label 内容 + 缺 API key 标识 + dangling disabled 选项）
  - 1 个 invariant 测试（provider_id=null 强制 model=undefined）

#### 净代码变化

| 文件 | 行数变化 |
|---|---|
| `LlmPage.tsx` | -100 行（删徽章 + popover state + LlmConnectionConfig section） |
| `PresetOverrideEditor.tsx` | -210 行（整个文件删除） |
| `presetOverride.ts` | +30 行（新增 buildModelSelectOptions 等） |
| `PresetModelSelect.tsx` | +120 行（新组件） |
| `tests/presetOverride.test.ts` | -50 行（删 T11/T12/T15 + popover 相关）+ 80 行（新增 resolveEffective 测试） |
| **净** | **约 -130 行** |

测试结果：81 / 81 通过（FE 14 个 v4 + Rust 11 个 PR1 后端契约不变）。

#### 三个 v4 设计选择（已用户确认 A/A/A）

1. ✅ "跟随默认"文案显示当前实际值：`跟随润色默认（智谱 · GLM-4-Flash）`
2. ✅ 失效态在下拉里展示 disabled 选项 `(已删除) prov-xyz`，不悄悄改用户数据
3. ✅ "自定义模型 ID" input 选了 provider 后**始终展开**，无折叠

---

### 2026-05-09 — 方案 A 落地裁剪（v3）

用户拍板 DP1 选方案 A 后做的二次裁剪。

**砍掉的内容**

- R8.1 GlobalNoticeHost 失效横幅 → 砍（折叠为 details 段保留方案 B 历史参考）
- R8.1.1 通知层扩展（GlobalNoticePayload action / NoticeCapsule 按钮 / GlobalNoticeHost pointerEvents / resolveGlobalNotice 第 4 源） → 砍
- 关键代码位点表通知层 4 个文件（globalNotice.ts / NoticeCapsule.tsx / GlobalNoticeHost.tsx / globalNotice.test.ts） → 划线标注
- 手工集成验证「切到失效 preset 出现 GlobalNoticeHost 横幅」 → 砍
- Acceptance Criteria「切换 active_preset 到失效 preset 时 GlobalNoticeHost 顶部出现横幅」 → 砍

**改写的内容**

- R5.3 confirm 文案大幅简化（不再需要"悬挂引用"等专业术语）+ confirmDelete 加同步清空 `presets.map(...)` 一行
- R3.4 标题加「（边界防御）」副词，明确仅手工编辑 config 才会触发
- R6.2 用途说明改写：派生集合不再用于 GlobalNoticeHost 触发，仅供 R3.1 红色徽章和 R3.4 popover 横幅
- PR4 范围大幅缩小（不再含通知层改造）
- Decision Highlights 第 7 条重写为方案 A 描述
- Consequences 软引用条目重写为方案 A trade-off
- T3 测试目的从"软引用降级"改为"手工编辑边界防御断言"
- 关键代码位点 ModelsPage deleteConfirm 行加注「+ confirmDelete 同步清空 preset.provider_id」

**新增的内容**

- T16 测试：`delete_provider_clears_referencing_preset_overrides`（方案 A 核心断言）
- Acceptance Criteria 新增 2 项：方案 A 同步清空验证 + 手工编辑 config 边界防御验证
- Consequences 新增方案 A 一致性收益描述
- DP1 标记为已解决
