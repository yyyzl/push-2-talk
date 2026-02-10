// 豆包流式 ASR WebSocket 客户端（二进制协议）
use crate::dictionary_utils::entries_to_words;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use futures_util::{SinkExt, StreamExt};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::http, tungstenite::Message};

// 双向流式模式（优化版本）
const WEBSOCKET_URL: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
const RESOURCE_ID: &str = "volc.seedasr.sauc.duration";
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 6;

/// 生成随机的 Sec-WebSocket-Key
fn generate_websocket_key() -> String {
    // 使用 UUID 生成 16 字节随机数据
    let uuid_bytes = uuid::Uuid::new_v4();
    general_purpose::STANDARD.encode(uuid_bytes.as_bytes())
}

pub struct DoubaoRealtimeSession {
    sender: mpsc::Sender<SessionCommand>,
    result_receiver: mpsc::Receiver<Result<String>>,
}

enum SessionCommand {
    SendAudio(Vec<u8>),
    Finish,
}

impl DoubaoRealtimeSession {
    pub async fn send_audio_chunk(&mut self, pcm_data: &[i16]) -> Result<()> {
        let bytes: Vec<u8> = pcm_data.iter().flat_map(|&s| s.to_le_bytes()).collect();
        self.sender
            .send(SessionCommand::SendAudio(bytes))
            .await
            .map_err(|_| anyhow::anyhow!("发送音频块失败"))
    }

    pub async fn finish_audio(&mut self) -> Result<()> {
        self.sender
            .send(SessionCommand::Finish)
            .await
            .map_err(|_| anyhow::anyhow!("发送结束标志失败"))
    }

    pub async fn wait_for_result(&mut self) -> Result<String> {
        match timeout(
            Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
            self.result_receiver.recv(),
        )
        .await
        {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("通道已关闭")),
            Err(_) => Err(anyhow::anyhow!("转录超时")),
        }
    }
}

pub struct DoubaoRealtimeClient {
    app_id: String,
    access_key: String,
    dictionary: Vec<String>,
}

impl DoubaoRealtimeClient {
    pub fn new(app_id: String, access_key: String, dictionary: Vec<String>) -> Self {
        Self {
            app_id,
            access_key,
            dictionary,
        }
    }

    pub async fn start_session(&self) -> Result<DoubaoRealtimeSession> {
        let websocket_key = generate_websocket_key();
        let request_id = uuid::Uuid::new_v4().to_string();

        let request = http::Request::builder()
            .uri(WEBSOCKET_URL)
            .header("Host", "openspeech.bytedance.com")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", &websocket_key)
            .header("X-Api-App-Key", &self.app_id)
            .header("X-Api-Access-Key", &self.access_key)
            .header("X-Api-Resource-Id", RESOURCE_ID)
            .header("X-Api-Connect-Id", &request_id)
            .body(())?;

        let (ws_stream, _) = connect_async(request).await?;
        let (mut write, mut read) = ws_stream.split();

        // 发送 Full Client Request
        let mut request_obj = serde_json::json!({
        "model_name": "bigmodel",
        "enable_nonstream":true, //开启二遍识别
         "enable_itn": true, //文本规范化
         "enable_punc": true, //启用标点
         "enable_ddc": true, //语义顺滑
        //  "show_speech_rate":true //语速
        });

        // 添加词库支持和对话上下文
        {
            let mut context_obj = serde_json::json!({
                "context_type": "dialog_ctx",
                "context_data": [
                    // 大模型的约束，但是效果存疑
                    {"text": "你好，请问有什么可以帮您的"},
                    {"text": "豆包语音识别真的不错呀"},
                    {"text": "当前聊天的场景是日常聊天，因此保留语气词，去除尾部句号"},
                    ]
            });

            if !self.dictionary.is_empty() {
                let purified_words = entries_to_words(&self.dictionary);
                let hotwords: Vec<serde_json::Value> = purified_words
                    .iter()
                    .map(|w| serde_json::json!({"word": w}))
                    .collect();
                context_obj["hotwords"] = serde_json::json!(hotwords);
                tracing::info!("豆包流式 ASR 词库: {} 个词（已提纯）", purified_words.len());
            } else {
                tracing::info!("豆包流式 ASR 词库: 未配置");
            }

            let context = context_obj.to_string();
            tracing::debug!("豆包流式 ASR context={}", context);
            request_obj["corpus"] = serde_json::json!({"context": context});
        }

        let config = serde_json::json!({
            "user": {"uid": &self.app_id},
            "audio": {"format": "pcm", "rate": 16000, "bits": 16, "channel": 1},
            "request": request_obj
        });
        tracing::debug!(
            "豆包 Full Client Request: {}",
            serde_json::to_string_pretty(&config)?
        );
        let msg = build_message(0x1, 0x1, 1, &serde_json::to_vec(&config)?, 0x1)?; // Gzip 压缩
        write.send(Message::Binary(msg.clone().into())).await?;
        tracing::debug!("豆包 Full Client Request 已发送: {} bytes", msg.len());

        // 等待 Full Client Request 的响应
        if let Some(response) = read.next().await {
            match response {
                Ok(Message::Binary(data)) => {
                    tracing::debug!("豆包 Full Client Request 响应: {} bytes", data.len());
                    // 解析响应检查是否成功（适配新的返回类型）
                    match parse_response(&data) {
                        Ok((text, _is_last)) => {
                            if !text.is_empty() {
                                tracing::debug!("豆包初始响应包含文本（意外）: {}", text);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("豆包初始响应（预期无文本）: {}", e);
                        }
                    }
                }
                Ok(other) => {
                    tracing::warn!("豆包 Full Client Request 收到非二进制响应: {:?}", other);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("豆包 Full Client Request 响应错误: {}", e));
                }
            }
        }

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(100);
        let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);

