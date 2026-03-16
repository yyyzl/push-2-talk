# Changelog

## 2026-03-16

### fix: 稳定豆包 realtime WebSocket 链路

- 将豆包实时识别链路从 `bigmodel_async` 调整为更符合当前按住说话场景的 `bigmodel_nostream`。
- 对齐豆包 WebSocket 二进制协议，修正 full request、audio-only request 和最后一包 finish 的发包语义。
- 为豆包 realtime 保留静音块和尾音，避免本地 VAD 破坏服务端分句与收尾。
- 将 `corpus` 收缩为热词直传，移除混合 `dialog_ctx` 的高风险请求形状。
- 增强错误诊断，透传 `X-Tt-Logid`、服务端 error frame 和 close reason。
- 修复音频发送确认逻辑，只有在音频块真正写入 WebSocket 后才视为发送完成，解决 `45000081 waiting next packet timeout`。

### verification

- `cargo test asr::realtime::doubao::tests --lib`
- `cargo test streaming_recorder::tests --lib`
- `cargo check`
