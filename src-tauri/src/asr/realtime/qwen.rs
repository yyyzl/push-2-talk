// qwen3-asr-flash-realtime WebSocket 客户端
// 实时流式语音识别，边录音边发送

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, tungstenite::http, MaybeTlsStream, WebSocketStream};
use tokio::net::TcpStream;

// WebSocket 写入端类型别名
type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

const WEBSOCKET_URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/realtime";
const MODEL: &str = "qwen3-asr-flash-realtime";
const IDLE_TIMEOUT_SECS: u64 = 180; // 3 分钟空闲超时
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 10; // 转录结果等待超时（秒）

/// WebSocket 实时 ASR 会话
pub struct RealtimeSession {
    sender: mpsc::Sender<SessionCommand>,
    result_receiver: mpsc::Receiver<Result<String>>,
}

enum SessionCommand {
    SendAudio(Vec<u8>),  // PCM 数据（已 Base64 编码）
    Commit,              // 提交音频缓冲区
    Close,               // 关闭连接
}

impl RealtimeSession {
    /// 发送音频块（PCM 16-bit, 16kHz, 单声道）
    pub async fn send_audio_chunk(&self, pcm_data: &[i16]) -> Result<()> {
        // 转换为字节数组
        let bytes: Vec<u8> = pcm_data.iter()
            .flat_map(|&sample| sample.to_le_bytes())
            .collect();

        self.sender.send(SessionCommand::SendAudio(bytes)).await
            .map_err(|_| anyhow::anyhow!("发送音频块失败：通道已关闭"))
    }

    /// 提交音频缓冲区（手动 commit 模式）
    pub async fn commit_audio(&self) -> Result<()> {
        self.sender.send(SessionCommand::Commit).await
            .map_err(|_| anyhow::anyhow!("提交音频失败：通道已关闭"))
    }

    /// 等待最终转录结果（带超时）
    pub async fn wait_for_result(&mut self) -> Result<String> {
        match timeout(
            Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
            self.result_receiver.recv()
        ).await {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("等待结果失败：通道已关闭")),
            Err(_) => Err(anyhow::anyhow!("转录超时：{}秒内未收到结果", TRANSCRIPTION_TIMEOUT_SECS)),
        }
    }

    /// 关闭会话
    pub async fn close(&self) -> Result<()> {
        let _ = self.sender.send(SessionCommand::Close).await;
        Ok(())
    }
}

/// WebSocket 连接池（智能连接管理）
pub struct ConnectionPool {
    api_key: String,
    connection: Arc<Mutex<Option<PooledConnection>>>,
    dictionary: Vec<String>,
}

struct PooledConnection {
    last_used: Instant,
}

impl ConnectionPool {
    pub fn new(api_key: String, dictionary: Vec<String>) -> Self {
        Self {
            api_key,
            connection: Arc::new(Mutex::new(None)),
            dictionary,
        }
    }

    /// 获取或创建会话
    pub async fn get_session(&self) -> Result<RealtimeSession> {
        let mut conn_guard = self.connection.lock().await;

        // 检查现有连接是否可用且未超时
        if let Some(ref conn) = *conn_guard {
            if conn.last_used.elapsed() < Duration::from_secs(IDLE_TIMEOUT_SECS) {
                // 复用现有连接 - 但实际上每次转录需要新会话
                // WebSocket realtime API 每次转录是独立的会话
                tracing::info!("连接池中有活跃连接，但 realtime API 需要新会话");
            }
        }

        // 创建新会话
        *conn_guard = None; // 清理旧连接
        drop(conn_guard);

        self.create_new_session().await
    }