        let mut sequence = 1i32;
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio(audio) => {
                        sequence += 1;
                        // 音频数据使用无压缩 (0x0) 以提高性能
                        if let Ok(msg) = build_message(0x2, 0x1, sequence, &audio, 0x0) {
                            if let Err(e) = write.send(Message::Binary(msg.into())).await {
                                tracing::error!("豆包发送音频块失败: {}", e);
                                break;
                            }
                        }
                    }
                    SessionCommand::Finish => {
                        // 关键修复: 先递增序列号，再取反，确保结束包占用新的序列号
                        sequence += 1;
                        let last_seq = -sequence;
                        tracing::debug!("豆包发送结束标志，sequence={}", last_seq);
                        // 结束包必须使用无压缩 (0x0)，payload 长度严格为 0
                        if let Ok(msg) = build_message(0x2, 0x3, last_seq, &[], 0x0) {
                            if let Err(e) = write.send(Message::Binary(msg.into())).await {
                                tracing::error!("豆包发送结束标志失败: {}", e);
                            }
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            let mut accumulated_text = String::new();
            let mut result_sent = false;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        tracing::debug!("豆包 WebSocket 收到二进制消息: {} bytes", data.len());
                        match parse_response(&data) {
                            Ok((text, is_final)) => {
                                if !text.is_empty() {
                                    accumulated_text = text; // 更新为最新文本
                                    tracing::debug!("豆包累积文本: {}", accumulated_text);
                                }
                                if is_final {
                                    let final_text = if accumulated_text.is_empty() {
                                        String::new()
                                    } else {
                                        accumulated_text.clone()
                                    };
                                    tracing::info!("豆包流式转录结果（最终包）: {}", final_text);
                                    let _ = result_tx.send(Ok(final_text)).await;
                                    result_sent = true;
                                    break;
                                }
                            }
                            Err(e) => {
                                // 中间响应可能没有最终结果，继续等待
                                tracing::debug!("豆包响应解析（非最终结果）: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        tracing::warn!("豆包 WebSocket 连接关闭: {:?}", frame);
                        // 连接关闭时返回已累积的文本
                        if !accumulated_text.is_empty() {
                            tracing::info!("豆包连接关闭，返回累积文本: {}", accumulated_text);
                            let _ = result_tx.send(Ok(accumulated_text.clone())).await;
                            result_sent = true;
                        } else {
                            tracing::warn!("豆包连接关闭，无转录结果");
                            let _ = result_tx
                                .send(Err(anyhow::anyhow!("WebSocket 连接被关闭")))
                                .await;
                            result_sent = true;
                        }
                        break;
                    }
                    Ok(other) => {
                        tracing::debug!("豆包 WebSocket 收到其他消息类型: {:?}", other);
                    }
                    Err(e) => {
                        tracing::error!("豆包 WebSocket 接收错误: {}", e);
                        let _ = result_tx
                            .send(Err(anyhow::anyhow!("WebSocket 错误: {}", e)))
                            .await;
                        result_sent = true;
                        break;
                    }
                }
            }

            // 关键修复：循环正常退出时（read.next() 返回 None），确保发送结果
            if !result_sent {
                if !accumulated_text.is_empty() {
                    tracing::info!("豆包连接结束，返回累积文本: {}", accumulated_text);
                    let _ = result_tx.send(Ok(accumulated_text)).await;
                } else {
                    tracing::warn!("豆包连接结束，无转录结果");
                    let _ = result_tx
                        .send(Err(anyhow::anyhow!("WebSocket 连接结束，无转录结果")))
                        .await;
                }
            }
            tracing::debug!("豆包 WebSocket 接收任务结束");
        });

