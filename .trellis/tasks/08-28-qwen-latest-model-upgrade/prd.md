# 千问最新 ASR 模型升级策略

## 目标

评估并接入千问最新 ASR 模型，在识别质量、实时性、热词能力、兼容性与成本之间取得合理平衡，并决定是直接替换当前底层默认模型，还是提供模型选择能力。

## 已知事实

- 用户确认“最新千问模型”指最新 ASR 模型，不是 AI 助手或后处理 LLM。
- 当前项目同时存在 HTTP 与实时 ASR 链路，升级策略需要分别核对协议与功能兼容性。
- 项目面向 Windows 10/11，需保留现有全局热键、流式转写和配置迁移行为。
- 已按方案 A 完成产品代码、配置迁移、双协议适配、设置页入口和回归测试。
- 当前 HTTP 固定使用 `qwen3-asr-flash`，实时固定使用 `qwen3-asr-flash-realtime`。
- 当前配置只有 Qwen provider，没有 Qwen model/profile 字段。
- 官方最新系列为 `qwen-audio-3.0-asr-flash` 与 `qwen-audio-3.0-asr-flash-streaming`，于 2026-07-30 上线。
- 新旧模型官方原价一致；新模型新增即时热词、Context、多方言和时间戳能力。
- 新模型的 HTTP 消息结构和实时 WebSocket 协议均不同于当前实现，不能只替换模型常量。

## 临时假设

- 最新模型可通过现有地域对应的千问/DashScope 凭据访问；workspace 专属域名迁移可后置。
- 用户更关注默认体验升级，而不是在设置页暴露大量底层型号。
- 旧模型可能仍有回退价值，尤其是在新模型区域可用性、实时协议或成本不同的情况下。

## 待确认问题

- 无。

## 需求（演进中）

- 查明当前 Qwen ASR 型号、端点和配置入口。
- 以千问官方资料确认最新 ASR 型号、协议、地域、价格和功能差异。
- 给出直接替换、受控多选和内部回退三种方案的成本与风险对比。
- 不把 Qwen ASR 升级与助手 LLM 模型升级混在同一任务中。
- 将 HTTP 与实时型号绑定为同一模型系列/profile，避免协议与型号错配。
- 保留一个可验证、可回滚的 Qwen3 兼容路径。
- 新模型复用现有热词候选编译结果，并映射为官方带权重即时热词参数。

## 验收标准（演进中）

- [x] 明确当前 HTTP 与实时链路使用的模型。
- [x] 明确官方最新 ASR 模型及其兼容限制。
- [x] 形成可执行的 MVP 决策与回滚策略。
- [x] Qwen Audio 3.0 profile 分别调用 `qwen-audio-3.0-asr-flash` 和 `qwen-audio-3.0-asr-flash-streaming`。
- [x] Qwen3 legacy profile 保持当前 HTTP 与 realtime 行为。
- [x] 旧配置缺少 profile 时默认迁移到 Qwen Audio 3.0。
- [x] ASR 设置页提供“最新版（推荐）/Qwen3 兼容版”受控选择，不允许任意型号文本。
- [x] HTTP 新协议正确发送音频、语言提示和带权重即时热词。
- [x] 实时新协议正确完成 `run-task → binary audio → finish-task → result` 生命周期。
- [x] 切换 profile 后客户端按新配置重建，provider/fallback、词典和纠错词对行为不回退。
- [x] Rust 单元测试、TypeScript 运行时测试、前端构建和 `cargo check` 通过。

## 完成定义

- 相关测试新增或更新。
- 前端构建、TypeScript 测试、Rust 相关测试和 `cargo check` 通过。
- 默认值、配置迁移和文档保持前后端一致。
- 明确发布与回滚方式。

## 暂不包含

- 升级 AI 助手或文本后处理 LLM。
- 重构全部 ASR provider 架构。
- 在没有实际价值时向普通用户暴露所有供应商内部型号。

