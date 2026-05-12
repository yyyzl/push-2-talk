# AI 助手接入联网搜索（function calling）

## Goal

为 push-to-talk 的「AI 助手模式」（按 Alt+Space 触发）增加**联网搜索**能力，使其能在用户问及实时信息（新闻、股价、最新文档、技术动态等）时**自主**调用搜索工具，并把结果整合为带可点击引用的回答。

**为什么做**：当前 AI 助手只能基于 LLM 训练数据回答，无法处理任何"今天/最新/现在"类问题。联网搜索是 AI 助手能力的关键缺口。

**为什么现在做**：
- 助手模式刚完成"结果面板异步交互"重构，事件机制已就位
- README 已展示"AI 助手历史展示引用文本"，朝多轮交互演进，正好顺势补完
- 业内（kelivo / ChatGPT / Perplexity / Claude.ai）已有成熟的 function calling + citation 模式可借鉴

**首版 Provider 范围（决策见下文 ADR）**：4 个 — Tavily（国际）+ Bocha（中文）+ Serper（Google 代理 / 国内兜底）+ SearXNG（自托管 / 零审查兜底）

---

## Requirements

### 功能需求

#### F1. Function calling 多轮循环（agentic loop）
- LLM 通过 `tools` 字段感知 `search_web` 工具的存在
- 当 LLM 在响应中返回 `tool_calls` 时，后端执行搜索并把结果作为 `role: tool` 消息回灌到 messages 数组
- 再次调用 LLM 直到 `finish_reason != "tool_calls"`
- **最多 3 轮工具调用**（防失控烧 token）

#### F2. 流式响应
- LLM 返回走 SSE streaming
- 流式片段中 `delta.tool_calls` 与 `delta.content` 分别累积
- 后端通过新事件 `assistant_turn_delta` 推增量给前端，前端逐字渲染

#### F3. 搜索 Provider 抽象 + 多引擎支持
- 首版接入 **4 个 Provider**：
  - **Tavily** — 国际 SaaS，自带 `answer` 摘要字段（对 LLM 友好），1000 次/月免费
  - **Bocha 博查** — 中文搜索质量优秀，国内可直连，试用 500 次
  - **Serper** — Google 搜索代理，结果质量高，2500 次/月免费（国内用户兜底，因 Tavily 国内访问可能不稳）
  - **SearXNG** — 自托管元搜索引擎，零成本零审查（高级用户兜底）
- 抽象层为工厂模式，支持后续扩展 Brave / Exa（不在首版）
- Provider 配置含 API key（SearXNG 可为空、Basic Auth 可选）、自定义 endpoint、Provider 专属选项（Serper `gl/hl/tbs`、SearXNG `language/time_range`）、连接测试（最小查询验证 API 凭证与 SearXNG JSON format 可用性）
- 失败自动降级：默认引擎超时 → 按优先级切换到其他已连通引擎

#### F4. 引用渲染
- 搜索结果按 kelivo 约定附带 `index` (1-based) + `id`（12 字符随机 hex，来自 6 字节随机数）
- system prompt 指导 LLM 用 `[citation](index:id)` 格式紧邻原文标注引用
- 前端 Markdown 渲染识别该格式 → 蓝色圆形 chip（数字标号）+ hover popover 显示标题/URL/摘要片段 + click 通过 `@tauri-apps/plugin-opener` 的 `openUrl(url)` 打开浏览器

#### F5. 触发策略
- **完全由 LLM 自主决定**是否搜索（不做关键词正则、不做 prompt 注入半截方案）
- 用户在结果面板输入栏左侧有 🌐 快捷开关，可临时关闭联网（关闭时不发送 tools 字段）
- AssistantConfig 有总开关，关闭时所有 AI 助手调用都不带 tools

#### F6. 搜索关键词来源
- 由 LLM tool_call 自己生成的 `query` 参数（不是原始 ASR 文本）
- 自动规避 TNL 规范化对搜索词的污染（例如 "cloud" 被改成 "Claude" 的问题）

#### F7. 文本处理模式默认禁用搜索
- AI 助手两种模式：QA（无选中文本）/ 文本处理（有选中文本）
- 联网搜索默认**只在 QA 模式生效**
- 提供独立子开关「文本处理模式也启用搜索」（默认关闭）

#### F8. 历史记录展示联网状态
- 主图标沿用 AI 助手 🤖（不为联网另造图标）
- AI 助手记录的顶部 chip 区加蓝色 `🌐 引用 N`（有联网时）或 `🌐 已联网`（联网但失败/无结果时）
- 双栏（用户问题 / AI 助手）下方加可折叠的「参考来源 · N 条」区，展开列出引用的标题 + URL + 来源域名

#### F9. 取消机制
- `tokio::sync::CancellationToken` 串联整个 agentic loop
- 结果面板生成中显示「停止生成」按钮（替代或并列于关闭按钮）
- 取消时：HTTP stream drop、正在执行的搜索请求中断、状态清理

#### F10. 失败降级
- 搜索失败 → 返回 `{"type":"tool_error","error":...}` JSON 给 LLM，由 LLM 自然语言降级回答
- 搜索失败不阻断对话流程
- 历史记录里失败的联网条目仍带 `🌐 已联网` chip 但不展示来源块

### 非功能需求

#### N1. 历史窗口压缩
- 当轮：完整保留 `tool_calls` + `tool` result 消息（含全部 items）
- 追问时压缩之前轮次的 tool result：仅保留 `[{title, url}]` 摘要，不含 snippet 全文
- 防止多轮追问时 context 爆炸

#### N2. 隐私提示
- README 明确告知：用户语音问题会发给第三方搜索 API
- 设置页"联网搜索"区块旁加小字提示

#### N3. 不破坏其他功能
- openai_client 改造影响润色（LlmPostProcessor）和学习（learning/）两条链路
- 必须做回归测试，确保 streaming 改造后非 streaming 调用方仍可用

#### N4. 配置迁移
- 旧版 AppConfig 加载新版字段缺失时使用默认值
- 保持单机桌面应用"无版本兼容包袱"原则（CLAUDE.md 指引）

---

## Acceptance Criteria

### 后端验收

- [ ] `openai_client` 支持 SSE streaming，可解析 OpenAI Chat Completions stream 格式（含 `delta.tool_calls`）
- [ ] `openai_client` 支持 `tools` 请求字段和 `tool_calls` 响应解析
- [ ] 新建 `src-tauri/src/search/` 模块，含 `mod.rs / types.rs / tavily.rs / bocha.rs / serper.rs / searxng.rs / registry.rs`
- [ ] Tavily provider 能正确调用 `https://api.tavily.com/search`，解析 `answer + results[]`
- [ ] Bocha provider 能正确调用 `https://api.bochaai.com/v1/web-search`，解析 `webPages.value`；若实际响应包裹在 `data` 下也兼容 `data.webPages.value`
- [ ] Serper provider 能正确调用 `https://google.serper.dev/search`，解析 `organic[]`，支持可选的 `gl` / `hl` / `tbs` 参数
- [ ] SearXNG provider 能调用用户自托管的 `{base_url}/search?q=...&format=json`，支持可选 Basic Auth，并能识别 JSON format 未启用导致的 403
- [ ] `assistant_processor` 改造为 agentic loop，最多 3 轮，超过则强制结束
- [ ] 工具结果以正确格式插入 messages（`assistant.tool_calls` + 紧跟同 id 的 `tool` 消息）
- [ ] 搜索失败返回 JSON 错误，LLM 仍能继续生成
- [ ] `CancellationToken` 在 loop 任意阶段取消时，立即停止 HTTP stream 和未完成搜索
- [ ] config.rs 新增 `AppConfig.search_config: SearchConfig` + `AssistantConfig` 联网字段，旧配置加载兼容
- [ ] TNL 不会修改 LLM 自己生成的 query 参数（搜索关键词独立路径）

### 前端验收（基于 prototype/ 已验证的交互）

#### 结果面板
- [ ] AssistantBubble 内部、Markdown 上方渲染 `ToolCallPanel`
- [ ] 工具调用 4 种状态正确显示：搜索中（spinner）/ 完成（折叠 + 展开看 N 条）/ 失败（红色降级）/ 多轮（每轮独立块）
- [ ] `[citation](index:id)` 在 Markdown 中渲染为 18px 蓝色圆形 chip
- [ ] chip hover 出 320px popover，含标题/URL/摘要前 100 字 + "点击跳转原文 ↗"
- [ ] chip 点击调用 `@tauri-apps/plugin-opener` 的 `openUrl(url)` 跳浏览器
- [ ] 输入栏左侧有 34×34 联网 toggle（开启 steel 蓝、关闭灰底）
- [ ] 流式响应通过 `assistant_turn_delta` 事件逐字渲染
- [ ] 可恢复降级通过 `assistant_turn_warning` 显示轻量提示，不进入错误气泡、不阻断最终回答
- [ ] 生成中显示「停止生成」按钮，点击触发 `cancel_assistant_generation` 命令
- [ ] 耗时栏新增「搜索 X.Xs」（仅有联网时显示）

#### AI 助手设置页
- [ ] 在「LLM 连接配置」与「问答模式提示词」之间插入「联网搜索」区块
- [ ] 主开关 + 描述
- [ ] 启用后展开：默认引擎下拉 + `[⚙ 管理引擎]` 黑底按钮 + 最大循环 / 单次结果 NumberStepper + 子开关「文本处理模式也启用搜索」
- [ ] 「管理引擎」按钮打开右侧 720px 滑出抽屉
- [ ] 抽屉含通用设置 + Provider 卡片网格 + 添加 Modal（含模板选择 / API Key 隐显 / 测试连接）
- [ ] 抽屉按 Esc 关闭（Modal 打开时不会误触）
- [ ] 完全无独立的 sidebar「联网搜索」一级入口

#### 历史记录页
- [ ] AI 助手记录主图标仍是 🤖（不为联网新增图标）
- [ ] 顶部 chip 区有联网时显示蓝色 `🌐 引用 N`，联网失败时显示 `🌐 已联网`
- [ ] AI 助手双栏下方有可折叠的「🌐 参考来源 · N 条 ▾」
- [ ] 展开后每条来源：编号 + 标题 + 来源域名 + ExternalLink 图标 + 点击跳浏览器
- [ ] 联网开关关闭时（整体或单条），上述 chip 与折叠区均不显示