        Ok(DoubaoRealtimeSession {
            sender: cmd_tx,
            result_receiver: result_rx,
        })
    }
}

fn build_message(
    msg_type: u8,
    flags: u8,
    sequence: i32,
    payload: &[u8],
    compression_type: u8, // 0x0=无压缩, 0x1=Gzip
) -> Result<Vec<u8>> {
    // 根据压缩类型处理 payload
    let final_payload = if compression_type == 0x1 {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload)?;
        encoder.finish()?
    } else {
        payload.to_vec() // 不压缩
    };

    // 序列化方法：full client request (0x1) 用 JSON，audio only (0x2) 用 none
    let serialization = if msg_type == 0x1 { 0x1 } else { 0x0 };

    let mut msg = vec![
        0x11,                                    // Protocol version 1, header size 1 (4 bytes)
        (msg_type << 4) | flags,                 // Message type + flags
        (serialization << 4) | compression_type, // Serialization + compression
        0x00,                                    // Reserved
    ];
    msg.extend_from_slice(&sequence.to_be_bytes());
    msg.extend_from_slice(&(final_payload.len() as u32).to_be_bytes());
    msg.extend_from_slice(&final_payload);
    Ok(msg)
}

fn parse_response(data: &[u8]) -> Result<(String, bool)> {
    if data.len() < 4 {
        return Err(anyhow::anyhow!("响应太短: {} bytes", data.len()));
    }

    // 解析 header
    let header_size = (data[0] & 0x0f) as usize * 4;
    let message_type = data[1] >> 4;
    let message_flags = data[1] & 0x0f;
    let _serialization = data[2] >> 4;
    let compression = data[2] & 0x0f;

    tracing::debug!(
        "豆包响应 header: size={}, type={:#x}, flags={:#x}, compression={}",
        header_size,
        message_type,
        message_flags,
        compression
    );

    // 检查是否是错误响应
    if message_type == 0xf {
        let error_code = if data.len() >= header_size + 4 {
            u32::from_be_bytes([
                data[header_size],
                data[header_size + 1],
                data[header_size + 2],
                data[header_size + 3],
            ])
        } else {
            0
        };
        return Err(anyhow::anyhow!("服务器返回错误: code={}", error_code));
    }

    // 跳过 header，检查是否有 sequence
    let mut offset = header_size;

    // 如果 flags 包含 sequence (0x01 或 0x03)
    if message_flags & 0x01 != 0 {
        if data.len() < offset + 4 {
            return Err(anyhow::anyhow!("数据不足以包含 sequence"));
        }
        let sequence = i32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        tracing::debug!("豆包响应 sequence: {}", sequence);
        offset += 4;
    }

    // 读取 payload size
    if data.len() < offset + 4 {
        return Err(anyhow::anyhow!("数据不足以包含 payload size"));
    }
    let payload_size = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;

    if data.len() < offset + payload_size {
        return Err(anyhow::anyhow!(
            "数据不完整: 需要 {} bytes，实际 {} bytes",
            offset + payload_size,
            data.len()
        ));
    }

    // 解压 payload
    let payload_data = &data[offset..offset + payload_size];
    let json_str = if compression == 0x1 {
        // Gzip 压缩
        let mut decoder = GzDecoder::new(payload_data);
        let mut s = String::new();
        decoder.read_to_string(&mut s)?;
        s
    } else {
        // 无压缩
        String::from_utf8(payload_data.to_vec())?
    };

    tracing::debug!("豆包响应 JSON: {}", json_str);

    let result: serde_json::Value = serde_json::from_str(&json_str)?;

    // 检查是否是最后一包的标志 (flags 0x02 或 0x03 表示最后一包)
    let is_last = message_flags & 0x02 != 0;

    // 提取文本结果（可能为空）
    let text = result["result"]["text"].as_str().unwrap_or("").to_string();

    // 如果是最后一包或者有文本内容，返回结果
    if is_last || !text.is_empty() {
        return Ok((text, is_last));
    }

    Err(anyhow::anyhow!("中间响应，等待更多数据"))
}
