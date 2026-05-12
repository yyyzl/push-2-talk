# Provider API 核验记录

核验日期：2026-05-11

## Tavily

- 官方端点：`POST https://api.tavily.com/search`
- 鉴权：`Authorization: Bearer <token>`
- `max_results` 默认 5，允许 0-20。
- `include_answer` 默认 `false`；若 PRD 依赖 `answer` 字段，首版请求必须显式传 `include_answer:true`。
- `search_depth: basic` 每次 1 credit；`advanced` 每次 2 credits。
- 免费额度：官方文档显示每月 1,000 API credits。
- 来源：https://docs.tavily.com/documentation/api-reference/endpoint/search、https://docs.tavily.com/documentation/api-credits

## Bocha

- 官方端点：`POST https://api.bochaai.com/v1/web-search`
- 鉴权：`Authorization: Bearer <api-key>`
- 请求示例字段：`query`、`freshness`、`summary:true`、`count`。
- 官方页面示例响应为顶层 `webPages.value[]`，条目字段包含 `name`、`url`、`siteName`、`snippet`、`summary`、`datePublished`。
- 实施时同时对照 kelivo 的 Bocha provider；如果实际响应包裹在 `data` 下，解析层应兼容两种结构并在测试 fixture 中覆盖。
- 来源：https://open.bochaai.com/

## Serper

- 官方搜索端点通常使用 `POST https://google.serper.dev/search`
- 鉴权：`X-API-KEY: <api-key>`
- 结果主体包含 `organic[]`，常见字段为 `title`、`link`、`snippet`。
- 官网标注注册可获得 2,500 free queries，并说明结果为实时 Google SERP。
- PRD 中 `gl` / `hl` / `tbs` 作为可选高级参数保留；实施时用 Serper 当前 API 文档或 kelivo provider 再确认字段名。
- 来源：https://serper.dev/

## SearXNG

- 官方 Search API 支持 `GET /search` 与 `POST /search`。
- JSON 调用示例：`GET {base_url}/search?q=<query>&format=json`。
- `q` 必填；`language`、`time_range`、`categories`、`engines` 等为可选参数。
- JSON format 必须在实例 `settings.yml` 中启用；未启用时请求 `format=json` 会返回 403。
- SearXNG 本身没有统一 API key 机制；若自托管实例放在 Basic Auth 后面，客户端可提供 Basic Auth。
- 来源：https://docs.searxng.org/dev/search_api.html

## 实施结论

- `SearchConfig` 应归属 `AppConfig`，搜索引擎配置与 LLM Provider 配置分离。
- `SearchProviderConfig.api_key` 应为 `Option<String>`，因为 SearXNG 可不需要 key。
- `test_search_provider` 需要区分：凭证错误、endpoint 不可达、SearXNG JSON 未开启、空结果但连接成功。
- Provider fixture 测试应覆盖 Tavily `answer`、Bocha 顶层/`data` 包裹两种响应、Serper `organic[]`、SearXNG 403 JSON 未开启。
