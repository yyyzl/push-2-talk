use crate::config::AsrLanguageMode;
use crate::dictionary_utils::entries_to_words;
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use reqwest::Url;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{
    client_async_tls, connect_async,
    tungstenite::{http, Message},
    MaybeTlsStream, WebSocketStream,
};

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

const WEBSOCKET_URL: &str = "wss://api.x.ai/v1/realtime";
const DEFAULT_MODEL: &str = "grok-2-audio";
const READY_TIMEOUT_SECS: u64 = 8;
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 10;
const MAX_PROXY_RESPONSE_BYTES: usize = 8192;

enum SessionCommand {
    SendAudio(Vec<u8>),
    Commit,
    Close,
}

pub struct GrokRealtimeSession {
    sender: mpsc::Sender<SessionCommand>,
    result_receiver: mpsc::Receiver<Result<String>>,
}

impl GrokRealtimeSession {
    pub async fn send_audio_chunk(&self, pcm_data: &[i16]) -> Result<()> {
        let bytes: Vec<u8> = pcm_data
            .iter()
            .flat_map(|&sample| sample.to_le_bytes())
            .collect();

        self.sender
            .send(SessionCommand::SendAudio(bytes))
            .await
            .map_err(|_| anyhow::anyhow!("发送 Grok 音频块失败：通道已关闭"))
    }

    pub async fn commit_audio(&self) -> Result<()> {
        self.sender
            .send(SessionCommand::Commit)
            .await
            .map_err(|_| anyhow::anyhow!("提交 Grok 音频失败：通道已关闭"))
    }

    pub async fn wait_for_result(&mut self) -> Result<String> {
        match timeout(
            Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
            self.result_receiver.recv(),
        )
        .await
        {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("等待 Grok 转录结果失败：通道已关闭")),
            Err(_) => Err(anyhow::anyhow!(
                "Grok 转录超时：{}秒内未收到结果",
                TRANSCRIPTION_TIMEOUT_SECS
            )),
        }
    }

    pub async fn close(&self) -> Result<()> {
        let _ = self.sender.send(SessionCommand::Close).await;
        Ok(())
    }
}

pub struct GrokRealtimeClient {
    api_key: String,
    model: String,
    proxy: Option<String>,
    dictionary: Vec<String>,
    language_mode: AsrLanguageMode,
}

impl GrokRealtimeClient {
    pub fn new(
        api_key: String,
        model: String,
        proxy: String,
        dictionary: Vec<String>,
        language_mode: AsrLanguageMode,
    ) -> Self {
        Self {
            api_key,
            model: if model.trim().is_empty() {
                DEFAULT_MODEL.to_string()
            } else {
                model
            },
            proxy: normalize_proxy_url(&proxy),
            dictionary,
            language_mode,
        }
    }