### 集成验收

- [ ] 跑通完整链路：按 Alt+Space → 录音"今天 OpenAI 有什么新闻" → ASR → 助手→ LLM tool_call → Tavily 搜索 → 结果回灌 → LLM 生成 → 结果面板显示带 `[1][2]` 引用的回答
- [ ] 跑通追问：在结果面板输入"API 价格大概多少" → 再次触发 search_web → 第 2 轮回答带新引用
- [ ] 跑通取消：搜索过程中点「停止生成」→ 立即终止，状态清理
- [ ] 跑通降级：把 Tavily key 故意改错 → LLM 收到 tool_error → 自然语言告知用户搜索失败
- [ ] 润色功能（dictation 模式 LlmPostProcessor）和词库学习功能（learning/llm_judge）回归测试通过
- [ ] 历史记录新条目正确显示 `🌐 引用 5` chip 和折叠的「参考来源」

---

## Definition of Done

- [ ] 所有 Acceptance Criteria 通过
- [ ] `cargo check` / `cargo build --release` 通过
- [ ] `npm run test:ts` 通过
- [ ] 手工跑通端到端链路（含取消、降级、追问）
- [ ] README.md 增加「联网搜索」章节，含隐私提示和 Provider 申请链接
- [ ] CLAUDE.md 更新（架构变更：openai_client streaming、search/ 模块、agentic loop）
- [ ] 保留现有 dark 模式兼容，不新增独立的 dark 模式功能分支
- [ ] `prototype/` 不纳入提交；开发期间仅作为本地 UI 参考，任务收尾时删除本地目录

---

## Technical Approach

### 1. openai_client 改造（前置依赖）

新增能力：
- SSE streaming：`reqwest::Response::bytes_stream()` + `eventsource-stream` crate（或手写 line-by-line 解析）
- `tools` 请求字段：序列化 `Vec<ToolDefinition>` 到请求体
- `tool_calls` 响应解析：在 stream 增量中累积 `delta.tool_calls[].function.{name,arguments}`，最终拼成完整 `ToolCall` 列表

新接口（与现有 `chat()` / `chat_simple()` 并存，不破坏调用方）：
```rust
pub async fn chat_stream(
    &self,
    messages: Vec<Message>,
    options: ChatOptions,
    tools: Option<Vec<ToolDefinition>>,
    cancel_token: CancellationToken,
) -> Result<impl Stream<Item = Result<StreamChunk>>>
```

`StreamChunk` 含 `delta_content: Option<String>` / `delta_tool_calls: Option<Vec<...>>` / `finish_reason: Option<String>`。

### 2. search/ 模块

```
src-tauri/src/search/
├── mod.rs           # SearchService trait + 工厂
├── types.rs         # SearchResult, SearchResultItem, SearchProviderConfig
├── tavily.rs        # impl SearchService for Tavily
├── bocha.rs         # impl SearchService for Bocha
├── serper.rs        # impl SearchService for Serper（Google 代理）
├── searxng.rs       # impl SearchService for SearXNG（自托管）
└── registry.rs      # 多 Provider 管理 + 测试连接 + 降级策略
```

`SearchService` trait：
```rust
#[async_trait]
pub trait SearchService: Send + Sync {
    async fn search(&self, query: &str, max_results: u32, timeout: Duration) -> Result<SearchResult>;
    async fn test_connection(&self) -> Result<u32 /* latency_ms */>;
}
```

返回结果统一附 `index` + `id`（6 位 UUID）。

### 3. assistant_processor 改造为 agentic loop

伪代码：
```rust
async fn process_with_search(...) -> Result<AssistantResponse> {
    let mut messages = build_initial_messages(...);
    let tools = if config.search_enabled && mode == QA {
        Some(vec![SEARCH_WEB_TOOL_DEF])
    } else {
        None
    };
    let mut loop_count = 0;
    let max_loops = 3;

    loop {
        loop_count += 1;
        let stream = openai_client.chat_stream(messages.clone(), opts, tools.clone(), cancel.clone()).await?;
        let (content, tool_calls, finish) = consume_stream(stream, app_handle, cancel.clone()).await?;

        if tool_calls.is_empty() || finish != "tool_calls" || loop_count >= max_loops {
            return Ok(AssistantResponse { content, citations });
        }

        // assistant message with tool_calls
        messages.push(Message::assistant_with_tool_calls(content, tool_calls.clone()));

        // execute each tool, append result
        for tc in &tool_calls {
            let result = execute_tool(tc, &search_registry).await;
            messages.push(Message::tool(tc.id.clone(), tc.function.name.clone(), result));
        }
    }
}
```

### 4. 前端流式渲染

新增事件 `assistant_turn_delta`（详细 payload 见 §A），前端 `useReducer` 增量更新 `currentTurn`；同时 `get_conversation_state` 按 §D 返回 in-flight 草稿，保证 hidden→visible 时可恢复流式中间态。

`ToolCallPanel` 在收到 tool_call 完成后渲染。

### 5. 配置扩展

`SearchConfig` 归属 `AppConfig`，不挂到 `SharedLlmConfig`。理由：搜索引擎不是 LLM Provider 的子资源，AssistantPage、tray 和 backend registry 都应读取同一个搜索配置源，避免出现两个"默认搜索引擎"。

```rust
// AppConfig 新增
pub search_config: SearchConfig,

// 新建 SearchConfig
pub struct SearchConfig {
    pub providers: Vec<SearchProviderConfig>,
    pub default_provider_id: Option<String>,
    pub max_results: u32,        // 默认 5
    pub timeout_secs: u32,        // 默认 6
    pub enable_fallback: bool,    // 失败时切换其他 provider
}

pub struct SearchProviderConfig {
    pub id: String,
    pub provider_type: String,       // "tavily" | "bocha" | "serper" | "searxng"
    pub display_name: String,
    pub enabled: bool,               // 默认 true；fallback 只选 enabled provider
    pub endpoint: Option<String>,    // 自定义反代 / SearXNG base_url
    pub api_key: Option<String>,     // Tavily / Bocha / Serper 必填；SearXNG 可为空
    pub basic_auth_username: Option<String>, // 仅 SearXNG
    pub basic_auth_password: Option<String>, // 仅 SearXNG
    pub serper_gl: Option<String>,   // 仅 Serper
    pub serper_hl: Option<String>,   // 仅 Serper
    pub serper_tbs: Option<String>,  // 仅 Serper
    pub searxng_language: Option<String>,    // 仅 SearXNG
    pub searxng_time_range: Option<String>,  // 仅 SearXNG: day/month/year
}

// AssistantConfig 新增
pub enable_web_search: bool,           // 默认 false（首次启用需用户主动开）
pub web_search_max_loops: u32,         // 默认 3
pub web_search_in_text_mode: bool,     // 默认 false
```

### 6. UI 集成（以现有组件为主，prototype 作参考）

`prototype/` 是交互与视觉参考，不直接覆盖主工程组件。实施策略：
- `prototype/src/components/ToolCallPanel.tsx` 可搬入 `src/components/assistant/ToolCallPanel.tsx`，替换 mock 数据为真实事件状态，补 light/dark 变体。
- `prototype/src/components/SearchProvidersDrawer.tsx` 可搬入 `src/components/assistant/SearchProvidersDrawer.tsx`，接入 `AppConfig.search_config` 的持久化与 `test_search_provider`。
- `prototype/src/components/CitationMarkdown.tsx` **只作参考**；主工程必须扩展现有 `src/components/MarkdownRenderer.tsx`，保留代码块、表格、GFM 与 dark mode 行为。
- `ResultPanelWindow.tsx` 内嵌 `ToolCallPanel`，`MarkdownRenderer` 接收 `citations` prop。
- `AssistantPage.tsx` 插入「联网搜索」区块 + Drawer 触发。
- `HistoryPage.tsx` 加联网 chip 和「参考来源」折叠。

---

## Decision (ADR-lite)

### ADR-1：触发方式 — 选 function calling

**Context**
联网搜索可以用三种模式实现：
- **A. Prompt 注入（半截方案）**：每次调用前用规则/小模型判断是否需要搜索，搜到结果拼到 system prompt
- **B. Function calling（完整方案）**：让 LLM 通过 tool_calls 自主决定是否搜
- **C. 厂商内置搜索**（如 Gemini grounded search）：由 LLM 提供商代管

**Decision**：采用 B（Function calling 多轮循环）

**Consequences**

优点：
- 语义最干净（LLM 决定 vs. 规则猜测）
- 支持多轮工具调用（搜→看→改 query 再搜）
- 与业内主流（kelivo / ChatGPT / Claude.ai）一致，易理解
- 引用机制成熟，体验好

代价：
- 必须先改造 `openai_client` 支持 streaming + tools（约 2 工作日预付）
- 影响润色 / 学习两条链路，需回归测试
- 不所有 LLM provider 都支持 tool calling（OpenAI 兼容的主流 provider 都支持，本项目无影响）

未来演进空间：
- 工具循环架构可复用，未来加 `read_webpage` / `query_local_db` / `execute_code` 等工具不需要再改框架
- Provider registry 可扩展更多搜索引擎

---

### ADR-2：首版 Provider 范围 — 选 4 个

**Context**
kelivo 接了 15 个搜索 Provider。我们需要决定首版接多少，避免过度工程也不过度保守。可选范围：
- **A. 极简 MVP（2 个）**：Tavily + Bocha（一国际一中文）
- **B. 兜底覆盖（4 个）**：A + Serper + SearXNG（加国内兜底 + 自托管兜底）
- **C. 主流齐全（6 个）**：B + Brave + Exa（再加独立索引 + 语义搜索）
- **D. 全量对齐 kelivo（15 个）**

**Decision**：采用 B（4 个）

**Consequences**

为什么不选 A：
- Tavily 国内访问可能不稳，纯靠 Bocha 单点风险
- 缺少"零成本/零审查"选项，对开发者用户群（本项目目标用户）不友好

