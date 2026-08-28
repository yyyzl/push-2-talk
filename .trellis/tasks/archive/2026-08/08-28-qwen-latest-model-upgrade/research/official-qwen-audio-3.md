# Qwen Audio 3.0 ASR 官方能力与兼容性

## 最新模型

阿里云百炼在 2026-07-30 新增 Qwen-Audio-3.0-ASR-Flash 系列：

- 短音频/非实时：`qwen-audio-3.0-asr-flash`
- 实时流式：`qwen-audio-3.0-asr-flash-streaming`
- 长音频文件：`qwen-audio-3.0-asr-flash-filetrans`

PushToTalk 的短句听写应配对使用前两项；filetrans 不适合当前按键说话交互。

## 相比当前 Qwen3 ASR 的价值

- 支持 30 个语种、汉语七大方言体系以及 20+ 地区口音。
- 强化标点、数字、日期、金额等文本归一化。
- HTTP 与实时模型均支持 Context 上下文增强。
- 支持预编译和请求内即时热词，最多 2,000 个；即时热词可带 1–5 权重，另支持最多 50 个权重 50 的超级热词。
- streaming 默认可返回句级和词级时间戳；当前 `qwen3-asr-flash-realtime` 不返回时间戳。
- 旧 Qwen3 ASR 支持情感识别，新 Qwen Audio 3.0 ASR 不支持；当前项目只读取转写文本，没有消费情感字段，因此这不是现有功能回退。

## 协议变化

### HTTP

新模型仍可通过 DashScope multimodal generation HTTP 端点调用，但消息和参数结构发生变化：

- 音频 content 使用 `type: input_audio` 与 `input_audio.data`。
- 必须提供 `parameters.format`，可提供 `sample_rate`。
- 即时热词使用 `parameters.vocabulary`，语言使用 `language_hints`。
- Context 使用音频消息之前的 `input_text` / `text` 消息。

因此不能只把当前 `qwen3-asr-flash` 常量替换为新型号；请求构造、响应解析和测试都要按新协议适配。

### 实时 WebSocket

新 streaming 模型使用 DashScope Recognition WebSocket 协议：

- 端点路径为 `/api-ws/v1/inference`，而当前客户端使用 `/api-ws/v1/realtime`。
- 建连后发送 `run-task`，收到 `task-started` 后以 Binary frame 发送音频，结束时发送 `finish-task`。
- 上下文和即时热词放在 `run-task` payload 中；运行中可通过 `continue-task` 更新上下文。

这与当前 OpenAI Realtime 风格的 session/audio/transcription 事件不是同一套协议，需要新的适配实现。

## 价格与地域

华北 2（北京）官方原价：

- 新旧短音频 HTTP：均为 `0.00022 元/秒`。
- 新旧实时：均为 `0.00033 元/秒`。

新加坡：

- 新旧短音频 HTTP：均为 `0.00026 元/秒`。
- 新旧实时：均为 `0.00066 元/秒`。

所以从官方原价看，升级没有新增单位成本。官方推荐使用 workspace 专属域名以提高稳定性，但示例仍提供 `dashscope.aliyuncs.com` 入口；是否引入 workspace ID 可与模型升级解耦。

## 官方资料

- [语音识别模型选型](https://help.aliyun.com/zh/model-studio/asr-model)
- [Qwen Audio 3.0 ASR Flash 模型信息](https://help.aliyun.com/zh/model-studio/qwen-audio-3-0-asr-flash)
- [Qwen Audio 3.0 ASR Flash Streaming 模型信息](https://help.aliyun.com/zh/model-studio/qwen-audio-3-0-asr-flash-streaming)
- [非实时 API 参考](https://help.aliyun.com/zh/model-studio/non-real-time-speech-recognition-for-fun-asr-flash)
- [实时客户端事件](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)
- [识别准确率、热词与上下文增强](https://help.aliyun.com/zh/model-studio/improve-asr-accuracy)
- [模型更新记录](https://help.aliyun.com/zh/model-studio/newly-released-models)
- [模型价格](https://help.aliyun.com/zh/model-studio/model-pricing)