    pub async fn start_session(&self) -> Result<GrokRealtimeSession> {
        tracing::info!(
            "创建 Grok WebSocket 连接: endpoint={}, model={}, proxy={}",
            WEBSOCKET_URL,
            self.model,
            self.proxy.as_deref().unwrap_or("<direct>")
        );

        let request = http::Request::builder()
            .uri(WEBSOCKET_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("OpenAI-Beta", "realtime=v1")
            .header("Host", "api.x.ai")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .body(())?;

        let ws_stream = connect_grok_websocket(request, self.proxy.as_deref()).await?;
        let (mut write, mut read) = ws_stream.split();

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(100);
        let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);
        let (ready_tx, ready_rx) = oneshot::channel::<Result<()>>();
        let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));

        let session_update = serde_json::json!({
            "type": "session.update",
            "session": {
                "modalities": ["text"],
                "instructions": build_transcription_instructions(self.language_mode, &self.dictionary),
                "audio": {
                    "input": {
                        "format": {
                            "type": "audio/pcm",
                            "rate": 16000
                        }
                    }
                },
                "input_audio_transcription": {
                    "model": self.model
                },
                "turn_detection": serde_json::Value::Null
            }
        });

        write
            .send(Message::Text(session_update.to_string()))
            .await
            .map_err(|e| anyhow::anyhow!("发送 Grok session.update 失败: {}", e))?;

        let write: Arc<Mutex<WsSink>> = Arc::new(Mutex::new(write));
        let write_clone = Arc::clone(&write);

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio(pcm_bytes) => {
                        let encoded = general_purpose::STANDARD.encode(&pcm_bytes);
                        let event = serde_json::json!({
                            "type": "input_audio_buffer.append",
                            "audio": encoded
                        });

                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Text(event.to_string())).await {
                            tracing::error!("发送 Grok 音频块失败: {}", e);
                            break;
                        }
                    }
                    SessionCommand::Commit => {
                        let event = serde_json::json!({
                            "type": "input_audio_buffer.commit"
                        });

                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Text(event.to_string())).await {
                            tracing::error!("发送 Grok commit 失败: {}", e);
                        }
                    }
                    SessionCommand::Close => {
                        let mut w = write_clone.lock().await;
                        let _ = w.close().await;
                        break;
                    }
                }
            }
        });

        tokio::spawn(async move {
            let mut final_text = String::new();
            let mut has_result = false;
            let mut ready_sent = false;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(data) => {
                                let event_type = data["type"].as_str().unwrap_or("");
                                tracing::debug!("Grok 收到事件: {}", event_type);

                                match event_type {
                                    "session.created" | "session.updated" => {
                                        if !ready_sent {
                                            ready_sent = true;
                                            if let Some(tx) = ready_tx.lock().await.take() {
                                                let _ = tx.send(Ok(()));
                                            }
                                        }
                                    }
                                    "conversation.item.input_audio_transcription.completed" => {
                                        if let Some(transcript) = data["transcript"].as_str() {
                                            final_text = transcript.to_string();
                                            has_result = true;
                                            tracing::info!("Grok 转录完成: {}", final_text);
                                        }
                                    }
                                    "error" => {
                                        let error_msg =
                                            data["error"]["message"].as_str().unwrap_or("未知错误");
                                        if !ready_sent {
                                            if let Some(tx) = ready_tx.lock().await.take() {
                                                let _ = tx.send(Err(anyhow::anyhow!(
                                                    "Grok 会话初始化失败: {}",
                                                    error_msg
                                                )));
                                            }
                                        }
                                        let _ = result_tx
                                            .send(Err(anyhow::anyhow!(
                                                "Grok API 错误: {}",
                                                error_msg
                                            )))
                                            .await;
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                tracing::warn!("解析 Grok 消息失败: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        tracing::info!("Grok WebSocket 连接关闭");
                        break;
                    }
                    Err(e) => {
                        if !ready_sent {
                            if let Some(tx) = ready_tx.lock().await.take() {
                                let _ = tx.send(Err(anyhow::anyhow!("Grok WebSocket 错误: {}", e)));
                            }
                        }
                        let _ = result_tx
                            .send(Err(anyhow::anyhow!("Grok WebSocket 错误: {}", e)))
                            .await;
                        return;
                    }
                    _ => {}
                }

                if has_result && !final_text.is_empty() {
                    let _ = result_tx
                        .send(Ok(strip_realtime_punctuation(final_text)))
                        .await;
                    return;
                }
            }

            if !ready_sent {
                if let Some(tx) = ready_tx.lock().await.take() {
                    let _ = tx.send(Err(anyhow::anyhow!("Grok 会话初始化失败：连接提前关闭")));
                }
            }

            if !has_result {
                let _ = result_tx
                    .send(Err(anyhow::anyhow!("Grok 未收到转录结果")))
                    .await;
            }
        });

        match timeout(Duration::from_secs(READY_TIMEOUT_SECS), ready_rx).await {
            Ok(Ok(Ok(()))) => Ok(GrokRealtimeSession {
                sender: cmd_tx,
                result_receiver: result_rx,
            }),
            Ok(Ok(Err(err))) => Err(err),
            Ok(Err(_)) => Err(anyhow::anyhow!("Grok 会话初始化失败：准备通道已关闭")),
            Err(_) => Err(anyhow::anyhow!(
                "Grok 会话初始化超时：{}秒内未收到 session.updated",
                READY_TIMEOUT_SECS
            )),
        }
    }
}

async fn connect_grok_websocket(
    request: http::Request<()>,
    proxy: Option<&str>,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    if let Some(proxy) = proxy {
        connect_via_http_proxy(request, proxy).await
    } else {
        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| anyhow::anyhow!("Grok WebSocket 连接失败: {}", e))?;
        Ok(ws_stream)
    }
}

