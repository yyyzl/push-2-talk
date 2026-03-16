// 豆包流式 ASR WebSocket 客户端（二进制协议）
use crate::dictionary_utils::entries_to_words;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use futures_util::{SinkExt, StreamExt};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::http, tungstenite::Message};

// 流式输入模式：更适合按住说话、松开拿最终文本的短语音场景
const WEBSOCKET_URL: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream";
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
    latest_text: Arc<Mutex<String>>,
}

enum SessionCommand {
    SendAudio {
        audio: Vec<u8>,
        ack: oneshot::Sender<Result<()>>,
    },
    Finish {
        audio: Vec<u8>,
        ack: oneshot::Sender<Result<()>>,
    },
}

impl DoubaoRealtimeSession {
    pub async fn send_audio_chunk(&mut self, pcm_data: &[i16]) -> Result<()> {
        let bytes: Vec<u8> = pcm_data.iter().flat_map(|&s| s.to_le_bytes()).collect();
        let (ack_tx, ack_rx) = oneshot::channel();
        self.sender
            .send(SessionCommand::SendAudio {
                audio: bytes,
                ack: ack_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("发送音频块失败"))?;
        ack_rx
            .await
            .map_err(|_| anyhow::anyhow!("音频块写入确认失败"))?
    }

    pub async fn finish_audio(&mut self) -> Result<()> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.sender
            .send(SessionCommand::Finish {
                audio: Vec::new(),
                ack: ack_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("发送结束标志失败"))?;
        ack_rx
            .await
            .map_err(|_| anyhow::anyhow!("结束包写入确认失败"))?
    }

    pub async fn finish_audio_with_tail(&mut self, pcm_data: &[i16]) -> Result<()> {
        let bytes: Vec<u8> = pcm_data.iter().flat_map(|&s| s.to_le_bytes()).collect();
        let (ack_tx, ack_rx) = oneshot::channel();
        self.sender
            .send(SessionCommand::Finish {
                audio: bytes,
                ack: ack_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("发送带尾音的结束标志失败"))?;
        ack_rx
            .await
            .map_err(|_| anyhow::anyhow!("带尾音结束包写入确认失败"))?
    }

    pub async fn wait_for_result(&mut self) -> Result<String> {
        self.wait_for_result_with_timeout(Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS))
            .await
    }

    pub async fn wait_for_result_with_timeout(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<String> {
        match timeout(timeout_duration, self.result_receiver.recv()).await {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("通道已关闭")),
            Err(_) => {
                let latest_text = self.latest_text.lock().unwrap().clone();
                if !latest_text.is_empty() {
                    tracing::warn!("豆包等待最终结果超时，返回最近中间结果: {}", latest_text);
                    Ok(latest_text)
                } else {
                    Err(anyhow::anyhow!("转录超时"))
                }
            }
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

        let (ws_stream, response) = connect_async(request).await?;
        if let Some(logid) = response.headers().get("X-Tt-Logid") {
            if let Ok(logid) = logid.to_str() {
                tracing::info!("豆包 WebSocket 建连成功, X-Tt-Logid={}", logid);
            }
        }
        let (mut write, mut read) = ws_stream.split();

        // 发送 Full Client Request
        let request_obj = build_request_object(&self.dictionary);

        let config = serde_json::json!({
            "user": {"uid": &self.app_id},
            "audio": {"format": "pcm", "rate": 16000, "bits": 16, "channel": 1},
            "request": request_obj
        });
        tracing::debug!(
            "豆包 Full Client Request: {}",
            serde_json::to_string_pretty(&config)?
        );
        let msg = build_message(0x1, 0x0, 0, &serde_json::to_vec(&config)?, 0x1)?; // 文档要求 full request 不带 sequence
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
                            let err_text = e.to_string();
                            if err_text.contains("服务器返回错误") {
                                return Err(anyhow::anyhow!(
                                    "豆包 Full Client Request 被服务端拒绝: {}",
                                    err_text
                                ));
                            }
                            tracing::debug!("豆包初始响应（预期无文本）: {}", err_text);
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
        let latest_text = Arc::new(Mutex::new(String::new()));
        let latest_text_for_reader = Arc::clone(&latest_text);

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio { audio, ack } => {
                        // 官方协议示例要求 audio-only request 不带 sequence，并使用 Gzip 压缩。
                        let send_result = match build_message(0x2, 0x0, 0, &audio, 0x1) {
                            Ok(msg) => write
                                .send(Message::Binary(msg.into()))
                                .await
                                .map_err(|e| anyhow::anyhow!("豆包发送音频块失败: {}", e)),
                            Err(e) => Err(e),
                        };
                        let should_break = send_result.is_err();
                        let _ = ack.send(send_result);
                        if should_break {
                            break;
                        }
                    }
                    SessionCommand::Finish { audio, ack } => {
                        tracing::debug!(
                            "豆包发送最后一包音频，flags=0x2, payload={} bytes",
                            audio.len()
                        );
                        let send_result = match build_message(0x2, 0x2, 0, &audio, 0x1) {
                            Ok(msg) => write
                                .send(Message::Binary(msg.into()))
                                .await
                                .map_err(|e| anyhow::anyhow!("豆包发送最后一包音频失败: {}", e)),
                            Err(e) => Err(e),
                        };
                        let should_break = send_result.is_err();
                        let _ = ack.send(send_result);
                        if should_break {
                            break;
                        }
                    }
                }
            }
            tracing::debug!("豆包 WebSocket 写入任务结束");
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
                                    *latest_text_for_reader.lock().unwrap() =
                                        accumulated_text.clone();
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
                                let err_text = e.to_string();
                                if err_text.contains("服务器返回错误") {
                                    tracing::error!("豆包服务端错误帧: {}", err_text);
                                    let _ = result_tx.send(Err(anyhow::anyhow!(err_text))).await;
                                    result_sent = true;
                                    break;
                                }
                                // 中间响应可能没有最终结果，继续等待
                                tracing::debug!("豆包响应解析（非最终结果）: {}", err_text);
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
                            let close_reason = frame
                                .as_ref()
                                .map(|f| format!("code={:?}, reason={}", f.code, f.reason))
                                .unwrap_or_else(|| "无 close frame".to_string());
                            tracing::warn!("豆包连接关闭，无转录结果: {}", close_reason);
                            let _ = result_tx
                                .send(Err(anyhow::anyhow!(
                                    "WebSocket 连接被关闭: {}",
                                    close_reason
                                )))
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
            latest_text,
        })
    }
}

