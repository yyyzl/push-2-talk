# 思考模式的可选项与旧配置

后端 `llm_reasoning::get_reasoning_options` 使用与真实请求相同的配置解析方法，分别解析润色、助手问答和选区处理的最终模型。前端只展示返回的选项，不复制模型解析或能力判断逻辑。

## 当前适配范围

这里描述的是本项目现有请求适配支持的保守子集，不是厂商完整能力表。不要因为某个字段能序列化，就认为模型支持该设置。

| 模型范围 | 新选择可见的选项 |
| --- | --- |
| 已列明的 Qwen 混合思考型号 | 默认、关闭、开启 |
| DeepSeek V4 / V4 Pro / V4 Flash | 默认、关闭、低、高、极高（请求映射为 max） |
| 已列明的 GPT-5、GPT-5.1/5.2/5.4/5.5 及 o 系列 | 默认、低、中、高 |
| Gemini 2.5 Flash / Flash Lite | 默认、关闭 |
| 未验证型号、仅思考型号及未知别名 | 默认 |

精确名单和已知日期快照匹配规则见 `selectable_efforts`。Qwen 旧 max 快照不展示后来才增加的开关。代理服务可以改变模型行为，因此本地匹配不等于云端实测；不能仅通过扩大前缀匹配来宣称支持所有新模型。

## 升级兼容

- 保留七种历史枚举的读写，保留原来的 `reasoning_patch` 及自定义请求体合并优先级。
- 历史配置保存的无效或重复选项仍显示为“已保存”的旧设置，附实际映射提示；不因打开页面、切换模型或读取失败而自动重写。
- 用户主动选新值时才保存。“默认”表示沿用既有配置，也可能继承共享或模式基础配置，不能承诺一定关闭思考。
- 后端读取失败时暂时禁用选择器，保留旧值并允许重试；忽略切换模型前的迟到响应。
- 新增 IPC 只读取传入的工作配置，不持久化数据，不输出凭据。

## 验证依据

Rust 测试覆盖真实模型解析、未知型号、旧值往返及每个非默认选项的请求参数非空且互不重复。TypeScript 测试覆盖旧值保留，浏览器烟测覆盖渲染、切换与失败重试。此次收敛不改变请求映射，未用真实账户逐模型调用云服务。

厂商文档仅用来核对能力差异，不能替代适配和实测：

- [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning)
- [Qwen thinking](https://www.alibabacloud.com/help/en/model-studio/deep-thinking)
- [DeepSeek chat completion](https://api-docs.deepseek.com/api/create-chat-completion/)
- [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai)