为什么不选 C/D：
- Brave / Exa 边际收益低，国内用户用得少
- 每多一个 Provider 增加约半天开发 + 长期维护成本
- 全量 kelivo 的 9 个边缘 Provider（Grok / Zhipu / Metaso 等）多为付费场景重合，YAGNI

每个 Provider 的角色定位：
| Provider | 角色 | 目标用户 |
|---|---|---|
| Tavily | 国际默认 | 海外用户、英文搜索 |
| Bocha | 中文默认 | 国内用户、中文搜索 |
| Serper | 国内兜底 | Tavily 不通时的 Google 替代 |
| SearXNG | 自托管兜底 | 高玩、有 docker 经验的开发者、零审查需求 |

未来扩展原则：
- 抽象层是工厂模式，加新 Provider 边际成本低（约半天）
- 用户反馈后再决定是否加 Brave / Exa / 其他
- 不接 Bing 本地解析（HTML 易坏，不适合放进产品）和 DuckDuckGo（免费但限速）

---

## Out of Scope

明确**不在本任务**做的事：

- ❌ **新增暗色模式功能**：不做独立的 dark mode 功能建设；但已有 ResultPanel dark mode 必须继续兼容，新联网组件也要跟随现有 `isDark` 变体
- ❌ **听写模式联网**：联网仅服务 AI 助手，听写流程不变
- ❌ **润色模式联网**：润色不需要实时信息，不引入工具
- ❌ **Sidebar 独立"联网搜索"入口**：IA 决策已定，不做
- ❌ **联网历史的独立筛选 Tab**：HistoryPage 顶部不加"联网"过滤 chip（已有的搜索框够用，避免顶栏拥挤）
- ❌ **首批接入 Brave / Exa**：首版接 4 个（Tavily + Bocha + Serper + SearXNG），Brave/Exa 留作后续扩展
- ❌ **本地搜索引擎（如 Bing 网页解析）**：易被反爬限制，可靠性差
- ❌ **搜索结果二次抓取（fetch full page）**：首版仅用 snippet，不抓取原文
- ❌ **统计面板**：DashboardPage 不加"联网调用次数"统计
- ❌ **多 Provider 并行搜索**：始终单引擎调用，失败才切换
- ❌ **搜索结果缓存**：每次都实时调用（避免缓存一致性复杂度）
- ❌ **用户自定义工具定义**：仅内置 `search_web` 工具

---

## UI 原型参考（已验收）

完整可交互的 UI 原型位于 `prototype/` 目录，独立 vite + React 工程，与主项目隔离。

### 启动方式
```bash
cd prototype
npm install   # 第一次需要装依赖
npm run dev   # 启动 vite，浏览器打开 http://localhost:5180
```

不需要 Tauri、不需要管理员权限、与主项目互不影响。

### 包含 3 个 Tab
1. **结果面板**（`prototype/src/mockups/ResultPanelMockup.tsx`）：4 种状态切换（搜索中 / 单轮完成 / 多轮追问 / 搜索失败），完全 1:1 对齐真实 `src/windows/ResultPanelWindow.tsx`
2. **AI 助手设置**（`prototype/src/mockups/AssistantPageMockup.tsx`）：完整模拟主窗口，含联网搜索区块和管理抽屉，对齐真实 `src/pages/AssistantPage.tsx`
3. **历史记录**（`prototype/src/mockups/HistoryPageMockup.tsx`）：8 条 mock 数据展示联网副标记，对齐真实 `src/pages/HistoryPage.tsx`

### 可复用组件（prototype 作为源参考）
- `prototype/src/components/CitationMarkdown.tsx` — 仅作 citation chip 交互参考；不直接搬入主工程
- `prototype/src/components/ToolCallPanel.tsx` — 可搬入 `src/components/assistant/ToolCallPanel.tsx`，接入真实事件状态
- `prototype/src/components/SearchProvidersDrawer.tsx` — 可搬入 `src/components/assistant/SearchProvidersDrawer.tsx`，接入 `AppConfig.search_config`

### 已验证的设计决策
- ✅ 工具块在 AssistantBubble 内部、Markdown 之前
- ✅ 引用 chip 视觉强度合适（蓝色 18px 圆形）
- ✅ 联网开关在输入栏左侧（与 send 按钮对称）
- ✅ Provider 管理用抽屉而非独立页（IA 不污染 sidebar）
- ✅ 历史副标记轻量（与"AI 助手"chip 平级，不抢戏）

---

## Technical Notes

### 文件清单
- 真实参考：`src/windows/ResultPanelWindow.tsx` / `src/pages/AssistantPage.tsx` / `src/pages/HistoryPage.tsx`
- 改造目标：`src-tauri/src/openai_client.rs` / `src-tauri/src/assistant_processor.rs` / `src-tauri/src/config.rs` / `src-tauri/src/lib.rs`（handle_assistant_mode）
- 新建模块：`src-tauri/src/search/`
- 本地临时 UI 原型：`prototype/`（不纳入提交；开发期间参考，收尾删除）

### 关键约束
- Windows-only 桌面应用（无跨平台兼容包袱，CLAUDE.md）
- 单机应用无版本兼容包袱（数据结构、API 格式可直接调整）
- 不新增独立暗色模式功能，但保留现有 ResultPanel dark mode 兼容
- 不破坏现有润色/学习两条 LLM 链路

### 业内最佳实践参考
- **kelivo** (Flutter)：`G:/project/开源小项目/kelivo/lib/core/services/search/` — 工具定义、引用格式、agentic loop 实现的成熟模板；本项目 4 个 Provider 的请求/响应格式直接对照其 `providers/{tavily,bocha,serper,searxng}_search_service.dart` 实现
- **Provider 官方文档核验**：见 [`research/provider-api-notes.md`](research/provider-api-notes.md)，实施时以官方文档 + 本地 kelivo 代码双重校验为准。
- **ChatGPT Search**：折叠 "Sources" 在回答下方
- **Perplexity**：来源卡片在前 + 引用 chip + 多步骤
- **Claude.ai web search**：折叠 "Searched the web for X"
- **Cherry Studio / ChatBox**：独立 Provider 管理 + 连接测试

### 4 个 Provider 的 API 速查（2026-05-11 核验，实施时再对照 kelivo 源码）
| Provider | 端点 | 鉴权 | 关键请求字段 | 关键响应字段 |
|---|---|---|---|---|
| Tavily | `POST https://api.tavily.com/search` | `Authorization: Bearer {key}` | `query`, `max_results`, `include_answer:true`, `search_depth:basic` | `answer`, `results[].{title,url,content}` |
| Bocha | `POST https://api.bochaai.com/v1/web-search` | `Authorization: Bearer {key}` | `query`, `count`, `summary:true`, `freshness?` | `webPages.value[].{name,url,snippet,summary,siteName,datePublished}` |
| Serper | `POST https://google.serper.dev/search` | `X-API-KEY: {key}` | `q`, `gl?`, `hl?`, `tbs?` | `organic[].{title,link,snippet}` |
| SearXNG | `GET {base_url}/search?q=...&format=json` | 可选 Basic Auth | URL query params | `results[].{title,url,content}` |

SearXNG 注意项：目标实例必须在 `settings.yml` 开启 JSON format；若返回 403，应在测试连接里提示"该实例未启用 JSON API"，而不是归类为 API key 错误。

### 隐私 / 安全
- 用户语音问题会被发送到第三方搜索 API（Tavily / Bocha）
- 必须在 README 和设置页明确告知
- API key 复用现有 LLM Provider 的存储方式（明文存于 `%APPDATA%\PushToTalk\config.json`，符合本项目现状）

### 风险
- **R1**: openai_client 改造影响润色 / 学习链路 → 缓解：保留旧 `chat()` 方法，新增 `chat_stream()`，调用方按需迁移
- **R2**: TNL 污染搜索关键词 → 缓解：架构决策已规避（用 LLM tool_call query，非原始 ASR）
- **R3**: 多轮工具调用烧 token → 缓解：硬编码上限 3 轮 + 用户可调
- **R4**: 搜索 API 超时拖慢用户体验 → 缓解：6s timeout + 失败降级 + "停止生成"按钮

---

## Appendix: Detailed Protocol & Schema Design

> 本附录补充原始 PRD 未落到字段级 / 状态机级的设计细节。开发实施时以本附录为准；与正文冲突的（旧版本）以附录为准。

### A. 事件协议字段表

| 事件 | 触发时机 | Payload | 说明 |
|------|---------|---------|------|
| `assistant_turn_pending` | ASR/文本输入完成后，LLM 调用前 | `{ session_id, user_instruction, selected_text?, has_selection, started_at_ms }` | session_id 必须新增 |
| `assistant_turn_delta` | 流式 chunk 到达（content 或 tool_calls） | `{ session_id, turn_index, loop_round, delta: { content?: string, tool_call?: { id, name, args_chunk, status } } }` | tool_call.status: `accumulating_args` / `executing` / `success` / `error` / `cancelled` |
| `tool_call_started` | 后端开始执行某 tool（参数已完整） | `{ session_id, turn_index, loop_round, tool_call_id, name, query }` | UI 切换为「搜索中」 |
| `tool_call_finished` | tool 执行返回 | `{ session_id, turn_index, loop_round, tool_call_id, status, results?, error?, duration_ms }` | results 含 `index/id/title/url/snippet/source` |
| `assistant_turn_complete` | 整轮 agentic loop 结束（`finish_reason != tool_calls` 或 `loop_count == max`） | `{ session_id, turn: ConversationTurnPayload, is_followup }` | turn 全量结构见 §B |
| `assistant_turn_warning` | 本轮可恢复能力降级 | `{ session_id, code, message }` | 例如 provider 不支持 tools、默认搜索引擎缺失；不替代 complete/error |
| `assistant_turn_error` | LLM 不可恢复失败 | `{ session_id, error_message, partial_content?: string }` | 失败 turn 不落 session.turns |
| `assistant_turn_cancelled` | 用户主动取消生成 | `{ session_id, partial_turn?: ConversationTurnPayload }` | 与 error 区分 UI 行为 |