fn build_request_object(dictionary: &[String]) -> serde_json::Value {
    let mut request_obj = serde_json::json!({
        "model_name": "bigmodel",
        "enable_itn": true,
        "enable_punc": true
    });

    // 官方文档将热词直传和 dialog_ctx 上下文列为两类独立格式。
    // 这里先只保留热词直传，避免混合 schema 导致 session failed。
    if !dictionary.is_empty() {
        let purified_words = entries_to_words(dictionary);
        let hotwords: Vec<serde_json::Value> = purified_words
            .iter()
            .map(|w| serde_json::json!({ "word": w }))
            .collect();
        let context = serde_json::json!({ "hotwords": hotwords }).to_string();
        tracing::info!(
            "豆包流式 ASR 词库: {} 个词（热词直传）",
            purified_words.len()
        );
        tracing::debug!("豆包流式 ASR hotwords context={}", context);
        request_obj["corpus"] = serde_json::json!({ "context": context });
    } else {
        tracing::info!("豆包流式 ASR 词库: 未配置");
    }

    request_obj
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
    if matches!(flags, 0x1 | 0x3) {
        msg.extend_from_slice(&sequence.to_be_bytes());
    }
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
        if data.len() < header_size + 8 {
            return Err(anyhow::anyhow!("服务器返回错误，但错误帧过短"));
        }
        let error_code = u32::from_be_bytes([
            data[header_size],
            data[header_size + 1],
            data[header_size + 2],
            data[header_size + 3],
        ]);
        let error_size = u32::from_be_bytes([
            data[header_size + 4],
            data[header_size + 5],
            data[header_size + 6],
            data[header_size + 7],
        ]) as usize;
        if data.len() < header_size + 8 + error_size {
            return Err(anyhow::anyhow!(
                "服务器返回错误，但错误信息不完整: code={}",
                error_code
            ));
        }
        let error_payload = &data[header_size + 8..header_size + 8 + error_size];
        let error_text = if compression == 0x1 {
            let mut decoder = GzDecoder::new(error_payload);
            let mut s = String::new();
            decoder.read_to_string(&mut s)?;
            s
        } else {
            String::from_utf8_lossy(error_payload).into_owned()
        };
        return Err(anyhow::anyhow!(
            "服务器返回错误: code={}, message={}",
            error_code,
            error_text
        ));
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
    let text = extract_result_text(&result);

    // 如果是最后一包或者有文本内容，返回结果
    if is_last || !text.is_empty() {
        return Ok((text, is_last));
    }

    Err(anyhow::anyhow!("中间响应，等待更多数据"))
}