## 技术记录

- 规划任务：`.trellis/tasks/08-28-qwen-latest-model-upgrade/`
- GitNexus 索引已刷新到当前 `main`。
- [`research/local-qwen-asr-paths.md`](research/local-qwen-asr-paths.md) 记录本地 HTTP、实时、配置和 UI 链路。
- [`research/official-qwen-audio-3.md`](research/official-qwen-audio-3.md) 记录官方最新型号、协议、能力、价格与地域信息。

## 可行方案

### 方案 A：新模型默认启用，保留兼容回退（推荐）

- 新增受控的 Qwen ASR profile：`qwen_audio_3` 与 `qwen3_legacy`。
- 新安装默认使用 Qwen Audio 3.0；旧配置迁移到新默认，遇到明确兼容问题时可切回 Qwen3。
- 普通设置页不展示任意型号列表；兼容开关可放在高级设置，或首版只保留内部配置/自动回退。

优点：默认体验升级，同时有回滚手段；不会把供应商内部型号复杂度全部暴露给用户。
缺点：需要同时维护两套协议适配，直到旧链路退役。

### 方案 B：直接替换并删除旧链路

- HTTP 和实时客户端直接迁移到 Qwen Audio 3.0，不保留 Qwen3。

优点：代码最终最简洁。
缺点：上线即全量切换，真实听写效果或地域兼容出现问题时只能发新版本回滚。

### 方案 C：完整型号选择器

- 在 ASR 设置页展示多个千问模型/快照版本，并把 model、endpoint、protocol、capabilities 配置化。

优点：高级用户自由度最高，未来接更多型号方便。
缺点：当前只有两套有价值的 profile，UI、迁移、校验与测试成本明显偏高，还容易产生协议不匹配状态。

## 推荐决策

优先采用方案 A。产品层面是“默认升级”，工程层面是“可回滚的双 profile”；不建议首版做任意模型多选，也不建议无回退地硬替换。

## 决策（ADR-lite）

**背景**：用户希望立即使用最新 Qwen ASR，同时项目已发布多个版本，需要避免新协议在真实环境出现问题时只能重新发版回滚。

**决策**：采用方案 A。新增受控 profile，`qwen_audio_3` 为默认，`qwen3_legacy` 为兼容选项；缺少该字段的旧配置自动采用 `qwen_audio_3`。HTTP 与实时型号由 profile 成对映射，不开放任意型号输入。

**后果**：首版需要维护两套 Qwen 协议适配，但普通用户默认获得最新版能力，并能在高级设置中一键回退。暂不做请求级自动双重调用，也暂不引入 workspace ID 配置。

## 实现与验证结果

- 后端与前端统一新增 `qwen_audio_3` / `qwen3_legacy` profile；缺失或未知值归一化到最新版。
- 修复 Serde 数字边界命名：`QwenAudio3` 显式使用 `qwen_audio_3`，并兼容读取修复前可能落盘的 `qwen_audio3`。
- HTTP 最新链路使用多模态请求和即时热词 `vocabulary`；实时最新链路使用 DashScope `run-task` 协议与二进制 PCM 音频帧。
- 设置页展示受控双选项和实际 HTTP/实时模型名；切换后沿用现有立即保存与客户端重建机制。
- `npm run test:ts`：145 项通过。
- `npm run build`：通过。
- `cargo check --all-targets`、`cargo fmt --check`：通过。
- Qwen 相关 Rust 测试与 ASR 配置 IPC 回归测试通过；完整 Rust 库测试以单线程运行 443 项全部通过。
- 修复后使用 `--no-sign` 与仅本次生效的 updater artifact 覆盖配置成功生成 1.6.3 主程序和 NSIS 安装包。
- 尚未使用真实 DashScope API Key 与语音样本做在线冒烟测试，发布前应补一次 HTTP 与实时链路人工验证。