注意：`assistant_turn_delta` / `tool_call_*` 是 push 增量，可能因 hidden WebView 丢失；`get_conversation_state` 必须能恢复同等的当前 in-flight 草稿状态，见 §D。

### B. `ConversationTurnPayload` 字段表（扩展）

| 字段 | 类型 | 来源 | 说明 |
|------|------|------|------|
| user_instruction | string | 现有 | — |
| selected_text | string? | 现有 | — |
| has_selection | bool | 现有 | — |
| assistant_response | string | 现有 | 含 `[citation](n:id)` 原文 |
| asr_time_ms | u64 | 现有 | 文本追问时 = 0 |
| llm_time_ms | u64 | 现有 | 累计所有 loop 的 LLM 时间 |
| **search_time_ms** | u64? | 新增 | 累计所有 tool_call duration，仅有联网时填充 |
| **tool_calls** | Vec<ToolCallPayload> | 新增 | 按 `loop_round` 排序 |
| **citations** | Vec<SearchResultItem> | 新增 | 全 tool_call 结果扁平化，CitationChip 查表用 |
| **web_searched** | bool | 新增 | `true` = UI 显示 `🌐 引用 N`；`false` = 不显示联网 chip |
| **search_failed** | bool | 新增 | `web_searched=true & search_failed=true` = `🌐 已联网`（无来源块） |

### C. `ToolCallPayload` / `SearchResultItem` Schema

```rust
pub struct ToolCallPayload {
    pub id: String,             // OpenAI 返回的 tool_call_id（用于消息回灌）
    pub name: String,           // 目前仅 "search_web"
    pub query: String,          // LLM 生成的搜索关键词
    pub status: String,         // "running" | "success" | "error" | "cancelled"
    pub results: Option<Vec<SearchResultItem>>,
    pub error_message: Option<String>,
    pub duration_ms: Option<u64>,
    pub loop_round: u32,        // 第几轮 agentic loop（1-based）
}

pub struct SearchResultItem {
    pub index: u32,             // 同 turn 内全局递增（多 tool_call 合并编号）
    pub id: String,             // 12 字符随机 hex（6 字节），防 [citation] 错配
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,         // 域名简写
}

pub struct AssistantWarningPayload {
    pub session_id: String,
    pub code: String,           // "tools_unsupported" | "search_provider_missing" | ...
    pub message: String,
}

pub struct TurnCancelledPayload {
    pub session_id: String,
    pub partial_turn: Option<ConversationTurnPayload>,
}
```

### D. `get_conversation_state` Pull 必须等价 Push

Pull 返回的 `ConversationStatePayload.turns[i]` schema 必须与 `assistant_turn_complete.turn` **完全一致**（含 tool_calls / citations）。同时，流式期间也要返回 in-flight 草稿；否则 ResultPanel 从 hidden→visible 时若错过 `assistant_turn_pending` / `assistant_turn_delta` / `tool_call_*`，只能等整轮 complete 后才恢复 UI。

`ConversationStatePayload` 新增：

```rust
pub struct ConversationStatePayload {
    pub session_id: String,
    pub turns: Vec<ConversationTurnPayload>,
    pub system_prompt_mode: String,         // "qa" | "text_processing"
    pub search_override: Option<bool>,       // §G L3 状态
    pub is_processing: bool,                // = is_assistant_processing.load()
    pub current_loop_round: Option<u32>,    // 流式期间用，pending 轮
    pub pending_turn: Option<TurnPendingPayload>,
    pub draft_assistant_response: Option<String>,
    pub draft_tool_calls: Vec<ToolCallPayload>,
    pub last_warning: Option<AssistantWarningPayload>,
    pub last_error: Option<TurnErrorPayload>,
    pub last_cancelled: Option<TurnCancelledPayload>,
}
```

前端恢复规则：
- `turns` 非空：渲染已完成轮次。
- `pending_turn` 存在：渲染 UserBubble + draft AssistantBubble / ToolCallPanel。
- `draft_assistant_response` 非空：显示已流式生成的内容；`is_processing=false && last_cancelled!=None` 时追加灰色「已停止」标签。
- `draft_tool_calls` 非空：按 `status` 渲染搜索中 / 成功 / 失败 / 取消。
- `last_warning` 只作为本轮轻量提示，不阻断 complete。

后端状态要求：`ConversationSession` 需要保存当前未完成轮次的 draft 状态（pending user、partial content、tool calls、warning/error/cancelled），而不是只在事件里临时构造。`assistant_turn_complete` 后将 draft 转入 `turns` 并清空 draft；`assistant_turn_cancelled` 保留 draft 供 UI 展示，但不写入 `turns` 和历史。

---

### E. `openai_client` Message 模型扩展

```rust
pub enum Role { System, User, Assistant, Tool }   // 新增 Tool

pub struct ToolCallSpec {
    pub id: String,
    pub function_name: String,
    pub arguments_json: String,
}

pub struct Message {
    pub role: Role,
    pub content: Option<String>,                    // 仅 tool_calls 时可为 None
    pub tool_calls: Option<Vec<ToolCallSpec>>,       // 仅 Assistant role 使用
    pub tool_call_id: Option<String>,               // 仅 Tool role 使用
    pub name: Option<String>,                       // Tool role 时 = function_name
}
```

**向后兼容**：现有 `Message::system / user / assistant(content)` 构造器保留；新增 `Message::assistant_with_tools(content?, tool_calls)` / `Message::tool_result(tool_call_id, name, content)`。`Option<...>` 字段 None 时序列化跳过（serde skip_serializing_if），不污染润色 / 仲裁 / 学习链路的请求体。

### F. SSE Streaming 接口

```rust
pub struct StreamChunk {
    pub delta_content: Option<String>,
    pub delta_tool_calls: Option<Vec<DeltaToolCall>>,
    pub finish_reason: Option<String>,              // "stop" | "tool_calls" | "length"
}

pub struct DeltaToolCall {
    pub index: u32,                                 // OpenAI delta 内 tool_call 索引
    pub id: Option<String>,                         // 首 chunk 出现
    pub function_name: Option<String>,              // 首 chunk 出现
    pub arguments_delta: Option<String>,            // 增量 JSON 字符串
}

pub async fn chat_stream(
    &self,
    messages: Vec<Message>,
    options: ChatOptions,
    tools: Option<Vec<ToolDefinition>>,
    cancel_token: CancellationToken,
) -> Result<impl Stream<Item = Result<StreamChunk>>>;
```

**SSE 解析兼容矩阵**（PR1 必测）：
- ✅ OpenAI 官方
- ✅ 智谱 GLM（含 GLM-4-Flash）
- ✅ DeepSeek（含 `reasoning_content` 字段忽略）
- ✅ 通义千问（OpenAI 兼容端点）
- 边界：`data: [DONE]` 结束符、空行 keepalive、`event:` 前缀、HTTP 错误码包装

**不支持 tools 的 LLM provider**：客户端层 detect → 自动回退「无 tools 的普通助手回答」+ 发 `assistant_turn_warning` 提示用户本轮未联网；不发 `assistant_turn_error`，因为这是可恢复降级。

---

### G. 联网开关三层优先级真值表

| 层级 | 字段 | 持久化 | 范围 |
|------|------|--------|------|
| L1 总开关 | `AssistantConfig.enable_web_search` | ✅ JSON | 全局 |
| L2 文本处理子开关 | `AssistantConfig.web_search_in_text_mode` | ✅ JSON | TextProcessing session |
| L3 面板临时开关 | `ConversationSession.search_override: Option<bool>` | ❌ session 期内 | 当前 session |

**优先级链**：L1=off → 任何情况都不发；L1=on → L3 (Some 显式覆盖) > L2 (mode 限制) > 默认（QA=on, TextProc=off）。

**真值表（8 组合）**：

| L1 | L2 | session.mode | L3 | tools 是否发送 |
|----|----|--------------|----|---------------|
| off | * | * | * | ❌ |
| on | * | QA | None | ✅ |
| on | * | QA | Some(true) | ✅ |
| on | * | QA | Some(false) | ❌ |
| on | off | TextProcessing | None | ❌ |
| on | on | TextProcessing | None | ✅ |
| on | off | TextProcessing | Some(true) | ✅（用户强制覆盖） |
| on | on | TextProcessing | Some(false) | ❌ |

**生命周期**：L3 在 session 创建时 = None（继承 L1/L2 默认）；用户点 🌐 toggle → 写入 L3；`dismiss_conversation` 销毁 session → L3 丢失，下次新 session 重新继承。

**前后端同步**：
- 新 command `set_session_search_override(enabled: bool) -> Result<()>`，写入 `ConversationSession.search_override`
- 前端 toggle 点击 → 立即 invoke 该 command，**保证语音追问 / 文本追问行为一致**
- toggle 视觉状态从 `get_conversation_state.search_override` 读取（首次面板打开时同步）

---

### H. 取消机制详细设计

**State 扩展**：
```rust
pub struct AppState {
    /// 当前正在进行的助手生成的取消 token（Some = 生成中）
    assistant_cancel_token: Arc<Mutex<Option<CancellationToken>>>,
}
```

**新增 Command**：
```rust
#[tauri::command]
async fn cancel_assistant_generation(state: State<'_, AppState>) -> Result<(), String>;
```
- 取出 token 并 `.cancel()`
- **不**清理 session（用户可重试）
- 发 `assistant_turn_cancelled` 事件（含 `partial_turn?`）

**UI 行为**：
- 生成期间 ResultPanel **底部操作栏**用「停止生成」按钮（Square 图标 + 红色文字）替代「关闭」按钮位置，完成后恢复「关闭」
- 标题栏关闭按钮（X）行为 = `dismiss_conversation`；若生成中，后端先 cancel 等待 ≤500ms，再清理 session 并 hide 面板
- **Esc 行为分层**：
  - pending 中（`is_assistant_processing=true`）→ Esc = `cancel_assistant_generation`，保留 session + 面板
  - 非 pending → Esc = `dismiss_conversation`
  - textarea 聚焦时同样遵循上面两条；不再单独走 dismiss，避免输入框聚焦时误关掉正在生成的会话

