// 千问 ASR WebSocket 客户端
// 同时支持 Qwen Audio 3.1 / 3.0 协议与 Qwen3 旧版兼容协议

use crate::config::{AsrLanguageMode, QwenAsrProfile};
use crate::personalization::hotword_compiler::{
    compile_asr_pack_with_correction_pairs, render_qwen_corpus_text, QWEN_REALTIME_MAX_HOTWORDS,
};
use crate::personalization::CorrectionPair;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async, tungstenite::http, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

// WebSocket 写入端类型别名
type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

const QWEN_AUDIO_3_WEBSOCKET_URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
const QWEN3_LEGACY_WEBSOCKET_URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/realtime";
const IDLE_TIMEOUT_SECS: u64 = 180; // 3 分钟空闲超时
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 10; // 转录结果等待超时（秒）
const TASK_START_TIMEOUT_SECS: u64 = 10;

fn asr_language_code(language_mode: AsrLanguageMode) -> &'static str {
    match language_mode {
        AsrLanguageMode::Auto => "auto",
        AsrLanguageMode::Zh => "zh",
    }
}

#[cfg(test)]
fn build_input_audio_transcription(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> serde_json::Value {
    build_input_audio_transcription_with_pairs(language_mode, dictionary, &[])
}

fn build_input_audio_transcription_with_pairs(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> serde_json::Value {
    let mut input_audio_transcription = serde_json::json!({
        "language": asr_language_code(language_mode)
    });

    let hotword_pack = compile_asr_pack_with_correction_pairs(
        dictionary,
        correction_pairs,
        QWEN_REALTIME_MAX_HOTWORDS,
    );
    let corpus_text = render_qwen_corpus_text(&hotword_pack);

    if !corpus_text.is_empty() {
        tracing::info!(
            "Qwen 流式 ASR 词库: {} 个词（已编译）, corpus={}",
            hotword_pack.words.len(),
            corpus_text
        );
        input_audio_transcription["corpus"] = serde_json::json!({"text": corpus_text});
    } else {
        tracing::info!("Qwen 流式 ASR 词库: 未配置");
    }

    input_audio_transcription
}

fn build_qwen_audio_3_run_task(
    profile: QwenAsrProfile,
    task_id: &str,
    language_mode: AsrLanguageMode,
    dictionary: &[String],
    correction_pairs: &[CorrectionPair],
) -> serde_json::Value {
    let hotword_pack = compile_asr_pack_with_correction_pairs(
        dictionary,
        correction_pairs,
        QWEN_REALTIME_MAX_HOTWORDS,
    );
    let mut parameters = serde_json::json!({
        "format": "pcm",
        "sample_rate": 16000
    });

    if !hotword_pack.words.is_empty() {
        let vocabulary = hotword_pack
            .words
            .iter()
            .map(|hotword| (hotword.text.clone(), serde_json::json!(4)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        parameters["vocabulary"] = serde_json::Value::Object(vocabulary);
        tracing::info!(
            "Qwen Audio 3.x 流式 ASR 词库: {} 个词（已编译）",
            hotword_pack.words.len()
        );
    } else {
        tracing::info!("Qwen Audio 3.x 流式 ASR 词库: 未配置");
    }

    if language_mode == AsrLanguageMode::Zh {
        parameters["language_hints"] = serde_json::json!(["zh"]);
    }

    serde_json::json!({
        "header": {
            "action": "run-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": profile.realtime_model(),
            "parameters": parameters,
            "input": {}
        }
    })
}

fn build_qwen_audio_3_finish_task(task_id: &str) -> serde_json::Value {
    serde_json::json!({
        "header": {
            "action": "finish-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "input": {}
        }
    })
}

#[derive(Debug, PartialEq, Eq)]
enum QwenAudio3ServerEvent {
    TaskStarted,
    FinalSentence(String),
    TaskFinished,
    Failed(String),
    Ignore,
}

fn parse_qwen_audio_3_server_event(data: &serde_json::Value) -> QwenAudio3ServerEvent {
    match data["header"]["event"].as_str().unwrap_or("") {
        "task-started" => QwenAudio3ServerEvent::TaskStarted,
        "result-generated" => {
            let sentence = &data["payload"]["output"]["sentence"];
            if sentence["heartbeat"].as_bool().unwrap_or(false)
                || !sentence["sentence_end"].as_bool().unwrap_or(false)
            {
                return QwenAudio3ServerEvent::Ignore;
            }

            sentence["text"]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(|text| QwenAudio3ServerEvent::FinalSentence(text.to_string()))
                .unwrap_or(QwenAudio3ServerEvent::Ignore)
        }
        "task-finished" => QwenAudio3ServerEvent::TaskFinished,
        "task-failed" => {
            let code = data["header"]["error_code"]
                .as_str()
                .unwrap_or("UNKNOWN_ERROR");
            let message = data["header"]["error_message"]
                .as_str()
                .unwrap_or("未知错误");
            QwenAudio3ServerEvent::Failed(format!("{}: {}", code, message))
        }
        _ => QwenAudio3ServerEvent::Ignore,
    }
}

/// WebSocket 实时 ASR 会话
pub struct RealtimeSession {
    sender: mpsc::Sender<SessionCommand>,
    result_receiver: mpsc::Receiver<Result<String>>,
}

enum SessionCommand {
    SendAudio(Vec<u8>), // PCM 字节；协议层决定二进制直传或 Base64 封装
    Commit,             // 提交音频缓冲区
    Close,              // 关闭连接
}

impl RealtimeSession {
    /// 发送音频块（PCM 16-bit, 16kHz, 单声道）
    pub async fn send_audio_chunk(&self, pcm_data: &[i16]) -> Result<()> {
        // 转换为字节数组
        let bytes: Vec<u8> = pcm_data
            .iter()
            .flat_map(|&sample| sample.to_le_bytes())
            .collect();

        self.sender
            .send(SessionCommand::SendAudio(bytes))
            .await
            .map_err(|_| anyhow::anyhow!("发送音频块失败：通道已关闭"))
    }

    /// 提交音频缓冲区（手动 commit 模式）
    pub async fn commit_audio(&self) -> Result<()> {
        self.sender
            .send(SessionCommand::Commit)
            .await
            .map_err(|_| anyhow::anyhow!("提交音频失败：通道已关闭"))
    }

    /// 等待最终转录结果（带超时）
    pub async fn wait_for_result(&mut self) -> Result<String> {
        match timeout(
            Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
            self.result_receiver.recv(),
        )
        .await
        {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("等待结果失败：通道已关闭")),
            Err(_) => Err(anyhow::anyhow!(
                "转录超时：{}秒内未收到结果",
                TRANSCRIPTION_TIMEOUT_SECS
            )),
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
    correction_pairs: Vec<CorrectionPair>,
    language_mode: AsrLanguageMode,
    profile: QwenAsrProfile,
}

struct PooledConnection {
    last_used: Instant,
}

impl ConnectionPool {
    pub fn new(api_key: String, dictionary: Vec<String>, language_mode: AsrLanguageMode) -> Self {
        Self::new_with_correction_pairs(api_key, dictionary, Vec::new(), language_mode)
    }

    pub fn new_with_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
    ) -> Self {
        Self::new_with_profile_and_correction_pairs(
            api_key,
            dictionary,
            correction_pairs,
            language_mode,
            QwenAsrProfile::default(),
        )
    }

    pub fn new_with_profile_and_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
        profile: QwenAsrProfile,
    ) -> Self {
        Self {
            api_key,
            connection: Arc::new(Mutex::new(None)),
            dictionary,
            correction_pairs,
            language_mode,
            profile,
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
        let profile = self.profile;
        let url = match profile {
            QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                QWEN_AUDIO_3_WEBSOCKET_URL.to_string()
            }
            QwenAsrProfile::Qwen3Legacy => {
                format!(
                    "{}?model={}",
                    QWEN3_LEGACY_WEBSOCKET_URL,
                    profile.realtime_model()
                )
            }
        };
        tracing::info!(
            "创建 WebSocket 连接: {}, 模型: {}",
            url,
            profile.realtime_model()
        );

        let mut request_builder = http::Request::builder()
            .uri(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Host", "dashscope.aliyuncs.com")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            );
        if profile == QwenAsrProfile::Qwen3Legacy {
            request_builder = request_builder.header("OpenAI-Beta", "realtime=v1");
        }
        let request = request_builder.body(())?;

        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket 连接失败: {}", e))?;

        tracing::info!("WebSocket 连接成功");

        let (mut write, mut read) = ws_stream.split();

        // 创建命令通道
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(100);
        // 创建结果通道
        let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);

        let corpus_for_check = render_qwen_corpus_text(&compile_asr_pack_with_correction_pairs(
            &self.dictionary,
            &self.correction_pairs,
            QWEN_REALTIME_MAX_HOTWORDS,
        ));
        let task_id = match profile {
            QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                Some(uuid::Uuid::new_v4().to_string())
            }
            QwenAsrProfile::Qwen3Legacy => None,
        };

        match profile {
            QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                let run_task = build_qwen_audio_3_run_task(
                    profile,
                    task_id.as_deref().expect("最新版协议必须有 task_id"),
                    self.language_mode,
                    &self.dictionary,
                    &self.correction_pairs,
                );
                write
                    .send(Message::Text(run_task.to_string()))
                    .await
                    .map_err(|e| anyhow::anyhow!("发送 run-task 失败: {}", e))?;

                timeout(Duration::from_secs(TASK_START_TIMEOUT_SECS), async {
                    loop {
                        let message = read
                            .next()
                            .await
                            .ok_or_else(|| anyhow::anyhow!("等待 task-started 时连接已关闭"))?
                            .map_err(|e| anyhow::anyhow!("等待 task-started 失败: {}", e))?;

                        match message {
                            Message::Text(text) => {
                                let data: serde_json::Value =
                                    serde_json::from_str(&text).map_err(|e| {
                                        anyhow::anyhow!("解析 task-started 失败: {}", e)
                                    })?;
                                match parse_qwen_audio_3_server_event(&data) {
                                    QwenAudio3ServerEvent::TaskStarted => {
                                        tracing::info!("Qwen Audio 3.x 任务已启动");
                                        return Ok::<(), anyhow::Error>(());
                                    }
                                    QwenAudio3ServerEvent::Failed(message) => {
                                        anyhow::bail!("Qwen Audio 3.x 启动失败: {}", message);
                                    }
                                    _ => {}
                                }
                            }
                            Message::Close(_) => {
                                anyhow::bail!("等待 task-started 时服务端关闭连接");
                            }
                            _ => {}
                        }
                    }
                })
                .await
                .map_err(|_| {
                    anyhow::anyhow!("等待 task-started 超时（{} 秒）", TASK_START_TIMEOUT_SECS)
                })??;
            }
            QwenAsrProfile::Qwen3Legacy => {
                let input_audio_transcription = build_input_audio_transcription_with_pairs(
                    self.language_mode,
                    &self.dictionary,
                    &self.correction_pairs,
                );
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
                        "turn_detection": serde_json::Value::Null
                    }
                });

                write
                    .send(Message::Text(session_update.to_string()))
                    .await
                    .map_err(|e| anyhow::anyhow!("发送 session.update 失败: {}", e))?;
                tracing::info!("已发送 Qwen3 旧版 session.update 配置");
            }
        }

        // 启动发送任务
        let write: Arc<Mutex<WsSink>> = Arc::new(Mutex::new(write));
        let write_clone = Arc::clone(&write);
        let sender_task_id = task_id.clone();

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio(pcm_bytes) => {
                        let mut w = write_clone.lock().await;
                        let send_result = match profile {
                            QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                                w.send(Message::Binary(pcm_bytes.into())).await
                            }
                            QwenAsrProfile::Qwen3Legacy => {
                                let encoded = general_purpose::STANDARD.encode(&pcm_bytes);
                                let event = serde_json::json!({
                                    "event_id": format!("event_{}", std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis()),
                                    "type": "input_audio_buffer.append",
                                    "audio": encoded
                                });
                                w.send(Message::Text(event.to_string())).await
                            }
                        };
                        if let Err(e) = send_result {
                            tracing::error!("发送音频块失败: {}", e);
                            break;
                        }
                    }
                    SessionCommand::Commit => {
                        let event = match profile {
                            QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                                build_qwen_audio_3_finish_task(
                                    sender_task_id.as_deref().expect("最新版协议必须有 task_id"),
                                )
                            }
                            QwenAsrProfile::Qwen3Legacy => serde_json::json!({
                                "event_id": format!("event_{}", std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()),
                                "type": "input_audio_buffer.commit"
                            }),
                        };

                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Text(event.to_string())).await {
                            tracing::error!("发送 commit 失败: {}", e);
                        }
                        tracing::info!("已发送千问实时 ASR 提交事件");
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
        tokio::spawn(async move {
            let mut final_text = String::new();
            let mut has_result = false;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(data) => match profile {
                                QwenAsrProfile::QwenAudio3_1 | QwenAsrProfile::QwenAudio3 => {
                                    let event_type = data["header"]["event"].as_str().unwrap_or("");
                                    tracing::debug!("收到 Qwen Audio 3.x 事件: {}", event_type);

                                    match parse_qwen_audio_3_server_event(&data) {
                                        QwenAudio3ServerEvent::FinalSentence(sentence) => {
                                            final_text.push_str(&sentence);
                                        }
                                        QwenAudio3ServerEvent::TaskFinished => {
                                            if final_text.is_empty() {
                                                let _ = result_tx
                                                    .send(Err(anyhow::anyhow!("未收到转录结果")))
                                                    .await;
                                                return;
                                            }
                                            has_result = true;
                                        }
                                        QwenAudio3ServerEvent::Failed(message) => {
                                            tracing::error!("Qwen Audio 3.x API 错误: {}", message);
                                            let _ = result_tx
                                                .send(Err(anyhow::anyhow!("API 错误: {}", message)))
                                                .await;
                                            return;
                                        }
                                        QwenAudio3ServerEvent::TaskStarted
                                        | QwenAudio3ServerEvent::Ignore => {}
                                    }
                                }
                                QwenAsrProfile::Qwen3Legacy => {
                                    let event_type = data["type"].as_str().unwrap_or("");
                                    tracing::debug!("收到 Qwen3 旧版事件: {}", event_type);

                                    match event_type {
                                        "session.created" | "session.updated" => {
                                            tracing::info!("会话已创建/更新");
                                        }
                                        "input_audio_buffer.committed" => {
                                            tracing::info!("音频缓冲区已提交");
                                        }
                                        "conversation.item.input_audio_transcription.completed" => {
                                            if let Some(transcript) = data["transcript"].as_str() {
                                                final_text = transcript.to_string();
                                                has_result = true;
                                            }
                                        }
                                        "response.audio_transcript.delta" => {
                                            if let Some(delta) = data["delta"].as_str() {
                                                final_text.push_str(delta);
                                            }
                                        }
                                        "response.audio_transcript.done" => {
                                            if let Some(transcript) = data["transcript"].as_str() {
                                                final_text = transcript.to_string();
                                            }
                                            has_result = true;
                                        }
                                        "response.done" => {
                                            has_result = true;
                                        }
                                        "error" => {
                                            let error_msg = data["error"]["message"]
                                                .as_str()
                                                .unwrap_or("未知错误");
                                            tracing::error!("API 错误: {}", error_msg);
                                            let _ = result_tx
                                                .send(Err(anyhow::anyhow!(
                                                    "API 错误: {}",
                                                    error_msg
                                                )))
                                                .await;
                                            return;
                                        }
                                        _ => {
                                            tracing::debug!("未处理的事件类型: {}", event_type);
                                        }
                                    }
                                }
                            },
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
                        let _ = result_tx
                            .send(Err(anyhow::anyhow!("WebSocket 错误: {}", e)))
                            .await;
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
                        let _ = result_tx
                            .send(Err(anyhow::anyhow!("录音无效，已跳过")))
                            .await;
                        break;
                    }

                    // 实时模式下删除所有标点符号
                    let punctuation = [
                        '。', '，', '！', '？', '、', '；', '：', '"', '"', '.', ',', '!', '?',
                        ';', ':', '"', '\'', '（', '）', '(', ')', '【', '】', '[', ']', '《',
                        '》', '<', '>', '—', '…', '·', '\u{2018}', '\u{2019}',
                    ]; // 中文单引号 ' '
                    final_text = final_text
                        .chars()
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
    pub fn new(api_key: String, dictionary: Vec<String>, language_mode: AsrLanguageMode) -> Self {
        Self {
            pool: ConnectionPool::new(api_key, dictionary, language_mode),
        }
    }

    pub fn new_with_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
    ) -> Self {
        Self::new_with_profile_and_correction_pairs(
            api_key,
            dictionary,
            correction_pairs,
            language_mode,
            QwenAsrProfile::default(),
        )
    }

    pub fn new_with_profile_and_correction_pairs(
        api_key: String,
        dictionary: Vec<String>,
        correction_pairs: Vec<CorrectionPair>,
        language_mode: AsrLanguageMode,
        profile: QwenAsrProfile,
    ) -> Self {
        Self {
            pool: ConnectionPool::new_with_profile_and_correction_pairs(
                api_key,
                dictionary,
                correction_pairs,
                language_mode,
                profile,
            ),
        }
    }

    /// 创建新的转录会话
    pub async fn start_session(&self) -> Result<RealtimeSession> {
        self.pool.get_session().await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_input_audio_transcription, build_input_audio_transcription_with_pairs,
        build_qwen_audio_3_finish_task, build_qwen_audio_3_run_task,
        parse_qwen_audio_3_server_event, QwenAudio3ServerEvent,
    };
    use crate::config::{AsrLanguageMode, QwenAsrProfile};
    use crate::personalization::hotword_compiler::QWEN_REALTIME_MAX_HOTWORDS;
    use crate::personalization::CorrectionPair;

    #[test]
    fn qwen_audio_3_1_realtime_uses_selected_model() {
        let event = build_qwen_audio_3_run_task(
            QwenAsrProfile::QwenAudio3_1,
            "00000000-0000-4000-8000-000000000000",
            AsrLanguageMode::Auto,
            &[],
            &[],
        );
        assert_eq!(
            event["payload"]["model"],
            "qwen-audio-3.1-asr-flash-streaming"
        );
        assert_eq!(event["header"]["action"], "run-task");
        assert_eq!(event["payload"]["parameters"]["format"], "pcm");
        assert_eq!(event["payload"]["parameters"]["sample_rate"], 16000);
        assert!(event["payload"]["parameters"]
            .get("language_hints")
            .is_none());
        assert!(event["payload"]["parameters"].get("vocabulary").is_none());
    }

    #[test]
    fn builds_auto_language_for_qwen_session_update() {
        let dictionary: Vec<String> = vec![];
        let transcription = build_input_audio_transcription(AsrLanguageMode::Auto, &dictionary);
        assert_eq!(transcription["language"], "auto");
    }

    #[test]
    fn builds_zh_language_for_qwen_session_update() {
        let dictionary: Vec<String> = vec![];
        let transcription = build_input_audio_transcription(AsrLanguageMode::Zh, &dictionary);
        assert_eq!(transcription["language"], "zh");
    }

    #[test]
    fn limits_qwen_realtime_corpus_with_hotword_compiler() {
        let dictionary = (0..(QWEN_REALTIME_MAX_HOTWORDS + 5))
            .map(|idx| format!("词{}", idx))
            .collect::<Vec<_>>();

        let transcription = build_input_audio_transcription(AsrLanguageMode::Auto, &dictionary);
        let corpus = transcription["corpus"]["text"].as_str().unwrap();

        assert_eq!(corpus.split('、').count(), QWEN_REALTIME_MAX_HOTWORDS);
        assert!(!corpus.contains('|'));
    }

    #[test]
    fn qwen_realtime_corpus_includes_runtime_correction_pairs() {
        let dictionary = vec!["Rust|auto|tool".to_string()];
        let pairs = vec![CorrectionPair::new("windsurf", "winds surf", "Windsurf")];

        let transcription =
            build_input_audio_transcription_with_pairs(AsrLanguageMode::Auto, &dictionary, &pairs);
        let corpus = transcription["corpus"]["text"].as_str().unwrap();

        assert_eq!(corpus, "Windsurf、Rust");
        assert!(!corpus.contains("winds surf"));
        assert!(!corpus.contains('|'));
    }

    #[test]
    fn builds_qwen_audio_3_run_task_with_binary_pcm_contract() {
        let dictionary = vec!["Rust|auto|tool".to_string()];
        let pairs = vec![CorrectionPair::new("windsurf", "winds surf", "Windsurf")];

        let event = build_qwen_audio_3_run_task(
            QwenAsrProfile::QwenAudio3,
            "00000000-0000-4000-8000-000000000000",
            AsrLanguageMode::Auto,
            &dictionary,
            &pairs,
        );

        assert_eq!(event["header"]["action"], "run-task");
        assert_eq!(event["header"]["streaming"], "duplex");
        assert_eq!(
            event["payload"]["model"],
            "qwen-audio-3.0-asr-flash-streaming"
        );
        assert_eq!(event["payload"]["parameters"]["format"], "pcm");
        assert_eq!(event["payload"]["parameters"]["sample_rate"], 16000);
        assert_eq!(event["payload"]["parameters"]["vocabulary"]["Windsurf"], 4);
        assert_eq!(event["payload"]["parameters"]["vocabulary"]["Rust"], 4);
        assert!(event["payload"]["parameters"]
            .get("language_hints")
            .is_none());
    }

    #[test]
    fn builds_qwen_audio_3_zh_language_hint_and_finish_task() {
        let run_task = build_qwen_audio_3_run_task(
            QwenAsrProfile::QwenAudio3,
            "00000000-0000-4000-8000-000000000000",
            AsrLanguageMode::Zh,
            &[],
            &[],
        );
        let finish_task = build_qwen_audio_3_finish_task("00000000-0000-4000-8000-000000000000");

        assert_eq!(
            run_task["payload"]["parameters"]["language_hints"],
            serde_json::json!(["zh"])
        );
        assert_eq!(finish_task["header"]["action"], "finish-task");
        assert_eq!(finish_task["payload"]["input"], serde_json::json!({}));
    }

    #[test]
    fn parses_qwen_audio_3_final_sentence_and_task_failure() {
        let final_sentence = serde_json::json!({
            "header": { "event": "result-generated" },
            "payload": {
                "output": {
                    "sentence": {
                        "text": "好，我知道了",
                        "heartbeat": false,
                        "sentence_end": true
                    }
                }
            }
        });
        let failed = serde_json::json!({
            "header": {
                "event": "task-failed",
                "error_code": "CLIENT_ERROR",
                "error_message": "request timeout"
            },
            "payload": {}
        });

        assert_eq!(
            parse_qwen_audio_3_server_event(&final_sentence),
            QwenAudio3ServerEvent::FinalSentence("好，我知道了".to_string())
        );
        assert_eq!(
            parse_qwen_audio_3_server_event(&failed),
            QwenAudio3ServerEvent::Failed("CLIENT_ERROR: request timeout".to_string())
        );
    }
}
