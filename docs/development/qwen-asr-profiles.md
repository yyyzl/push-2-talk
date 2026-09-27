# 千问模型选择与历史配置兼容

## 选择模型

`src/shared/qwen-models.json` 是前后端共享的模型目录。每条记录包含精确模型 ID、显示名称、识别模式和请求协议。设置页分别选择“松开后识别”和“边说边识别”；首页显示当前录音模式实际使用的模型。

配置新增可选的 `asr_config.qwen_models`：

```json
{
  "qwen_profile": "qwen3_legacy",
  "qwen_models": {
    "http": "qwen3-asr-flash-2026-02-10",
    "realtime": "qwen-audio-3.1-asr-flash-message"
  }
}
```

- 有显式模型 ID 时使用该 ID；缺少某一模式的 ID 时，沿用原 `qwen_profile` 对应的模型。
- 原 main 没有 `qwen_profile`，升级时保留原 Qwen3 模型。新安装仍可默认 Audio 3.1。两种默认行为不能混用。
- `qwen_audio3` 继续作为历史 3.0 别名读取，保存为 `qwen_audio_3`。
- 3.0、3.1、Qwen3 的既有显式选择均保留。切换一个模式不覆盖另一个模式、服务商或凭据。
- 未识别的模型 ID 原样保存，界面显示待修复选项；仅使用该模型时拒绝启动，提供明确错误。其他服务商不受闲置模型影响。
- 不在请求失败时悄悄更换模型。用户原本开启的跨服务商备用策略仍然保留。
- `filetrans` 是异步长文件任务协议，不应塞进当前录音或实时接口的模型列表。

## 协议契约

| 模型族 | 模式 | 协议 |
|---|---|---|
| Audio 3.1 / 3.0 Flash | HTTP | multimodal-generation；`input_audio` Data URI；WAV；字符串采样率 `16000`；inline vocabulary |
| Qwen3 Flash（含日期版本） | HTTP | 原 `audio` 内容和 system corpus；`output.choices` 返回文本 |
| Audio 3.1 / 3.0 Streaming | 实时 | `/inference`；run-task → task-started → PCM 二进制 → finish-task → task-finished |
| Audio 3.1 Message | 实时 | 同样使用任务协议；自动识别语言，不发送 Streaming 专有的 `language_hints` |
| Qwen3 Realtime（含日期版本） | 实时 | `/realtime?model=精确ID`；session.update；Base64 append；commit |

Qwen3 实时自动语言必须省略 `language`，不能发送字面量 `auto`，后者会被云端拒绝。指定中文时发送 `zh`。任务协议的中间句子不参与最终拼接；等待任务完成后返回全部最终句子。

当前使用阿里云北京地域。已有 `dashscope.aliyuncs.com` 地址仍被官方支持；本次不改用户地域或计费设置。

## 升级和写入契约

- SenseVoice 的历史实时标记不会改变其 HTTP 能力；豆包输入法固定使用流式模式。
- 新增搜索配置默认不开启助手联网；LLM 推理参数、自定义请求体、分模式助手覆盖均为可选字段。
- 旧 TNL 配置缺少口语清洗模式时使用 `off`，不对旧用户自动开启新的文本改写。
- 根级 API Key、旧平铺 LLM 配置、单快捷键仍可迁移。不能因一个字段解析失败而返回整套默认配置。
- 已有 canonical 文件无法解析时返回错误并保持文件原样；仅 canonical 缺失时读取旧 `.json.bak`。
- 保存使用独立临时文件、sync 和平台原子替换；不先移动/删除原配置。目标是目录时拒绝写入。
- 旧 localStorage 仅能补充没有任何后端凭据的安装。豆包输入法三项凭据中的任意一项存在，均不能被旧缓存覆盖。
- 前端加载失败必须显示错误并向初始化流程传播，不能当成加载成功后自动保存默认状态。
- 用户清空的预设列表、已有自定义助手提示词必须保留。缺失一个提示词只补全该字段。

## 验证入口

```sh
npm run test:ts
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo run --manifest-path src-tauri/Cargo.toml --bin test_api -- \
  --asr qwen --mode realtime --model qwen-audio-3.1-asr-flash-message --file sample.wav
```

- `test_api` 只从进程环境 `DASHSCOPE_API_KEY` 读取凭据，真实调用正式客户端，单模型超时 45 秒。测试音频为 16 kHz 单声道 PCM16 WAV。
- HTTP/WebSocket 本地服务测试遍历全部目录模型，验证实际传输、认证、模型、参数、结束消息和响应解析。
- `config/compatibility_tests.rs` 覆盖升级、原子写入、损坏配置、旧备份和连续保存。可通过 `PTT_COMPAT_CONFIG_PATH` 显式运行被忽略的本地配置副本测试；绝不写入源文件，断言不输出密钥。
- `npm run dev` 后打开 `/tests/ui/asr-model-selection.html` 可重复验收正式 `AsrPage` 的选择、保存、重载、失败回滚及运行锁定。该隔离页面使用虚构凭据，保存到专用 localStorage，不连接真实 Tauri 配置。

## 官方依据（2026-09-28 核查）

- [模型目录](https://help.aliyun.com/zh/model-studio/model-list-speech-recognition/)
- [Audio 3.x HTTP](https://help.aliyun.com/zh/model-studio/fun-asr-flash-recorded-speech-recognition-http-api)
- [Streaming 客户端事件](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)
- [Message 客户端事件](https://help.aliyun.com/zh/model-studio/qwen-asr-message-client-events)、[服务端事件](https://help.aliyun.com/zh/model-studio/qwen-asr-message-server-events)
- [Qwen3 实时客户端事件](https://help.aliyun.com/zh/model-studio/qwen-asr-realtime-client-events)