**取消后语义**：
- 已生成 content 保留到 `assistant_turn_cancelled.partial_turn` 但**不写入 session.turns**
- **不落历史记录**（与 error 一致）
- 用户可重试（重按热键 / 再发文本追问）

**并发策略**：
- 生成期间按助手热键 → 忽略 + tooltip「正在生成，按 [停止] 取消」
- 录音模式（press/toggle）与「停止生成」按钮互不干扰（按钮仅 LLM 阶段显示）

---

### I. `SearchConfig` Schema + Provider 生命周期

```rust
// AppConfig 扩展
pub struct AppConfig {
    // 现有字段...
    #[serde(default)]
    pub search_config: SearchConfig,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SearchConfig {
    #[serde(default)]
    pub providers: Vec<SearchProviderConfig>,
    #[serde(default)]
    pub default_provider_id: Option<String>,    // 删光后置 None
    #[serde(default = "default_max_results")]
    pub max_results: u32,                        // 默认 5
    #[serde(default = "default_search_timeout")]
    pub timeout_secs: u32,                       // 默认 6
    #[serde(default = "default_enable_fallback")]
    pub enable_fallback: bool,                   // 默认 true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SearchProviderConfig {
    pub id: String,                              // UUID
    pub provider_type: String,                   // "tavily" | "bocha" | "serper" | "searxng"
    pub display_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,                // 自定义反代 / SearXNG base_url
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,                 // Tavily / Bocha / Serper 必填；SearXNG 可为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_auth_username: Option<String>,     // 仅 SearXNG
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_auth_password: Option<String>,     // 仅 SearXNG
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_gl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_hl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serper_tbs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_time_range: Option<String>,
}

// AssistantConfig 扩展
pub struct AssistantConfig {
    // 现有字段...
    #[serde(default)]
    pub enable_web_search: bool,                 // 默认 false（首次启用需用户主动开）
    #[serde(default = "default_web_search_max_loops")]
    pub web_search_max_loops: u32,               // 默认 3
    #[serde(default)]
    pub web_search_in_text_mode: bool,           // 默认 false
}
```

**新增 Command**：
- `test_search_provider(provider: SearchProviderConfig) -> Result<u32 latency_ms, String>`
- `save_search_config(config: SearchConfig)`（也可走现有 `patch_config_fields` 路径）

**Provider 删除策略**：
- 删除最后一个 → `default_provider_id` 置 None
- 删除当前 default → 自动切到 `providers[0]`，若 providers 为空置 None
- agentic loop 启动前若 default 为 None / default provider disabled / 必填凭证为空且 `enable_web_search=true` → **short-circuit**：不发 tools 字段，本轮不联网（不算失败、UI 无 🌐 chip），同时发 `assistant_turn_warning` 提示配置不完整

**配置迁移**：旧 config 缺 `AppConfig.search_config` → 使用 `SearchConfig::default()`；缺 `enable_web_search` → false。

---

### J. Markdown 渲染集成策略（关键决策修正）

❗ **不要用 prototype 的 `CitationMarkdown` 直接替换 `MarkdownRenderer`**——后者支持代码块（Prism syntax-highlighter）、表格、标题层级、列表、引用块、暗色模式；前者只覆盖 `p / li / strong / a` 4 个组件。

**正确做法**：在 [`src/components/MarkdownRenderer.tsx`](src/components/MarkdownRenderer.tsx) 内增加 citation 渲染：

1. 新增可选 prop `citations?: SearchResultItem[]`，无引用时退化为现有行为（不破坏润色 / 历史 / 仲裁等其他用例）
2. 在 `p / li / strong / em / blockquote / td / th / h1 / h2 / h3 / a` 11 个组件中调用统一的 `transformChildren(children, citations)` 工具函数。`code`（inline）和 `pre` 内不走 transform（保持代码字面量）。完整覆盖矩阵见 §Y.3
3. CitationChip 子组件**必须支持 dark mode**（accent 色：`isDark ? "#7BA9D3" : "var(--steel)"`）
4. prototype/CitationMarkdown.tsx **仅作参考**，不进 src/

### K. Dark Mode 兼容（修正 Out of Scope）

原 PRD「不引入 dark」与「不破坏现有功能」自相矛盾——[ResultPanelWindow.tsx](src/windows/ResultPanelWindow.tsx) 全文走 `isDark` 分支。修正措辞为：

> 不**新增**独立的 dark 模式分支代码，但已有的 dark 支持必须保留。所有从 prototype/ 搬入 src/ 的新组件（ToolCallPanel / CitationChip / SearchProvidersDrawer）必须同时实现 light + dark 变体。

具体 token 映射：
- 背景：`isDark ? "rgba(255,255,255,0.05)" : "rgba(20,20,19,0.02)"`
- 边框：`isDark ? "rgba(232,230,220,0.08)" : "var(--sand)"`
- accent：`isDark ? "#7BA9D3" : "var(--steel)"`
- error accent：`isDark ? "#E07A5C" : "#C05F3F"`

---

### L. `openai_client` 改造的全部影响链路

| 链路 | 文件:行 | 调用 | 需 stream | 需 tools |
|------|---------|------|----------|---------|
| 语句润色 | llm_post_processor.rs:293 | chat_simple | ❌ | ❌ |
| **TNL 候选仲裁** | llm_post_processor.rs:491 | chat_simple | ❌ | ❌ |
| 词库学习判断 | learning/llm_judge.rs:150 | chat | ❌ | ❌ |
| AI 助手（首轮 QA） | assistant_processor.rs:60 | chat_simple | ✅ | ✅ |
| AI 助手（首轮 TextProc） | assistant_processor.rs:98 | chat_simple | ✅ | ✅ |
| AI 助手（追问） | assistant_processor.rs:142 | chat | ✅ | ✅ |

原 PRD 漏列「TNL 候选仲裁」。改造原则：保留 `chat() / chat_simple()` 同步接口零变更，新增 `chat_stream()` 接口；Message 扩展字段通过 `Option<...>` 序列化默认跳过，老调用方零修改。

---

### M. 历史持久化 schema 扩展

`TranscriptionResult`（[lib.rs:237](src-tauri/src/lib.rs:237)）新增：
```rust
pub citations: Option<Vec<SearchResultItem>>,
pub tool_calls_summary: Option<Vec<ToolCallSummary>>,   // 压缩版：仅 query/results_count/duration
pub web_searched: bool,                                  // 默认 false
pub search_failed: bool,                                 // 默认 false
```

前端 `HistoryRecord`（[types/index.ts:213](src/types/index.ts:213)）对应字段：`citations?, toolCallsSummary?, webSearched, searchFailed`。

**复制行为分两类**：
- 内部存储 / `copy_latest_reply`：保留 `[citation](n:id)` 原文（前端 chip 渲染所需）
- `copy_full_conversation` / `format_conversation_for_copy`：将 `[citation](n:id)` **转换为可点 markdown 链接** `[¹](url)` 等（外部编辑器友好）

**老历史兼容**：缺字段使用默认值；UI 在 `web_searched=false` 时不显示联网 chip 与「参考来源」折叠区。

---

### N. `process_followup` 必须改造为 agentic 历史构造

当前 [assistant_processor.rs:171](src-tauri/src/assistant_processor.rs:171) `build_followup_messages` 只生成 user/assistant 二元历史。引入 tool calling 后必须：

1. 历史轮次的 assistant 消息**带上原 tool_calls**（OpenAI 协议要求 assistant.tool_calls 后必须紧跟同 id 的 tool 消息）
2. 历史 tool result 按 N1 压缩规则：**当前轮**完整保留；**追问前的旧轮**仅保留 `[{title, url, id}]` 摘要（去 snippet）
3. 压缩**保留 `id` 字段**，否则旧轮 `[citation](n:id)` chip 失效
4. 若历史中存在搜索失败的 tool_call，回灌时仍要发 `{"type": "tool_error", "error": ...}` 占位（不能丢，否则 assistant.tool_calls 与 tool 消息数量不匹配，API 会 400）

伪代码补充：
```rust
fn build_agentic_followup_messages(
    system_prompt: &str,
    history: &[ConversationTurn],   // 含 tool_calls
    new_instruction: &str,
    new_selected_text: Option<&str>,
) -> Vec<Message> {
    let mut messages = vec![Message::system(system_prompt)];
    let window = sliding_window(history, MAX_CONVERSATION_TURNS);

    for (i, turn) in window.iter().enumerate() {
        let is_last = i == window.len() - 1;
        messages.push(Message::user(format_user_content(&turn.user_instruction, turn.selected_text.as_deref())));

        // 重建该 turn 的 tool_calls + tool messages（旧轮压缩）
        for tc in &turn.tool_calls {
            messages.push(Message::assistant_with_tools(None, vec![tc.to_spec()]));
            let payload = if is_last {
                tc.full_result_json()           // 当前轮完整 JSON
            } else {
                tc.compact_result_json()        // 旧轮仅 [{title, url, id}]
            };
            messages.push(Message::tool_result(tc.id.clone(), "search_web", payload));
        }

        // 最终 assistant 文本回复
        if !turn.assistant_response.is_empty() {
            messages.push(Message::assistant(&turn.assistant_response));
        }
    }
    messages.push(Message::user(format_user_content(new_instruction, new_selected_text)));
    messages
}
```

---

### O. 错误状态机精细化

| 状态 | 触发 | ToolCall.status | UI 表现 |
|------|------|-----------------|---------|
| 搜索中 | tool_call_started | running | spinner + 灰背景 |
| 成功有结果 | finished + `results.len() > 0` | success | 折叠 + 可展开列表 |
| **成功空结果** | finished + `results.len() == 0` | success | 「未找到相关结果」文案，不可展开 |
| 网络/超时失败 | finished + error | error | 红色降级，errorMessage 显示原因 |
| **取消** | cancel_token.cancel | cancelled | 灰色 + 「已停止」 |