fn extract_result_text(result: &serde_json::Value) -> String {
    if let Some(text) = result["result"]["text"].as_str() {
        return text.to_string();
    }

    result["result"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find_map(|item| item["text"].as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{build_message, build_request_object, parse_response};
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn gzip_bytes(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).expect("gzip write should succeed");
        encoder.finish().expect("gzip finish should succeed")
    }

    #[test]
    fn full_client_request_uses_header_and_payload_only() {
        let payload = br#"{"request":"demo"}"#;
        let compressed = gzip_bytes(payload);

        let msg = build_message(0x1, 0x0, 1, payload, 0x1).expect("message should build");

        assert_eq!(&msg[..4], &[0x11, 0x10, 0x11, 0x00]);
        assert_eq!(msg.len(), 8 + compressed.len());
        assert_eq!(
            u32::from_be_bytes(msg[4..8].try_into().unwrap()) as usize,
            compressed.len()
        );
        assert_eq!(&msg[8..], compressed.as_slice());
    }

    #[test]
    fn last_audio_packet_uses_last_packet_flag_without_sequence() {
        let payload = [1_u8, 2, 3, 4, 5];
        let compressed = gzip_bytes(&payload);

        let msg = build_message(0x2, 0x2, 99, &payload, 0x1).expect("message should build");

        assert_eq!(&msg[..4], &[0x11, 0x22, 0x01, 0x00]);
        assert_eq!(msg.len(), 8 + compressed.len());
        assert_eq!(
            u32::from_be_bytes(msg[4..8].try_into().unwrap()) as usize,
            compressed.len()
        );
        assert_eq!(&msg[8..], compressed.as_slice());
    }

    #[test]
    fn parse_response_includes_server_error_message() {
        let message = br#"{"message":"empty audio"}"#;
        let mut frame = vec![0x11, 0xf0, 0x10, 0x00];
        frame.extend_from_slice(&45000002_u32.to_be_bytes());
        frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
        frame.extend_from_slice(message);

        let err = parse_response(&frame).expect_err("error frame should fail");
        let err_text = err.to_string();
        assert!(err_text.contains("45000002"));
        assert!(err_text.contains("empty audio"));
    }

    #[test]
    fn request_object_uses_hotwords_only_context_schema() {
        let request = build_request_object(&["OpenAI".to_string(), "字节跳动".to_string()]);
        let context = request["corpus"]["context"]
            .as_str()
            .expect("context should be serialized json");
        let context_json: serde_json::Value =
            serde_json::from_str(context).expect("context should be valid json");

        assert!(context_json.get("hotwords").is_some());
        assert!(context_json.get("context_type").is_none());
        assert!(context_json.get("context_data").is_none());
    }
}
