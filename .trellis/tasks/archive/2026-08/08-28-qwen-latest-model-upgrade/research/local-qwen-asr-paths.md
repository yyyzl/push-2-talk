# 本地 Qwen ASR 接入现状

## 结论

当前项目把 Qwen 型号和协议直接固化在两个客户端中，没有独立的 ASR 模型配置。HTTP 与实时链路分别绑定一对 Qwen3 ASR 型号，因此“增加模型选择”不是只改前端下拉框，而是需要把型号、协议和能力绑定成一个受控 profile。

## HTTP 链路

- 文件：`src-tauri/src/asr/http/qwen.rs`
- 固定模型：`qwen3-asr-flash`
- 固定端点：`https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation`
- 音频以 WAV Base64 Data URI 放入 `input.messages`。
- 个人词典与纠错词对被编译后渲染为顿号分隔的 corpus，通过 system content 传给模型。
- 响应解析固定读取 `output.choices[0].message.content[0].text`。
- 客户端构造函数没有 model、endpoint 或 protocol 参数。

## 实时链路

- 文件：`src-tauri/src/asr/realtime/qwen.rs`
- 固定模型：`qwen3-asr-flash-realtime`
- 固定端点：`wss://dashscope.aliyuncs.com/api-ws/v1/realtime`
- 使用 OpenAI Realtime 风格事件，包括 `session.update`、音频 append/commit 和 transcription completed。
- 个人词典通过 `session.input_audio_transcription.corpus.text` 传入。
- 客户端构造函数同样没有 model、endpoint 或 protocol 参数。

## 配置与界面

- `src-tauri/src/config.rs` 的 `AsrConfig` 只有 credentials、provider selection 和 language mode，没有 Qwen 型号字段。
- `use_realtime_asr` 是全局 HTTP/实时模式开关。
- `src/pages/AsrPage.tsx` 只选择 provider；Qwen 型号仅作为只读说明展示。
- `src/constants/index.ts` 的 Qwen 展示值固定为 `qwen3-asr-flash`。
- `src-tauri/src/test_api.rs` 也直接固定 `qwen3-asr-flash`。
- `src-tauri/src/lib.rs` 在启动和实时会话创建时直接构造上述两个客户端。

## 对方案的影响

1. **只替换常量不可行**：新一代 HTTP 请求字段和实时 WebSocket 事件协议都与当前实现不同。
2. **任意模型文本框不合适**：不同型号属于不同协议族，允许用户自由填写会产生“型号合法但协议不匹配”的状态。
3. **如果保留选择，应选择模型系列/profile**：例如 `Qwen Audio 3.0（推荐）` 映射新 HTTP + 新 streaming 客户端，`Qwen3 兼容版` 映射当前 HTTP + realtime 客户端。
4. **现有热词编译器可以复用候选选择逻辑**，但新模型应将结果映射成带权重的 `vocabulary`，而不是继续拼接 corpus 字符串。

## GitNexus 结果

- 刷新后索引：4,834 nodes、9,943 edges、419 clusters、296 flows。
- Qwen HTTP/实时客户端结构的图谱上游风险显示 LOW，但图谱未完整识别 Rust 构造函数在 `lib.rs` 中的调用，因此实现时仍需按实际调用点做跨层测试。