**API key / provider 缺失 short-circuit**：`enable_web_search=true` 但 `default_provider_id` 为 None、provider disabled、必填 key 为空或 SearXNG endpoint 为空 → 启动 loop 前不发 tools 字段，前端 chip 区不显示 🌐（视为本轮未联网），**不算失败**，仅通过 `assistant_turn_warning` 提示配置不完整。

---

### P. UI 微调清单

- **输入栏宽度**：480px 面板加入 34×34 globe toggle 后 textarea 实际可用宽度 ~360px，确认 placeholder「输入追问...」不被截断
- **`assistant_turn_pending` payload** 新增 `session_id`（前端可校验归属）
- **`config_updated` 事件**触发时前端结果面板**不**重置 pendingTurn / pendingToolCall（仅刷新 theme）；toggle 状态从 `session.search_override` 读取，与 config 解耦
- **引用 id**：6 字节随机 hex（12 字符），降低同 turn 内碰撞概率；CitationChip 用 `(index, id)` 双键查找 + index 单键 fallback
- **`is_assistant_processing` 命名沿用**，但语义扩展为「整个 agentic loop 期间为 true」（含 tool 执行）
- **超时设计**：单次 `SearchService::search` 内部 6s timeout（独立 reqwest client），整个 agentic loop 不超时（受 `ASSISTANT_TIMEOUT_SECS=300` 约束）；流式 LLM 调用使用 `read_timeout`（非 `total_timeout`）

---

### Q. Commit Plan 调整为 13 个 commit（修订自 §T-Z）

| # | Commit | 范围 | 预计 |
|---|---|---|---|
| 1 | `feat(llm): openai_client Message 模型扩展支持 Tool role` | openai_client.rs 增 Role::Tool + ToolCallSpec + Message 新字段 + 单测；老调用方零变更 | 0.5d |
| 2 | `feat(llm): openai_client SSE streaming + fixture 测试` | chat_stream() 接口 + SSE 解析 + 6 个 fixture 单测（§Z.1） | 1.5d |
| 3 | `feat(llm): openai_client tools / tool_calls 累积 + 不支持时降级` | tools 请求字段 + delta.tool_calls 累积 + warn 事件 fallback | 1d |
| 4 | `feat(search): search 模块 + 4 个 provider + test_search_provider 命令` | src-tauri/src/search/{tavily,bocha,serper,searxng}.rs + 测试连接命令 | 2d |
| 5 | `feat(config): SearchConfig + AssistantConfig 联网字段 + 真值表实现` | config.rs 扩展 + Default 兜底 + L1/L2/L3 真值表 | 0.5d |
| 6 | `refactor(assistant): 统一 process_turn 入口 + deprecated 旧三方法` | §V — 合并 process / process_with_context / process_followup | 0.5d |
| 7 | `feat(assistant): agentic loop + cancel_token + session.search_override + tool 历史重建` | assistant_processor.rs + lib.rs handle_assistant_mode + cancel_assistant_generation / set_session_search_override 命令 | 2d |
| 8 | `feat(assistant): send_text_question 改 spawn task + dismiss 联动 cancel` | §U + §W — 立即 return + 后台 task；dismiss 触发 cancel 等 ≤500ms | 0.5d |
| 9 | `feat(assistant): emit_conversation_history 扁平化 + citation 重编号 + format_for_copy 双分支` | §T.1 + §X — Internal/External 两种复制语义 | 0.5d |
| 10 | `feat(ui): MarkdownRenderer 支持 citation（11 元素 + dark）` | §Y.3 — 扩 p/li/strong/em/blockquote/td/th/h1/h2/h3/a；code/pre 不动 | 0.5d |
| 11 | `feat(ui): ResultPanel ToolCallPanel + 流式 + 停止生成 + globe toggle + 取消 UI 残留` | ResultPanelWindow.tsx + 从 prototype 搬入 ToolCallPanel + CancelledBubble + 重试按钮 + dark 变体 | 2d |
| 12 | `feat(ui): AssistantPage 联网区块 + Drawer + HistoryPage 副标记 + useTauriEventListeners 字段映射 + tray 开关` | §T.2 + §Y.4 + §Y.5 — 9 个文件 | 1.5d |
| 13 | `docs: README 联网搜索章节 + 隐私提示 + CLAUDE.md 架构更新` | README.md + CLAUDE.md | 0.5d |

**合计：约 13 工作日**

PR 拆分：1-3 = PR1（openai_client 改造），4-9 = PR2（搜索后端 + agentic loop + 数据流），10-12 = PR3（前端集成），13 = PR4（文档收尾）。

---

### R. Acceptance Criteria 补充

#### 协议层（PR1 / PR2 验收）
- [ ] `Message` 序列化向后兼容：润色 / 仲裁 / 学习三条非 stream 链路调用 `chat_simple` 输出的请求体与改造前完全一致（字段级 diff 单测）
- [ ] SSE 解析单测覆盖 OpenAI / GLM / DeepSeek / 通义 4 家厂商响应样本
- [ ] `chat_stream` 在 `cancel_token.cancel()` 触发时 ≤500ms 内停止 yield 并 drop HTTP stream
- [ ] 不支持 tools 的 LLM provider 自动降级为普通助手回答 + 发 `assistant_turn_warning` 事件
- [ ] `process_followup` 重建消息时 assistant.tool_calls 与 tool 消息 1:1 匹配（API 不返回 400）
- [ ] `build_agentic_followup_messages` 压缩旧轮 snippet 但保留 id

#### 状态机层（PR2 / PR3 验收）
- [ ] L1/L2/L3 真值表 8 个组合各通过一次手工测试
- [ ] 取消生成保留 partial_content + 不落 session.turns + 不落历史
- [ ] API key 缺失 short-circuit：不发 tools / 不算失败 / UI 无 🌐 chip
- [ ] 工具调用「成功空结果」状态正确显示「未找到相关结果」
- [ ] 多轮 agentic loop（≥2 轮）能正确发 `tool_call_started` / `tool_call_finished` 各两次

#### UI / Pull 等价（PR3 验收）
- [ ] 面板从 hidden→visible 切换时 pull 拿到的 turns 含 tool_calls + citations，chip hover popover 正常显示
- [ ] 面板从 hidden→visible 切换且生成仍在进行时，pull 能恢复 `pending_turn` / `draft_assistant_response` / `draft_tool_calls`，不必等 complete
- [ ] toggle 点击立即 invoke `set_session_search_override`，语音追问读取一致
- [ ] MarkdownRenderer 增加 citation 渲染后不破坏代码块 / 表格 / dark 模式
- [ ] 复制对话时 `[citation](n:id)` 自动转换为可点 markdown 链接
- [ ] HistoryPage 联网失败 turn 显示 `🌐 已联网` 但无来源块；成功 turn 显示 `🌐 引用 N` + 折叠区

#### 数据流验收（PR2 / PR3 验收）— §T 新增
- [ ] `emit_conversation_history` 扁平化输出：`TranscriptionResult.citations.len()` = `session.turns` 内全部 `tool_calls.results` 之和
- [ ] 多轮 session 入历史后 `[citation](n:id)` 的 n 值**全局递增且无冲突**（regex 扫描 `text` 字段所有匹配）
- [ ] `src/hooks/useTauriEventListeners.ts` 的 `transcription_complete` handler 正确把 `web_searched / search_failed / citations / tool_calls_summary` 写入 `HistoryRecord`
- [ ] 老历史（无新字段）加载时 `webSearched=false` / `searchFailed=false` / `citations=undefined`，UI 不渲染联网 chip 与折叠区

#### dismiss / cancel 副作用验收 — §U 新增
- [ ] 生成中点击「关闭」→ 触发 cancel → 等待 ≤500ms → 清理 session → 隐藏面板 → tracing 日志显示 token 取消而非自然完成
- [ ] 生成中按 Esc → 触发 `cancel_assistant_generation` → 保留 session + 面板 + partial_content，不落历史
- [ ] 生成中点击「停止生成」→ partial_content 在面板保留 + CancelledBubble 显示 + `is_assistant_processing` 立即落 false + 用户可立即重试
- [ ] cancel 后 100ms 内再按热键能进入新会话（不被 CAS 拒绝）

#### 入口统一验收 — §V 新增
- [ ] 首轮 QA / 首轮 TextProcessing / 录音追问 / 文本追问 4 条路径**全部走单一 `process_turn` 入口**
- [ ] 旧 `process` / `process_with_context` / `process_followup` 在 PR2 末标记 deprecated 后无 `unused` warning，且功能性 0 差异

#### 工程验收 — §Z 新增
- [ ] `cargo test` 跑通 6 个 SSE fixture 解析单测（OpenAI / GLM / DeepSeek / Qwen / 错误 / DONE 边界）
- [ ] 本任务提交不包含 `prototype/`；最终收尾前删除本地 `prototype/`，确保它不再出现在 `git status`
- [ ] reqwest 版本检查记录在 PR1 第一个 commit 的 message 中（≥ 0.12 用 `read_timeout()`，否则用 `tokio::time::timeout` 包 stream）
- [ ] tray 联网开关与设置页 L1 总开关双向同步通过 `emit_config_updated`

---

### S. Definition of Done 增补

- [ ] 所有 §R 中的 AC 通过
- [ ] PR1 合并后回归：润色 / 仲裁 / 学习功能跑通无差异
- [ ] PR2 合并后回归：原 AI 助手（不启用联网）跑通新会话 / 追问 / 文本追问 / 关闭面板 4 条路径
- [ ] PR3 合并后端到端：按 Alt+Space → 录音 → 联网 → 引用 chip / 取消 / 降级 4 个场景手工跑通
- [ ] dismiss 联动 cancel 端到端验证（手工：开始生成 → 立即关面板 → 查看 tracing 日志确认 token 取消而非完成）
- [ ] tray 联网开关与设置页双向同步（修一处另一处即时反映）
- [ ] cancel 后用户体感：UserBubble 保留 + 「重试」按钮立即可用 + 重试能成功

---

### T. 数据流图：事件 → AppState → 历史持久化（补 §A/§B/§M 漏画的多跳链路）