    async fn create_new_session(&self) -> Result<RealtimeSession> {
        let url = format!("{}?model={}", WEBSOCKET_URL, MODEL);
        tracing::info!("创建 WebSocket 连接: {}", url);

        // 构建请求
        let request = http::Request::builder()
            .uri(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("OpenAI-Beta", "realtime=v1")
            .header("Host", "dashscope.aliyuncs.com")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
            .body(())?;

        let (ws_stream, _) = connect_async(request).await
            .map_err(|e| anyhow::anyhow!("WebSocket 连接失败: {}", e))?;

        tracing::info!("WebSocket 连接成功");

        let (mut write, mut read) = ws_stream.split();

        // 创建命令通道
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(100);
        // 创建结果通道
        let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);

        // 发送 session.update 配置会话
        // 词库用顿号分隔
        let corpus_text = self.dictionary.join("、");

        let mut input_audio_transcription = serde_json::json!({
            "language": "zh"
        });
        if !corpus_text.is_empty() {
            tracing::info!("Qwen 流式 ASR 词库: {} 个词, corpus={}", self.dictionary.len(), corpus_text);
            input_audio_transcription["corpus"] = serde_json::json!({"text": corpus_text});
        } else {
            tracing::info!("Qwen 流式 ASR 词库: 未配置");
        }

        let session_update = serde_json::json!({
            "event_id": format!("event_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()),
            "type": "session.update",
            "session": {
                "modalities": ["text"],
                "input_audio_format": "pcm",
                "sample_rate": 16000,
                "input_audio_transcription": input_audio_transcription,
                "turn_detection": serde_json::Value::Null  // 禁用 VAD，使用手动 commit
            }
        });

        write.send(Message::Text(session_update.to_string())).await
            .map_err(|e| anyhow::anyhow!("发送 session.update 失败: {}", e))?;

        tracing::info!("已发送 session.update 配置");

        // 启动发送任务
        let write: Arc<Mutex<WsSink>> = Arc::new(Mutex::new(write));
        let write_clone = Arc::clone(&write);

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio(pcm_bytes) => {
                        let encoded = general_purpose::STANDARD.encode(&pcm_bytes);
                        let event = serde_json::json!({
                            "event_id": format!("event_{}", std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis()),
                            "type": "input_audio_buffer.append",
                            "audio": encoded
                        });

                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Text(event.to_string())).await {
                            tracing::error!("发送音频块失败: {}", e);
                            break;
                        }
                    }
                    SessionCommand::Commit => {
                        let event = serde_json::json!({
                            "event_id": format!("event_{}", std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis()),
                            "type": "input_audio_buffer.commit"
                        });

                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Text(event.to_string())).await {
                            tracing::error!("发送 commit 失败: {}", e);
                        }
                        tracing::info!("已发送 input_audio_buffer.commit");
                    }
                    SessionCommand::Close => {
                        let mut w = write_clone.lock().await;
                        let _ = w.close().await;
                        break;
                    }
                }
            }
        });

        // 启动接收任务
        let corpus_for_check = corpus_text.clone();
        tokio::spawn(async move {
            let mut final_text = String::new();
            let mut has_result = false;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(data) => {
                                let event_type = data["type"].as_str().unwrap_or("");
                                tracing::debug!("收到事件: {}", event_type);

                                match event_type {
                                    "session.created" | "session.updated" => {
                                        tracing::info!("会话已创建/更新");
                                    }
                                    "input_audio_buffer.committed" => {
                                        tracing::info!("音频缓冲区已提交");
                                    }
                                    "conversation.item.input_audio_transcription.completed" => {
                                        // 转录完成
                                        if let Some(transcript) = data["transcript"].as_str() {
                                            final_text = transcript.to_string();
                                            has_result = true;
                                            tracing::info!("转录完成: {}", final_text);
                                        }
                                    }
                                    "response.audio_transcript.delta" => {
                                        // 增量转录结果
                                        if let Some(delta) = data["delta"].as_str() {
                                            final_text.push_str(delta);
                                            tracing::debug!("增量转录: {}", delta);
                                        }
                                    }
                                    "response.audio_transcript.done" => {
                                        // 转录完成
                                        if let Some(transcript) = data["transcript"].as_str() {
                                            final_text = transcript.to_string();
                                        }
                                        has_result = true;
                                        tracing::info!("转录完成: {}", final_text);
                                    }
                                    "response.done" => {
                                        // 响应完成，发送结果
                                        has_result = true;
                                    }
                                    "error" => {
                                        let error_msg = data["error"]["message"]
                                            .as_str()
                                            .unwrap_or("未知错误");
                                        tracing::error!("API 错误: {}", error_msg);
                                        let _ = result_tx.send(Err(anyhow::anyhow!("API 错误: {}", error_msg))).await;
                                        return;
                                    }
                                    _ => {
                                        tracing::debug!("未处理的事件类型: {}", event_type);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("解析消息失败: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        tracing::info!("WebSocket 连接关闭");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket 错误: {}", e);
                        let _ = result_tx.send(Err(anyhow::anyhow!("WebSocket 错误: {}", e))).await;
                        return;
                    }
                    _ => {}
                }

                // 如果已有结果，发送并退出
                if has_result && !final_text.is_empty() {
                    // 检测词库回显（千问特有问题：录音为空时返回词库内容）
                    // 必须在过滤标点之前比较，因为词库用顿号分隔
                    if !corpus_for_check.is_empty() && final_text == corpus_for_check {
                        tracing::warn!("检测到词库回显，过滤无效结果");
                        let _ = result_tx.send(Err(anyhow::anyhow!("录音无效，已跳过"))).await;
                        break;
                    }

                    // 实时模式下删除所有标点符号
                    let punctuation = ['。', '，', '！', '？', '、', '；', '：', '"', '"',
                                       '.', ',', '!', '?', ';', ':', '"', '\'',
                                       '（', '）', '(', ')', '【', '】', '[', ']',
                                       '《', '》', '<', '>', '—', '…', '·',
                                       '\u{2018}', '\u{2019}'];  // 中文单引号 ' '
                    final_text = final_text.chars()
                        .filter(|c| !punctuation.contains(c))
                        .collect();

                    let _ = result_tx.send(Ok(final_text.clone())).await;
                    break;
                }
            }

            // 如果循环结束但没有发送结果
            if !has_result {
                let _ = result_tx.send(Err(anyhow::anyhow!("未收到转录结果"))).await;
            }
        });

        Ok(RealtimeSession {
            sender: cmd_tx,
            result_receiver: result_rx,
        })
    }
}

/// 简化的实时转录客户端
pub struct QwenRealtimeClient {
    pool: ConnectionPool,
}

impl QwenRealtimeClient {
    pub fn new(api_key: String, dictionary: Vec<String>) -> Self {
        Self {
            pool: ConnectionPool::new(api_key, dictionary),
        }
    }

    /// 创建新的转录会话
    pub async fn start_session(&self) -> Result<RealtimeSession> {
        self.pool.get_session().await
    }
}