async fn connect_via_http_proxy(
    request: http::Request<()>,
    proxy: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let proxy_url = Url::parse(proxy).with_context(|| format!("无效的代理地址: {}", proxy))?;
    if proxy_url.scheme() != "http" {
        anyhow::bail!("Grok 代理仅支持 http:// 形式的 HTTP CONNECT 代理");
    }

    let proxy_host = proxy_url
        .host_str()
        .context("代理地址缺少 host")?
        .to_string();
    let proxy_port = proxy_url
        .port_or_known_default()
        .context("代理地址缺少端口")?;

    let uri = request.uri();
    let target_host = uri.host().context("Grok WebSocket URL 缺少 host")?;
    let target_port = uri.port_u16().unwrap_or(443);

    let mut stream = TcpStream::connect((proxy_host.as_str(), proxy_port))
        .await
        .with_context(|| format!("连接代理失败: {}:{}", proxy_host, proxy_port))?;

    let mut connect_request = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
        target_host, target_port, target_host, target_port
    );

    if !proxy_url.username().is_empty() {
        let password = proxy_url.password().unwrap_or("");
        let credentials = format!("{}:{}", proxy_url.username(), password);
        let auth = general_purpose::STANDARD.encode(credentials.as_bytes());
        connect_request = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Connection: Keep-Alive\r\nProxy-Authorization: Basic {}\r\n\r\n",
            target_host, target_port, target_host, target_port, auth
        );
    }

    stream
        .write_all(connect_request.as_bytes())
        .await
        .context("向代理发送 CONNECT 请求失败")?;

    let mut response = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = stream
            .read(&mut buf)
            .await
            .context("读取代理 CONNECT 响应失败")?;
        if n == 0 {
            anyhow::bail!("代理在建立 CONNECT 隧道前关闭了连接");
        }
        response.extend_from_slice(&buf[..n]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() >= MAX_PROXY_RESPONSE_BYTES {
            anyhow::bail!("代理 CONNECT 响应过大，无法继续握手");
        }
    }

    let response_text = String::from_utf8_lossy(&response);
    let status_line = response_text.lines().next().unwrap_or("");
    if !status_line.contains(" 200 ") {
        anyhow::bail!("代理 CONNECT 失败: {}", status_line);
    }

    let (ws_stream, _) = client_async_tls(request, stream)
        .await
        .map_err(|e| anyhow::anyhow!("通过代理建立 Grok WebSocket 失败: {}", e))?;
    Ok(ws_stream)
}

fn normalize_proxy_url(proxy: &str) -> Option<String> {
    let proxy = proxy.trim();
    if proxy.is_empty() {
        return None;
    }

    if proxy.contains("://") {
        Some(proxy.to_string())
    } else {
        Some(format!("http://{}", proxy))
    }
}

fn build_transcription_instructions(
    language_mode: AsrLanguageMode,
    dictionary: &[String],
) -> String {
    let mut instructions =
        String::from("你是专业的语音转录助手。只输出转录文本，不要解释，不要添加前缀。");

    match language_mode {
        AsrLanguageMode::Zh => {
            instructions.push_str(" 识别时优先按中文语境处理。");
        }
        AsrLanguageMode::Auto => {
            instructions.push_str(" 自动识别语种，但优先保证中文与英文术语准确。");
        }
    }

    let purified_words = entries_to_words(dictionary);
    if !purified_words.is_empty() {
        instructions.push_str(" 以下词汇请尽量保持指定写法：");
        instructions.push_str(&purified_words.join("、"));
        instructions.push('。');
    }

    instructions
}

fn strip_realtime_punctuation(mut text: String) -> String {
    let punctuation = [
        '。', '，', '！', '？', '、', '；', '：', '"', '.', ',', '!', '?', ';', ':', '\'', '（',
        '）', '(', ')', '【', '】', '[', ']', '《', '》', '<', '>', '—', '…', '·', '\u{2018}',
        '\u{2019}', '\u{201c}', '\u{201d}',
    ];

    text = text.chars().filter(|c| !punctuation.contains(c)).collect();
    text
}

#[cfg(test)]
mod tests {
    use super::{build_transcription_instructions, normalize_proxy_url};
    use crate::config::AsrLanguageMode;

    #[test]
    fn normalizes_proxy_without_scheme() {
        assert_eq!(
            normalize_proxy_url("127.0.0.1:7897").as_deref(),
            Some("http://127.0.0.1:7897")
        );
    }

    #[test]
    fn builds_instruction_with_dictionary_words() {
        let instructions = build_transcription_instructions(
            AsrLanguageMode::Auto,
            &["Claude".to_string(), "PushToTalk|auto".to_string()],
        );
        assert!(instructions.contains("Claude"));
        assert!(instructions.contains("PushToTalk"));
        assert!(!instructions.contains("|auto"));
    }
}