```
[ 用户按 Alt+Space 录音 ]
        ↓
[ ASR + TNL ]
        ↓
[ handle_assistant_mode → process_turn (统一入口，§V) ]
        ↓
  ┌────────────────────────────────────────┐
  │ chat_stream → SSE chunks               │
  │   ↓ 每片                                │
  │ assistant_turn_delta → ResultPanel     │
  │   ↓ tool_calls 完成                    │
  │ tool_call_started → ResultPanel        │
  │ search/registry::search()              │
  │ tool_call_finished → ResultPanel       │
  │   ↓                                    │
  │ messages.push(tool_result) → 下一轮    │
  └────────────────────────────────────────┘
        ↓ loop 退出 (finish_reason != tool_calls 或 loop_count >= max)
[ ConversationTurn { ..., tool_calls } 写入 session.turns ]
        ↓
[ assistant_turn_complete → ResultPanel ]
        ↓ 用户 dismiss（或自然结束后用户关）
[ dismiss_conversation ]
        ↓ §U：若 is_processing 则先 cancel 等 ≤500ms
[ emit_conversation_history(session) ]
        ↓ §T.1：扁平化 + 全局重编号 + 改写正文 [citation](n:id)
[ TranscriptionResult { ..., citations, tool_calls_summary, web_searched, search_failed } ]
        ↓ tauri emit
[ useTauriEventListeners.ts listen("transcription_complete") ]
        ↓ §T.2：字段映射
[ HistoryRecord { ..., citations, toolCallsSummary, webSearched, searchFailed } ]
        ↓
[ HistoryPage 渲染 chip + 折叠区 ]
```

**实施顺序锁定**：必须先 lib.rs 端 `emit_conversation_history` 改造（输出新字段），再 `src/hooks/useTauriEventListeners.ts` 端事件→HistoryRecord 映射，否则前端会接到带新字段的 payload 但不入库。

#### T.1 `emit_conversation_history` 改造伪码

```rust
fn emit_conversation_history(app: &AppHandle, session: &ConversationSession, inserted: bool) {
    // 1. 扁平化所有轮次的 tool_calls，建立 (turn_idx, local_idx) → global_idx 重编号表
    let mut all_citations: Vec<SearchResultItem> = Vec::new();
    let mut tool_calls_summary: Vec<ToolCallSummary> = Vec::new();
    let mut next_global_index: u32 = 1;
    let mut index_remap: HashMap<(u32, u32), u32> = HashMap::new();

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        for tc in &turn.tool_calls {
            tool_calls_summary.push(tc.compact_summary());
            if let Some(results) = &tc.results {
                for item in results {
                    let global_idx = next_global_index;
                    next_global_index += 1;
                    index_remap.insert((turn_idx as u32, item.index), global_idx);
                    let mut renumbered = item.clone();
                    renumbered.index = global_idx;
                    all_citations.push(renumbered);
                }
            }
        }
    }

    // 2. format 时按 remap 表改写每轮 assistant_response 中的 [citation](n:id)
    let renumbered_text = format_conversation_for_copy_with_remap(
        &session.turns, CopyMode::Internal, &index_remap
    );

    let web_searched = !tool_calls_summary.is_empty();
    let search_failed = web_searched
        && tool_calls_summary.iter().all(|s| s.status != "success" || s.results_count == 0);

    let result = TranscriptionResult {
        text: renumbered_text,
        // ... 现有 9 个字段
        citations: if web_searched { Some(all_citations) } else { None },
        tool_calls_summary: if web_searched { Some(tool_calls_summary) } else { None },
        web_searched,
        search_failed,
    };
    let _ = app.emit("transcription_complete", result);
}
```

#### T.2 `useTauriEventListeners.ts` 事件 handler 字段映射

```ts
// src/hooks/useTauriEventListeners.ts 内 listen("transcription_complete") 回调追加：
const newRecord: HistoryRecord = {
    // ... 现有字段保持不变
    citations: payload.citations ?? undefined,
    toolCallsSummary: payload.tool_calls_summary ?? undefined,
    webSearched: payload.web_searched ?? false,
    searchFailed: payload.search_failed ?? false,
};
```

---

### U. dismiss / cancel / error 副作用矩阵（修订 §H）

原 PRD 把 cancel 和 dismiss 拆得太干净。streaming 后实际应有"dismiss 触发 cancel"的级联，否则关面板时 token 还在烧到 LLM 完结。

| 触发场景 | `is_assistant_processing=true` 时 | `is_assistant_processing=false` 时 |
|---|---|---|
| 标题栏 X / 操作栏「关闭」 | **先 cancel_token.cancel() 等 ≤500ms → 清理 session → hide 面板**；不发 partial_turn（窗口要关） | 直接清理 session + hide 面板（现状） |
| Esc | cancel_token.cancel() → 发 `assistant_turn_cancelled` (含 partial_content) → **保留 session + 保留面板** + is_processing 立即 false | dismiss session + hide 面板 |
| 「停止生成」按钮 | cancel_token.cancel() → 发 `assistant_turn_cancelled` (含 partial_content) → **保留 session + 保留面板** + is_processing 立即 false | 按钮不显示 |
| LLM 不可恢复错误 | 自然结束 → 发 `assistant_turn_error` → 保留 session + 保留面板 + is_processing 落 false | N/A |
| 用户按助手热键 | **拒绝（CAS 失败）** + overlay tooltip「正在生成，按「停止」取消」 | 正常进入新会话 / 追问流程 |
| 输入栏发文本追问 | 拒绝（CAS 失败）+ inline 提示「正在生成，请稍候」 | 正常追问 |

#### U.1 `dismiss_conversation` 改造伪码

```rust
async fn dismiss_conversation(app, state) -> Result<(), String> {
    // 1. 若在飞，先取消
    if state.is_assistant_processing.load(Ordering::SeqCst) {
        if let Some(token) = state.assistant_cancel_token.lock().unwrap().take() {
            token.cancel();
        }
        // 等取消传播（最多 500ms，避免无限阻塞）
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while state.is_assistant_processing.load(Ordering::SeqCst)
              && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    // 2. 清理 session（现有逻辑）
    if let Some(session) = state.conversation_session.lock().unwrap().take() {
        emit_conversation_history(&app, &session, false);
    }
    hide_result_panel_window(&app).await;
    Ok(())
}
```

#### U.2 cancel 后 UI 残留约定

| 元素 | 状态 |
|---|---|
| `UserBubble` | 保留（用户提问视觉锚点） |
| `LoadingBubble` | **替换为新 `CancelledBubble`**（灰色背景 + 「已停止生成」+ 「重试」按钮） |
| 已部分流式渲染的 `AssistantBubble` | 保留显示 + inline「(已停止)」灰色标签 |
| 「重试」按钮 | 仅文本追问可重试（重新调 `send_text_question`，参数为最后一次 user_instruction）；语音追问需用户重按热键 |

#### U.3 is_processing 落 false 时机

`cancel_token.cancel()` 后，agentic loop 内 `tokio::select!` 检测到取消应**在第一时间** `state.is_assistant_processing.store(false, ...)`，不要等 stream drop / HTTP socket close 完成。否则用户连点重试会被 CAS 拒绝。

---

### V. agentic 入口统一改造（修订 §F1 与 §N）

原 PRD 只针对 `process_followup` 设计 agentic 历史构造，但 [`assistant_processor.rs`](src-tauri/src/assistant_processor.rs) 当前**首轮**根本不走 followup 路径（[lib.rs:2600-2606](src-tauri/src/lib.rs:2600) 调的是 `process` / `process_with_context`），必须统一。

**决策**：合并为单一 entry。`AssistantProcessor` 暴露：

```rust
pub async fn process_turn(
    &self,
    history: &[ConversationTurn],            // 首轮为空
    new_instruction: &str,
    new_selected_text: Option<&str>,
    prompt_mode: &PromptMode,
    tools: Option<Vec<ToolDefinition>>,      // None = 不联网
    cancel_token: CancellationToken,
    progress_emitter: impl Fn(StreamEvent),
) -> Result<TurnOutcome>;

pub struct TurnOutcome {
    pub assistant_response: String,           // 含 [citation](n:id) 原文
    pub tool_calls: Vec<ToolCallPayload>,     // 按 loop_round 排序
    pub llm_time_ms: u64,
    pub search_time_ms: Option<u64>,
}
```

- 首轮 QA：`history=[]`、`selected_text=None`、`prompt_mode=QA`
- 首轮 TextProc：`history=[]`、`selected_text=Some(...)`、`prompt_mode=TextProcessing`
- 录音追问：`history=session.turns`、其余按需
- 文本追问：同上 + `selected_text=None`、`asr_time_ms=0`（外层赋值）

`build_followup_messages` 改名为 `build_turn_messages`（首轮时 history 为空，只产出 `[system, user]`）。

旧的 `process` / `process_with_context` / `process_followup` 在 PR2 commit 6 标记 `#[deprecated]` 但保留至 PR2 完结便于增量迁移；PR2 commit 9 删除三个旧方法。

**lib.rs 统一**：[handle_assistant_mode lib.rs:2462](src-tauri/src/lib.rs:2462) 的"新会话 vs 追问"分支可压缩为单一 `process_turn` 调用，分支只负责构建 `prompt_mode`（首轮锁定）和 `history`（首轮空）。

---

### W. send_text_question streaming 改造（补 §H 漏点）

[lib.rs:4295 send_text_question](src-tauri/src/lib.rs:4295) 当前 `await` 整个 `process_followup`。streaming 后会让前端 `invoke()` 阻塞 5~30 秒，UI 输入框不能立即清空。

**决策**：改造为「立即 return + spawn task」：

```rust
#[tauri::command]
async fn send_text_question(text: String, app, state) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() { return Err("输入内容不能为空".into()); }

    // CAS 同步检查（立即返回错误，前端能立即捕获）
    if state.is_assistant_processing
       .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
       .is_err() {
        return Err("正在处理中，请稍候".into());
    }

    // 注册 cancel_token（与 cancel_assistant_generation / dismiss 共享）
    let cancel_token = CancellationToken::new();
    *state.assistant_cancel_token.lock().unwrap() = Some(cancel_token.clone());

    // 立即 spawn 后台任务
    let app_clone = app.clone();
    tokio::spawn(async move {
        run_text_followup_loop(app_clone, text, cancel_token).await;
        // task 末尾必须清理 is_processing + assistant_cancel_token
    });

    Ok(())  // <— 立即返回，前端 invoke 不阻塞
}
```

**前端配套**：`handleTextSend` 当前 `try { await invoke } catch { console.error }`，错误只 log 不显示。改造后 CAS 拒绝错误（"正在处理中"）应在输入栏 inline 提示（toast 或边框变红 + 文字提示）。

---

### X. citation index 跨轮处理 + 复制语义双分支（修订 §B / §M）

#### X.1 单 turn 内：按 §B 设计，全 tool_call 扁平化全局递增（不变）

#### X.2 跨轮入历史时：全局重编号 + 改写 markdown

存进 `HistoryRecord.citations` 的列表必须是**整 session 重编号后的扁平 Vec**，且 `assistant_response` 文本里的 `[citation](n:id)` 也要按 remap 表改写。理由：
- 简化前端 chip 查找（单一 Vec + index 直接命中）
- 与「复制全部对话」输出一致
- 实施成本低（regex replace）

实现位置：见 §T.1 伪码中的 `index_remap` 与 `format_conversation_for_copy_with_remap`。

#### X.3 `format_conversation_for_copy` 拆 mode 参数

[`assistant_processor.rs:221`](src-tauri/src/assistant_processor.rs:221) 改签名：

```rust
pub enum CopyMode {
    /// 用于内部存储（emit_conversation_history）/ copy_latest_reply：保留 [citation](n:id) 原文
    Internal,
    /// 用于 copy_full_conversation：[citation](n:id) → [ⁿ](url) 可点 markdown 链接（外部编辑器友好）
    External,
}

pub fn format_conversation_for_copy(turns: &[ConversationTurn], mode: CopyMode) -> String;

// emit_conversation_history 还需要带 remap 的变体
pub fn format_conversation_for_copy_with_remap(
    turns: &[ConversationTurn],
    mode: CopyMode,
    remap: &HashMap<(u32, u32), u32>,
) -> String;
```

调用方更新：
- `emit_conversation_history` → `format_conversation_for_copy_with_remap(turns, Internal, &remap)`
- `copy_latest_reply` → 直接复制最后一轮 `assistant_response`（保留 citation 原文，不变）
- `copy_full_conversation` → `format_conversation_for_copy(turns, External)`

External 模式 regex 替换 `\[citation\]\((\d+):([a-z0-9]+)\)` → `[ⁿ](url)`（n 取 superscript Unicode 字符，url 从 turn.tool_calls 找）；找不到对应 citation 时降级保留原文。

---

### Y. UI 微决策清单（修订 §H / §J / §P）

#### Y.1 「停止生成」按钮位置

**决策**：生成中**替换**底部操作栏的「关闭」按钮位置（变 `Square` 图标 + 红色 `var(--crail)` 文字 + 「停止生成」label）。生成完成后恢复「关闭」按钮。这样操作栏布局不抖动。

「复制全部 / 复制最新」按钮在生成中**禁用**（opacity 0.4 + cursor: not-allowed），不隐藏（避免布局跳动）。

#### Y.2 cancel 后 UI 残留状态

按 §U.2 表格规则：UserBubble 保留 / LoadingBubble→CancelledBubble / 部分流式 AssistantBubble 保留 + inline 标签 / 「重试」按钮。

#### Y.3 MarkdownRenderer transformChildren 元素覆盖

补全 §J 漏列的元素。最终 wrap 列表：

| 组件 | 是否 wrap | 理由 |
|---|---|---|
| `p` / `li` / `strong` / `em` / `blockquote` / `td` / `th` | ✅ | 段落级文本，常见 citation 位置 |
| `h1` / `h2` / `h3` | ✅ | LLM 偶尔在标题里写 [citation] |
| `a` （仅 children，不动 href） | ✅ | 链接文字内可能含 citation |
| `code` (inline) / `pre` 内 | ❌ | 代码字面量，应保持原文 |
| `hr` / `img` | ❌ | 无文本子节点 |

`transformChildren` 必须递归处理 `React.Children.map`，遇到嵌套 React 元素直接返回原节点（不深入），仅对 `string` 类型 child 做 regex 解析。

#### Y.4 AssistantPage Drawer 跨页生命周期

**决策**：Drawer state 用 `useState` 存在 [`AssistantPage.tsx`](src/pages/AssistantPage.tsx) 内部。用户切走 sidebar → AssistantPage 卸载 → Drawer state 自动丢失 → 下次切回默认 closed。理由：抽屉是临时操作面板，跨页保持反而违反预期。

#### Y.5 系统托盘菜单加联网开关

**决策**：在 [`TrayMenuState lib.rs:260`](src-tauri/src/lib.rs:260) 加 `web_search_item: CheckMenuItem`，菜单项 label「联网搜索 (Beta)」。绑定 `AssistantConfig.enable_web_search`（L1 总开关）。

托盘点击走 `mutate_persisted_config` + `emit_config_updated`，复用现有「语句润色」开关同款 pattern（[lib.rs:284-296](src-tauri/src/lib.rs:284) 附近）。`sync_tray_menu_from_config` 函数补一行同步逻辑。

#### Y.6 输入栏布局确认

480px 面板 + 34px globe toggle + 8px gap + textarea + 8px gap + 34px send = textarea 净宽约 **388px**。中文 placeholder「输入追问...」（5 字符）安全。globe toggle 视觉规格按 [`prototype/src/mockups/ResultPanelMockup.tsx`](prototype/src/mockups/ResultPanelMockup.tsx) 实现（开启 steel 蓝、关闭灰底）。

---

### Z. 测试基础设施 + 工程卫生（修订 §S 与原 PRD 缺失项）

#### Z.1 SSE 解析单测：fixture 文件方案

**决策**：不引入 wiremock 等 HTTP mock 库。改用 fixture 字符串 + 直接调用 SSE 解析函数：

```
src-tauri/tests/fixtures/sse/
├── openai_simple.txt          # 普通 content 流
├── openai_tool_call.txt       # 含 delta.tool_calls
├── openai_done_marker.txt     # data: [DONE] 边界 + 空行 keepalive
├── glm_with_thinking.txt      # GLM-4 含 reasoning_content（需忽略）
├── deepseek_reasoning.txt     # DeepSeek 含 reasoning 字段
├── qwen_compatible.txt        # 通义千问 OpenAI 兼容端点
└── error_500.txt              # HTTP 错误包装
```

测试样例：

```rust
#[test]
fn parse_openai_tool_call_stream() {
    let bytes = include_bytes!("fixtures/sse/openai_tool_call.txt");
    let chunks: Vec<StreamChunk> = parse_sse_stream(bytes).collect();
    assert_eq!(chunks.last().unwrap().finish_reason, Some("tool_calls".into()));
    let acc = accumulate_tool_calls(&chunks);
    assert_eq!(acc[0].function_name, "search_web");
}
```

理由：单测 zero 网络依赖，跑得快，CI 友好；fixtures 后续维护即更新文件，不改测试代码。

#### Z.2 reqwest 版本检查（PR1 启动前必做）

检查 [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml) 当前 `reqwest` 版本：
- ≥ 0.12：直接用 `ClientBuilder::read_timeout()`
- < 0.12：手写 `tokio::time::timeout(read_timeout, stream.next()).await` 包装每个 chunk 等待

把这一项写进 PR1 第一个 commit 的 message body（或 PR description 的 "Prep" 段）。

#### Z.3 prototype/ git 卫生

`prototype/` 目录当前 untracked（`?? prototype/`）。用户决策：**不提交 prototype**，但开发期间保留为本地 UI 参考；所有提交计划和 `git add` 必须排除 `prototype/`。任务收尾、原型不再需要时，删除本地 `prototype/` 目录，确保最终 `git status` 不再显示它。

本任务不强制修改 `.gitignore` 来隐藏 `prototype/`，避免把临时本地参考固化为仓库规则；若后续确实需要忽略构建产物，应另行确认后再处理。

#### Z.4 learning 模块隔离声明

确认 AI 助手联网回复**不触发** `learning/coordinator::start_learning_observation`。learning 入口在 [`pipeline/normal.rs`](src-tauri/src/pipeline/normal.rs) 的 dictation 流，不在 assistant 流。

新加的 `process_turn` 实现里加注释明示：
```rust
// 注意：本入口仅服务 AI 助手模式。结果显示在结果面板，不调用 text_inserter，
// 因此不触发 learning 模块。如需扩展到听写模式，请考虑 learning observation 触发时机。
```

---

### RESOLVED ISSUES

#### O-1: `prototype/` 目录的 git 提交策略

当前 `git status` 显示 `?? prototype/`（整目录 untracked）。用户决策：

- `prototype/` 不纳入本任务提交，也不作为交付产物。
- 开发期间可以继续读取 `prototype/` 作为本地 UI 参考。
- 所有提交计划必须显式排除 `prototype/`。
- 任务完成、原型不再需要时，删除本地 `prototype/` 目录，再进行最终收尾。

---

## 最终交付清单

PRD 修订完成（本文件），开发可按 §Q 13-commit 计划顺序实施。`prototype/` 提交策略已确认：仅作本地参考，不纳入提交，收尾时删除。

| 输出 | 位置 |
|---|---|
| 完整 PRD | 本文件（[.trellis/tasks/05-11-ai-assistant-web-search/prd.md](.trellis/tasks/05-11-ai-assistant-web-search/prd.md)） |
| 实施计划 | §Q（13 commit / 4 PR / ~13 工作日） |
| 验收清单 | §R + §R 4 个新增子节 |
| 完成定义 | §S + 3 项新增 DoD |
| 数据流参考图 | §T |
| 副作用矩阵 | §U |
| 入口统一伪码 | §V |
| Streaming 改造 | §W |
| Citation 跨轮处理 | §X |
| UI 微决策 | §Y |
| 测试 + 工程卫生 | §Z |
| 已确认事项 | RESOLVED ISSUES O-1 |
