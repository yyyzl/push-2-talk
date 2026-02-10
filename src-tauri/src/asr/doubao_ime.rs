// 豆包输入法 ASR 客户端 (非官方 API)
// 通过逆向安卓豆包输入法客户端协议实现
//
// 注意事项：
// - 本模块仅供学习和研究目的
// - 不保证未来的可用性和稳定性
// - 服务端协议可能随时变更导致功能失效
//
// 编译要求：
// - 需要启用 `doubao-ime` feature
// - 需要安装 CMake (Windows: choco install cmake)

use anyhow::{anyhow, Result};

#[cfg(feature = "doubao-ime")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "doubao-ime")]
use std::future::Future;
#[cfg(feature = "doubao-ime")]
use std::time::Duration;
#[cfg(feature = "doubao-ime")]
use tokio::sync::mpsc;
#[cfg(feature = "doubao-ime")]
use tokio::time::timeout;
#[cfg(feature = "doubao-ime")]
use tokio_tungstenite::{connect_async, tungstenite};

// ==================== 常量定义 ====================

/// 设备注册 API URL
#[cfg(feature = "doubao-ime")]
const REGISTER_URL: &str = "https://log.snssdk.com/service/2/device_register/";

/// Settings API URL (获取 Token)
#[cfg(feature = "doubao-ime")]
const SETTINGS_URL: &str = "https://is.snssdk.com/service/settings/v3/";

/// ASR WebSocket URL
#[cfg(feature = "doubao-ime")]
const WEBSOCKET_URL: &str = "wss://frontier-audio-ime-ws.doubao.com/ocean/api/v1/ws";

/// 豆包输入法的 APP ID
#[cfg(feature = "doubao-ime")]
const AID: u32 = 401734;

/// 转录超时时间（秒）
#[cfg(feature = "doubao-ime")]
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 15;

/// 注册和 Token 请求超时（秒）
#[cfg(feature = "doubao-ime")]
const CREDENTIAL_REQUEST_TIMEOUT_SECS: u64 = 10;

/// User-Agent
#[cfg(feature = "doubao-ime")]
const USER_AGENT: &str = "com.bytedance.android.doubaoime/100102018 (Linux; U; Android 16; en_US; Pixel 7 Pro; Build/BP2A.250605.031.A2; Cronet/TTNetVersion:94cf429a 2025-11-17 QuicVersion:1f89f732 2025-05-08)";

// ==================== 配置和凭据 ====================

/// 豆包输入法 ASR 客户端配置
#[derive(Clone, Debug)]
pub struct DoubaoImeClientConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub frame_duration_ms: u32,
    pub enable_punctuation: bool,
    pub connect_timeout_secs: u64,
    pub recv_timeout_secs: u64,
}

impl Default for DoubaoImeClientConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            frame_duration_ms: 20,
            enable_punctuation: true,
            connect_timeout_secs: 10,
            recv_timeout_secs: 15,
        }
    }
}

/// 设备凭据
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeviceCredentials {
    pub device_id: String,
    pub install_id: String,
    pub cdid: String,
    pub openudid: String,
    pub clientudid: String,
    pub token: String,
}

// ==================== 功能未启用时的存根实现 ====================

#[cfg(not(feature = "doubao-ime"))]
pub struct DoubaoImeRealtimeSession;

#[cfg(not(feature = "doubao-ime"))]
impl DoubaoImeRealtimeSession {
    pub async fn send_audio_chunk(&mut self, _pcm_data: &[i16]) -> Result<()> {
        Err(anyhow!(
            "豆包输入法 ASR 功能未启用。请使用 `--features doubao-ime` 编译，并确保已安装 CMake。"
        ))
    }

    pub async fn finish_audio(&mut self) -> Result<()> {
        Err(anyhow!("豆包输入法 ASR 功能未启用。"))
    }

    pub async fn wait_for_result(&mut self) -> Result<String> {
        Err(anyhow!("豆包输入法 ASR 功能未启用。"))
    }
}

#[cfg(not(feature = "doubao-ime"))]
pub struct DoubaoImeRealtimeClient {
    _config: DoubaoImeClientConfig,
}

#[cfg(not(feature = "doubao-ime"))]
impl DoubaoImeRealtimeClient {
    pub fn new(_client: reqwest::Client, config: DoubaoImeClientConfig) -> Self {
        Self { _config: config }
    }

    pub fn with_credentials(
        _client: reqwest::Client,
        config: DoubaoImeClientConfig,
        _credentials: DeviceCredentials,
    ) -> Self {
        Self { _config: config }
    }

    pub async fn ensure_credentials(&mut self) -> Result<&DeviceCredentials> {
        Err(anyhow!(
            "豆包输入法 ASR 功能未启用。请使用 `--features doubao-ime` 编译，并确保已安装 CMake。"
        ))
    }

    pub fn credentials(&self) -> Option<&DeviceCredentials> {
        None
    }

    pub async fn start_session(&mut self) -> Result<DoubaoImeRealtimeSession> {
        Err(anyhow!(
            "豆包输入法 ASR 功能未启用。请使用 `--features doubao-ime` 编译，并确保已安装 CMake。"
        ))
    }
}

#[cfg(not(feature = "doubao-ime"))]
pub struct DoubaoImeClient;

#[cfg(not(feature = "doubao-ime"))]
impl DoubaoImeClient {
    pub fn new(_client: reqwest::Client) -> Self {
        Self
    }

    pub fn with_credentials(_client: reqwest::Client, _credentials: DeviceCredentials) -> Self {
        Self
    }

    pub async fn ensure_credentials(&mut self) -> Result<&DeviceCredentials> {
        Err(anyhow!("豆包输入法 ASR 功能未启用。"))
    }

    pub async fn transcribe_wav(&mut self, _wav_path: &std::path::Path) -> Result<String> {
        Err(anyhow!(
            "豆包输入法 ASR 功能未启用。请使用 `--features doubao-ime` 编译，并确保已安装 CMake。"
        ))
    }

    pub async fn transcribe_pcm(&mut self, _pcm_bytes: &[u8]) -> Result<String> {
        Err(anyhow!("豆包输入法 ASR 功能未启用。"))
    }
}

// ==================== 功能启用时的完整实现 ====================

#[cfg(feature = "doubao-ime")]
mod implementation {
    use super::*;
    use opus;

    // ==================== Protobuf 消息定义 ====================

    /// 音频帧状态
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(i32)]
    #[allow(dead_code)]
    pub enum FrameState {
        Unspecified = 0,
        First = 1,
        Middle = 3,
        Last = 9,
    }

    impl FrameState {
        fn as_i32(&self) -> i32 {
            *self as i32
        }
    }

    /// ASR 请求消息 (Protobuf 手动编码)
    #[derive(Clone, Debug, Default)]
    struct AsrRequest {
        token: String,
        service_name: String,
        method_name: String,
        payload: String,
        audio_data: Vec<u8>,
        request_id: String,
        frame_state: i32,
    }

    impl AsrRequest {
        fn encode(&self) -> Vec<u8> {
            let mut buf = Vec::new();

            if !self.token.is_empty() {
                encode_string(&mut buf, 2, &self.token);
            }
            if !self.service_name.is_empty() {
                encode_string(&mut buf, 3, &self.service_name);
            }
            if !self.method_name.is_empty() {
                encode_string(&mut buf, 5, &self.method_name);
            }
            if !self.payload.is_empty() {
                encode_string(&mut buf, 6, &self.payload);
            }
            if !self.audio_data.is_empty() {
                encode_bytes(&mut buf, 7, &self.audio_data);
            }
            if !self.request_id.is_empty() {
                encode_string(&mut buf, 8, &self.request_id);
            }
            if self.frame_state != 0 {
                encode_varint_field(&mut buf, 9, self.frame_state as u64);
            }

            buf
        }
    }

    /// ASR 响应消息
    #[derive(Clone, Debug, Default)]
    struct AsrResponse {
        request_id: String,
        task_id: String,
        service_name: String,
        message_type: String,
        status_code: i32,
        status_message: String,
        result_json: String,
    }

    impl AsrResponse {
        fn decode(data: &[u8]) -> Result<Self> {
            let mut response = AsrResponse::default();
            let mut cursor = 0;

            while cursor < data.len() {
                let (field_number, wire_type, new_cursor) = decode_tag(data, cursor)?;
                cursor = new_cursor;

                match (field_number, wire_type) {
                    (1, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.request_id = s;
                        cursor = new_cursor;
                    }
                    (2, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.task_id = s;
                        cursor = new_cursor;
                    }
                    (3, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.service_name = s;
                        cursor = new_cursor;
                    }
                    (4, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.message_type = s;
                        cursor = new_cursor;
                    }
                    (5, 0) => {
                        let (v, new_cursor) = decode_varint(data, cursor)?;
                        response.status_code = v as i32;
                        cursor = new_cursor;
                    }
                    (6, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.status_message = s;
                        cursor = new_cursor;
                    }
                    (7, 2) => {
                        let (s, new_cursor) = decode_string(data, cursor)?;
                        response.result_json = s;
                        cursor = new_cursor;
                    }
                    (_, 0) => {
                        let (_, new_cursor) = decode_varint(data, cursor)?;
                        cursor = new_cursor;
                    }
                    (_, 2) => {
                        let (len, new_cursor) = decode_varint(data, cursor)?;
                        cursor = new_cursor + len as usize;
                    }
                    _ => break,
                }
            }

            Ok(response)
        }
    }

    // Protobuf 编解码辅助函数
    fn encode_varint(buf: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            buf.push((value as u8) | 0x80);
            value >>= 7;
        }
        buf.push(value as u8);
    }

    fn encode_varint_field(buf: &mut Vec<u8>, field_number: u32, value: u64) {
        let tag = (field_number << 3) | 0;
        encode_varint(buf, tag as u64);
        encode_varint(buf, value);
    }

    fn encode_string(buf: &mut Vec<u8>, field_number: u32, s: &str) {
        let tag = (field_number << 3) | 2;
        encode_varint(buf, tag as u64);
        encode_varint(buf, s.len() as u64);
        buf.extend_from_slice(s.as_bytes());
    }

    fn encode_bytes(buf: &mut Vec<u8>, field_number: u32, data: &[u8]) {
        let tag = (field_number << 3) | 2;
        encode_varint(buf, tag as u64);
        encode_varint(buf, data.len() as u64);
        buf.extend_from_slice(data);
    }

    fn decode_varint(data: &[u8], start: usize) -> Result<(u64, usize)> {
        let mut result: u64 = 0;
        let mut shift = 0;
        let mut cursor = start;

        loop {
            if cursor >= data.len() {
                return Err(anyhow!("Unexpected end of data while decoding varint"));
            }
            let byte = data[cursor];
            cursor += 1;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                return Err(anyhow!("Varint too long"));
            }
        }

        Ok((result, cursor))
    }

    fn decode_tag(data: &[u8], start: usize) -> Result<(u32, u32, usize)> {
        let (tag, cursor) = decode_varint(data, start)?;
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u32;
        Ok((field_number, wire_type, cursor))
    }

    fn decode_string(data: &[u8], start: usize) -> Result<(String, usize)> {
        let (len, cursor) = decode_varint(data, start)?;
        let len = len as usize;
        if cursor + len > data.len() {
            return Err(anyhow!("String length exceeds data"));
        }
        let s = String::from_utf8_lossy(&data[cursor..cursor + len]).to_string();
        Ok((s, cursor + len))
    }

    fn extract_text_candidate_from_result_json(result_json: &str) -> Option<(String, bool)> {
        let json = serde_json::from_str::<serde_json::Value>(result_json).ok()?;
        let results = json.get("results")?.as_array()?;

        let mut latest_text: Option<String> = None;
        let mut final_text: Option<String> = None;

        for result in results {
            let text = match result.get("text").and_then(|t| t.as_str()) {
                Some(value) => value,
                None => continue,
            };

            let is_interim = result
                .get("is_interim")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let is_vad_finished = result
                .get("is_vad_finished")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let nonstream_result = result
                .get("extra")
                .and_then(|e| e.get("nonstream_result"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            latest_text = Some(text.to_string());
            if nonstream_result || (!is_interim && is_vad_finished) {
                final_text = Some(text.to_string());
            }
        }

        if let Some(text) = final_text {
            return Some((text, true));
        }

        latest_text.map(|text| (text, false))
    }

    // ==================== 设备注册和 Token 获取 ====================

    pub async fn register_device(client: &reqwest::Client) -> Result<DeviceCredentials> {
        let cdid = uuid::Uuid::new_v4().to_string();
        let openudid = format!("{:016x}", rand_u64());
        let clientudid = uuid::Uuid::new_v4().to_string();

        let header = serde_json::json!({
            "device_id": 0,
            "install_id": 0,
            "aid": AID,
            "app_name": "oime",
            "version_code": 100102018,
            "version_name": "1.1.2",
            "manifest_version_code": 100102018,
            "update_version_code": 100102018,
            "channel": "official",
            "package": "com.bytedance.android.doubaoime",
            "device_platform": "android",
            "os": "android",
            "os_api": "34",
            "os_version": "16",
            "device_type": "Pixel 7 Pro",
            "device_brand": "google",
            "device_model": "Pixel 7 Pro",
            "resolution": "1080*2400",
            "dpi": "420",
            "language": "zh",
            "timezone": 8,
            "access": "wifi",
            "rom": "UP1A.231005.007",
            "rom_version": "UP1A.231005.007",
            "openudid": &openudid,
            "clientudid": &clientudid,
            "cdid": &cdid,
            "region": "CN",
            "tz_name": "Asia/Shanghai",
            "tz_offset": 28800,
            "sim_region": "cn",
            "carrier_region": "cn",
            "cpu_abi": "arm64-v8a",
            "build_serial": "unknown",
            "not_request_sender": 0,
            "sig_hash": "",
            "google_aid": "",
            "mc": "",
            "serial_number": ""
        });

        let body = serde_json::json!({
            "magic_tag": "ss_app_log",
            "header": header,
            "_gen_time": chrono_timestamp_ms()
        });

        let params = build_register_params(&cdid);

        let response = with_request_timeout(
            "register_device",
            client
                .post(REGISTER_URL)
                .query(&params)
                .header("User-Agent", USER_AGENT)
                .json(&body)
                .send(),
        )
        .await?;

        let response_json: serde_json::Value = response.json().await?;

        let device_id = response_json["device_id"]
            .as_i64()
            .ok_or_else(|| anyhow!("Missing device_id in response"))?;
        let install_id = response_json["install_id"].as_i64().unwrap_or(0);

        if device_id == 0 {
            return Err(anyhow!("Device registration failed: device_id is 0"));
        }

        Ok(DeviceCredentials {
            device_id: device_id.to_string(),
            install_id: install_id.to_string(),
            cdid,
            openudid,
            clientudid,
            token: String::new(),
        })
    }

    async fn with_request_timeout<T, F>(
        request_name: &'static str,
        request_future: F,
    ) -> Result<T>
    where
        F: Future<Output = std::result::Result<T, reqwest::Error>>,
    {
        timeout(
            Duration::from_secs(CREDENTIAL_REQUEST_TIMEOUT_SECS),
            request_future,
        )
        .await
        .map_err(|_| {
            anyhow!(
                "{} timed out after {}s",
                request_name,
                CREDENTIAL_REQUEST_TIMEOUT_SECS
            )
        })?
        .map_err(anyhow::Error::from)
    }

    fn finalize_unexpected_ws_close_result(final_text: &str) -> Result<String> {
        if final_text.trim().is_empty() {
            Err(anyhow!(
                "WebSocket closed unexpectedly before final result, and final text is empty"
            ))
        } else {
            Ok(final_text.to_string())
        }
    }

    pub async fn get_asr_token(
        client: &reqwest::Client,
        device_id: &str,
        cdid: &str,
    ) -> Result<String> {
        let aid_str = AID.to_string();
        let params = vec![
            ("device_platform", "android"),
            ("os", "android"),
            ("ssmix", "a"),
            ("channel", "official"),
            ("aid", aid_str.as_str()),
            ("app_name", "oime"),
            ("version_code", "100102018"),
            ("version_name", "1.1.2"),
            ("device_id", device_id),
            ("cdid", cdid),
        ];

        let body_str = "body=null";
        let x_ss_stub = format!("{:X}", md5::compute(body_str.as_bytes()));

        let response = with_request_timeout(
            "get_asr_token",
            client
                .post(SETTINGS_URL)
                .query(&params)
                .header("User-Agent", USER_AGENT)
                .header("x-ss-stub", x_ss_stub)
                .body(body_str)
                .send(),
        )
        .await?;

        let response_json: serde_json::Value = response.json().await?;

        let token = response_json["data"]["settings"]["asr_config"]["app_key"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing app_key in settings response"))?
            .to_string();

        Ok(token)
    }

    fn build_register_params(cdid: &str) -> Vec<(&'static str, String)> {
        vec![
            ("device_platform", "android".to_string()),
            ("os", "android".to_string()),
            ("ssmix", "a".to_string()),
            ("_rticket", chrono_timestamp_ms().to_string()),
            ("cdid", cdid.to_string()),
            ("channel", "official".to_string()),
            ("aid", AID.to_string()),
            ("app_name", "oime".to_string()),
            ("version_code", "100102018".to_string()),
            ("version_name", "1.1.2".to_string()),
            ("manifest_version_code", "100102018".to_string()),
            ("update_version_code", "100102018".to_string()),
            ("resolution", "1080*2400".to_string()),
            ("dpi", "420".to_string()),
            ("device_type", "Pixel 7 Pro".to_string()),
            ("device_brand", "google".to_string()),
            ("language", "zh".to_string()),
            ("os_api", "34".to_string()),
            ("os_version", "16".to_string()),
            ("ac", "wifi".to_string()),
        ]
    }

    fn chrono_timestamp_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn rand_u64() -> u64 {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        RandomState::new().build_hasher().finish()
    }

    // ==================== Opus 编码器 ====================

    pub struct OpusEncoder {
        encoder: opus::Encoder,
        samples_per_frame: usize,
    }

    impl OpusEncoder {
        pub fn new(sample_rate: u32, channels: u32, frame_duration_ms: u32) -> Result<Self> {
            let channels = match channels {
                1 => opus::Channels::Mono,
                2 => opus::Channels::Stereo,
                _ => return Err(anyhow!("Unsupported channel count: {}", channels)),
            };

            // 使用 Voip 模式：专为语音优化，提供更好的语音清晰度和更低延迟
            let encoder = opus::Encoder::new(sample_rate, channels, opus::Application::Voip)?;
            let samples_per_frame = (sample_rate * frame_duration_ms / 1000) as usize;

            Ok(Self {
                encoder,
                samples_per_frame,
            })
        }

        pub fn encode_frame(&mut self, pcm_i16: &[i16]) -> Result<Vec<u8>> {
            let mut output = vec![0u8; 4000];
            let len = self.encoder.encode(pcm_i16, &mut output)?;
            output.truncate(len);
            Ok(output)
        }

        pub fn samples_per_frame(&self) -> usize {
            self.samples_per_frame
        }
    }

    // ==================== 实时会话 ====================

    pub struct DoubaoImeRealtimeSession {
        sender: mpsc::Sender<SessionCommand>,
        result_receiver: mpsc::Receiver<Result<String>>,
    }

    enum SessionCommand {
        SendAudio(Vec<u8>),
        Finish,
    }

    impl DoubaoImeRealtimeSession {
        pub async fn send_audio_chunk(&mut self, pcm_data: &[i16]) -> Result<()> {
            let bytes: Vec<u8> = pcm_data.iter().flat_map(|&s| s.to_le_bytes()).collect();
            self.sender
                .send(SessionCommand::SendAudio(bytes))
                .await
                .map_err(|_| anyhow!("发送音频块失败"))
        }

        pub async fn finish_audio(&mut self) -> Result<()> {
            self.sender
                .send(SessionCommand::Finish)
                .await
                .map_err(|_| anyhow!("发送结束标志失败"))
        }

        pub async fn wait_for_result(&mut self) -> Result<String> {
            match timeout(
                Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS),
                self.result_receiver.recv(),
            )
            .await
            {
                Ok(Some(result)) => result,
                Ok(None) => Err(anyhow!("会话已关闭")),
                Err(_) => Err(anyhow!("转录超时")),
            }
        }
    }

    // ==================== 实时客户端 ====================

    pub struct DoubaoImeRealtimeClient {
        client: reqwest::Client,
        config: DoubaoImeClientConfig,
        credentials: Option<DeviceCredentials>,
    }

    impl DoubaoImeRealtimeClient {
        pub fn new(client: reqwest::Client, config: DoubaoImeClientConfig) -> Self {
            Self {
                client,
                config,
                credentials: None,
            }
        }

        pub fn with_credentials(
            client: reqwest::Client,
            config: DoubaoImeClientConfig,
            credentials: DeviceCredentials,
        ) -> Self {
            Self {
                client,
                config,
                credentials: Some(credentials),
            }
        }

        pub async fn ensure_credentials(&mut self) -> Result<&DeviceCredentials> {
            if self.credentials.is_none() {
                tracing::info!("豆包输入法 ASR: 注册新设备...");
                let mut creds = register_device(&self.client).await?;
                tracing::info!(
                    "豆包输入法 ASR: 设备注册成功，device_id={}",
                    creds.device_id
                );

                tracing::info!("豆包输入法 ASR: 获取 Token...");
                creds.token = get_asr_token(&self.client, &creds.device_id, &creds.cdid).await?;
                tracing::info!("豆包输入法 ASR: Token 获取成功");

                self.credentials = Some(creds);
            }

            Ok(self.credentials.as_ref().unwrap())
        }

        pub fn credentials(&self) -> Option<&DeviceCredentials> {
            self.credentials.as_ref()
        }

        pub async fn start_session(&mut self) -> Result<DoubaoImeRealtimeSession> {
            let creds = self.ensure_credentials().await?.clone();
            let config = self.config.clone();

            let ws_url = format!(
                "{}?aid={}&device_id={}",
                WEBSOCKET_URL, AID, creds.device_id
            );

            tracing::info!("豆包输入法 ASR: 连接 WebSocket...");

            let request = tungstenite::http::Request::builder()
                .uri(&ws_url)
                .header("User-Agent", USER_AGENT)
                .header("proto-version", "v2")
                .header("x-custom-keepalive", "true")
                .header("Host", "frontier-audio-ime-ws.doubao.com")
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", generate_websocket_key())
                .body(())?;

            let (ws_stream, _) = timeout(
                Duration::from_secs(config.connect_timeout_secs),
                connect_async(request),
            )
            .await
            .map_err(|_| anyhow!("WebSocket 连接超时"))??;

            tracing::info!("豆包输入法 ASR: WebSocket 连接成功");

            let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(100);
            let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);

            tokio::spawn(async move {
                if let Err(e) =
                    run_session(ws_stream, cmd_rx, result_tx.clone(), creds, config).await
                {
                    tracing::error!("豆包输入法会话错误: {}", e);
                    let _ = result_tx.send(Err(e)).await;
                }
            });

            Ok(DoubaoImeRealtimeSession {
                sender: cmd_tx,
                result_receiver: result_rx,
            })
        }
    }

    fn generate_websocket_key() -> String {
        use base64::Engine;
        let uuid_bytes = uuid::Uuid::new_v4();
        base64::engine::general_purpose::STANDARD.encode(uuid_bytes.as_bytes())
    }

    async fn run_session(
        ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        mut cmd_rx: mpsc::Receiver<SessionCommand>,
        result_tx: mpsc::Sender<Result<String>>,
        creds: DeviceCredentials,
        config: DoubaoImeClientConfig,
    ) -> Result<()> {
        let (mut ws_write, mut ws_read) = ws_stream.split();

        let request_id = uuid::Uuid::new_v4().to_string();

        // 1. 发送 StartTask
        let start_task = AsrRequest {
            token: creds.token.clone(),
            service_name: "ASR".to_string(),
            method_name: "StartTask".to_string(),
            request_id: request_id.clone(),
            ..Default::default()
        };
        ws_write
            .send(tungstenite::Message::Binary(start_task.encode().into()))
            .await?;

        let msg = ws_read
            .next()
            .await
            .ok_or_else(|| anyhow!("WebSocket closed unexpectedly"))??;
        let response = AsrResponse::decode(&msg.into_data())?;
        if response.message_type != "TaskStarted" {
            return Err(anyhow!(
                "Expected TaskStarted, got: {}",
                response.message_type
            ));
        }
        tracing::debug!("豆包输入法 ASR: TaskStarted");

        // 2. 发送 StartSession
        let session_config = serde_json::json!({
            "audio_info": {
                "channel": config.channels,
                "format": "speech_opus",
                "sample_rate": config.sample_rate
            },
            "enable_punctuation": config.enable_punctuation,
            "enable_speech_rejection": false,
            "extra": {
                "app_name": "com.android.chrome",
                "cell_compress_rate": 8,
                "did": creds.device_id,
                "enable_asr_threepass": true,
                "enable_asr_twopass": true,
                "input_mode": "tool"
            }
        });

        let start_session = AsrRequest {
            token: creds.token.clone(),
            service_name: "ASR".to_string(),
            method_name: "StartSession".to_string(),
            request_id: request_id.clone(),
            payload: session_config.to_string(),
            ..Default::default()
        };
        ws_write
            .send(tungstenite::Message::Binary(start_session.encode().into()))
            .await?;

        let msg = ws_read
            .next()
            .await
            .ok_or_else(|| anyhow!("WebSocket closed unexpectedly"))??;
        let response = AsrResponse::decode(&msg.into_data())?;
        if response.message_type != "SessionStarted" {
            return Err(anyhow!(
                "Expected SessionStarted, got: {} - {}",
                response.message_type,
                response.status_message
            ));
        }
        tracing::debug!("豆包输入法 ASR: SessionStarted");

        // 创建 Opus 编码器
        let mut encoder = OpusEncoder::new(
            config.sample_rate,
            config.channels,
            config.frame_duration_ms,
        )?;

        let samples_per_frame = encoder.samples_per_frame();
        let bytes_per_frame = samples_per_frame * 2;

        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut frame_index = 0u64;
        let mut final_text = String::new();
        let mut has_final_result = false;
        let timestamp_ms = chrono_timestamp_ms();

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(SessionCommand::SendAudio(data)) => {
                            pcm_buffer.extend(data);

                            while pcm_buffer.len() >= bytes_per_frame {
                                let pcm_frame: Vec<u8> = pcm_buffer.drain(..bytes_per_frame).collect();

                                let pcm_i16: Vec<i16> = pcm_frame
                                    .chunks_exact(2)
                                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                                    .collect();

                                let opus_frame = encoder.encode_frame(&pcm_i16)?;

                                let frame_state = if frame_index == 0 {
                                    FrameState::First
                                } else {
                                    FrameState::Middle
                                };

                                let request = AsrRequest {
                                    service_name: "ASR".to_string(),
                                    method_name: "TaskRequest".to_string(),
                                    request_id: request_id.clone(),
                                    payload: serde_json::json!({
                                        "extra": {},
                                        "timestamp_ms": timestamp_ms + (frame_index as i64) * (config.frame_duration_ms as i64)
                                    }).to_string(),
                                    audio_data: opus_frame,
                                    frame_state: frame_state.as_i32(),
                                    ..Default::default()
                                };

                                ws_write.send(tungstenite::Message::Binary(request.encode().into())).await?;
                                frame_index += 1;
                            }
                        }
                        Some(SessionCommand::Finish) => {
                            if !pcm_buffer.is_empty() {
                                while pcm_buffer.len() < bytes_per_frame {
                                    pcm_buffer.push(0);
                                }

                                let pcm_i16: Vec<i16> = pcm_buffer
                                    .chunks_exact(2)
                                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                                    .collect();

                                let opus_frame = encoder.encode_frame(&pcm_i16)?;

                                let request = AsrRequest {
                                    service_name: "ASR".to_string(),
                                    method_name: "TaskRequest".to_string(),
                                    request_id: request_id.clone(),
                                    payload: serde_json::json!({
                                        "extra": {},
                                        "timestamp_ms": timestamp_ms + (frame_index as i64) * (config.frame_duration_ms as i64)
                                    }).to_string(),
                                    audio_data: opus_frame,
                                    frame_state: FrameState::Last.as_i32(),
                                    ..Default::default()
                                };

                                ws_write.send(tungstenite::Message::Binary(request.encode().into())).await?;
                            } else if frame_index > 0 {
                                let silent_pcm = vec![0i16; samples_per_frame];
                                let opus_frame = encoder.encode_frame(&silent_pcm)?;

                                let request = AsrRequest {
                                    service_name: "ASR".to_string(),
                                    method_name: "TaskRequest".to_string(),
                                    request_id: request_id.clone(),
                                    payload: serde_json::json!({
                                        "extra": {},
                                        "timestamp_ms": timestamp_ms + (frame_index as i64) * (config.frame_duration_ms as i64)
                                    }).to_string(),
                                    audio_data: opus_frame,
                                    frame_state: FrameState::Last.as_i32(),
                                    ..Default::default()
                                };

                                ws_write.send(tungstenite::Message::Binary(request.encode().into())).await?;
                            }

                            let finish_session = AsrRequest {
                                token: creds.token.clone(),
                                service_name: "ASR".to_string(),
                                method_name: "FinishSession".to_string(),
                                request_id: request_id.clone(),
                                ..Default::default()
                            };
                            ws_write.send(tungstenite::Message::Binary(finish_session.encode().into())).await?;
                            tracing::debug!("豆包输入法 ASR: 已发送 FinishSession");
                        }
                        None => break,
                    }
                }

                msg = ws_read.next() => {
                    match msg {
                        Some(Ok(tungstenite::Message::Binary(data))) => {
                            let response = AsrResponse::decode(&data)?;

                            match response.message_type.as_str() {
                                "TaskFailed" | "SessionFailed" => {
                                    let _ = result_tx.send(Err(anyhow!(
                                        "ASR 失败: {}",
                                        response.status_message
                                    ))).await;
                                    return Ok(());
                                }
                                "SessionFinished" => {
                                    let _ = result_tx.send(Ok(final_text.clone())).await;
                                    return Ok(());
                                }
                                other_type => {
                                    tracing::debug!("豆包输入法 ASR: 收到消息类型: {}", other_type);
                                    if !response.result_json.is_empty() {
                                        tracing::debug!("豆包输入法 ASR: result_json = {}", response.result_json);
                                        if let Some((candidate_text, is_final)) =
                                            extract_text_candidate_from_result_json(
                                                &response.result_json,
                                            )
                                        {
                                            if is_final {
                                                final_text = candidate_text;
                                                has_final_result = true;
                                                tracing::info!("豆包输入法 ASR: ✓ 最终结果: {}", final_text);
                                            } else if !has_final_result {
                                                final_text = candidate_text;
                                                tracing::debug!("豆包输入法 ASR: 更新中间结果: {}", final_text);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            let _ = result_tx.send(Err(anyhow!("WebSocket 错误: {}", e))).await;
                            return Ok(());
                        }
                        None => {
                            // WebSocket 关闭但未收到 SessionFinished，仍发送已累积的结果
                            let _ = result_tx
                                .send(finalize_unexpected_ws_close_result(&final_text))
                                .await;
                            return Ok(());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ==================== HTTP 客户端 ====================

    pub struct DoubaoImeClient {
        client: reqwest::Client,
        config: DoubaoImeClientConfig,
        credentials: Option<DeviceCredentials>,
    }

    impl DoubaoImeClient {
        pub fn new(client: reqwest::Client) -> Self {
            Self {
                client,
                config: DoubaoImeClientConfig::default(),
                credentials: None,
            }
        }

        pub fn with_credentials(client: reqwest::Client, credentials: DeviceCredentials) -> Self {
            Self {
                client,
                config: DoubaoImeClientConfig::default(),
                credentials: Some(credentials),
            }
        }

        pub async fn ensure_credentials(&mut self) -> Result<&DeviceCredentials> {
            if self.credentials.is_none() {
                let mut creds = register_device(&self.client).await?;
                creds.token = get_asr_token(&self.client, &creds.device_id, &creds.cdid).await?;
                self.credentials = Some(creds);
            }
            Ok(self.credentials.as_ref().unwrap())
        }

        pub async fn transcribe_wav(&mut self, wav_path: &std::path::Path) -> Result<String> {
            let mut reader = hound::WavReader::open(wav_path)?;
            let spec = reader.spec();

            if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
                return Err(anyhow!("只支持 16-bit PCM WAV 文件"));
            }

            let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?;

            let pcm_data = if spec.sample_rate != 16000 {
                resample_linear(&samples, spec.sample_rate, 16000)
            } else {
                samples
            };

            let pcm_bytes: Vec<u8> = pcm_data.iter().flat_map(|&s| s.to_le_bytes()).collect();

            self.transcribe_pcm(&pcm_bytes).await
        }

        pub async fn transcribe_pcm(&mut self, pcm_bytes: &[u8]) -> Result<String> {
            let creds = self.ensure_credentials().await?.clone();
            let config = self.config.clone();

            let ws_url = format!(
                "{}?aid={}&device_id={}",
                WEBSOCKET_URL, AID, creds.device_id
            );

            let request = tungstenite::http::Request::builder()
                .uri(&ws_url)
                .header("User-Agent", USER_AGENT)
                .header("proto-version", "v2")
                .header("x-custom-keepalive", "true")
                .header("Host", "frontier-audio-ime-ws.doubao.com")
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", generate_websocket_key())
                .body(())?;

            let (ws_stream, _) = connect_async(request).await?;
            let (mut ws_write, mut ws_read) = ws_stream.split();

            let request_id = uuid::Uuid::new_v4().to_string();

            // StartTask
            let start_task = AsrRequest {
                token: creds.token.clone(),
                service_name: "ASR".to_string(),
                method_name: "StartTask".to_string(),
                request_id: request_id.clone(),
                ..Default::default()
            };
            ws_write
                .send(tungstenite::Message::Binary(start_task.encode().into()))
                .await?;

            let msg = ws_read
                .next()
                .await
                .ok_or_else(|| anyhow!("WebSocket closed"))??;
            let response = AsrResponse::decode(&msg.into_data())?;
            if response.message_type != "TaskStarted" {
                return Err(anyhow!(
                    "Expected TaskStarted, got: {}",
                    response.message_type
                ));
            }

            // StartSession
            let session_config = serde_json::json!({
                "audio_info": {
                    "channel": config.channels,
                    "format": "speech_opus",
                    "sample_rate": config.sample_rate
                },
                "enable_punctuation": config.enable_punctuation,
                "enable_speech_rejection": false,
                "extra": {
                    "app_name": "com.android.chrome",
                    "cell_compress_rate": 8,
                    "did": creds.device_id,
                    "enable_asr_threepass": true,
                    "enable_asr_twopass": true,
                    "input_mode": "tool"
                }
            });

            let start_session = AsrRequest {
                token: creds.token.clone(),
                service_name: "ASR".to_string(),
                method_name: "StartSession".to_string(),
                request_id: request_id.clone(),
                payload: session_config.to_string(),
                ..Default::default()
            };
            ws_write
                .send(tungstenite::Message::Binary(start_session.encode().into()))
                .await?;

            let msg = ws_read
                .next()
                .await
                .ok_or_else(|| anyhow!("WebSocket closed"))??;
            let response = AsrResponse::decode(&msg.into_data())?;
            if response.message_type != "SessionStarted" {
                return Err(anyhow!(
                    "Expected SessionStarted, got: {}",
                    response.message_type
                ));
            }

            // 编码并发送音频
            let mut encoder = OpusEncoder::new(
                config.sample_rate,
                config.channels,
                config.frame_duration_ms,
            )?;

            let samples_per_frame = encoder.samples_per_frame();
            let bytes_per_frame = samples_per_frame * 2;
            let timestamp_ms = chrono_timestamp_ms();

            let mut offset = 0;
            let mut frame_index = 0u64;

            while offset < pcm_bytes.len() {
                let end = std::cmp::min(offset + bytes_per_frame, pcm_bytes.len());
                let mut chunk = pcm_bytes[offset..end].to_vec();

                while chunk.len() < bytes_per_frame {
                    chunk.push(0);
                }

                let pcm_i16: Vec<i16> = chunk
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();

                let opus_frame = encoder.encode_frame(&pcm_i16)?;

                let frame_state = if frame_index == 0 {
                    FrameState::First
                } else if offset + bytes_per_frame >= pcm_bytes.len() {
                    FrameState::Last
                } else {
                    FrameState::Middle
                };

                let request = AsrRequest {
                    service_name: "ASR".to_string(),
                    method_name: "TaskRequest".to_string(),
                    request_id: request_id.clone(),
                    payload: serde_json::json!({
                        "extra": {},
                        "timestamp_ms": timestamp_ms + (frame_index as i64) * (config.frame_duration_ms as i64)
                    })
                    .to_string(),
                    audio_data: opus_frame,
                    frame_state: frame_state.as_i32(),
                    ..Default::default()
                };

                ws_write
                    .send(tungstenite::Message::Binary(request.encode().into()))
                    .await?;

                offset += bytes_per_frame;
                frame_index += 1;
            }

            // FinishSession
            let finish_session = AsrRequest {
                token: creds.token.clone(),
                service_name: "ASR".to_string(),
                method_name: "FinishSession".to_string(),
                request_id: request_id.clone(),
                ..Default::default()
            };
            ws_write
                .send(tungstenite::Message::Binary(finish_session.encode().into()))
                .await?;

            // 等待结果
            let mut final_text = String::new();
            let mut has_final_result = false;

            while let Some(msg) = ws_read.next().await {
                let msg = msg?;
                if let tungstenite::Message::Binary(data) = msg {
                    let response = AsrResponse::decode(&data)?;

                    match response.message_type.as_str() {
                        "TaskFailed" | "SessionFailed" => {
                            return Err(anyhow!("ASR 失败: {}", response.status_message));
                        }
                        "SessionFinished" => {
                            break;
                        }
                        other_type => {
                            tracing::debug!("豆包输入法 ASR (HTTP): 收到消息类型: {}", other_type);
                            if !response.result_json.is_empty() {
                                tracing::debug!(
                                    "豆包输入法 ASR (HTTP): result_json = {}",
                                    response.result_json
                                );
                                if let Some((candidate_text, is_final)) =
                                    extract_text_candidate_from_result_json(&response.result_json)
                                {
                                    if is_final {
                                        final_text = candidate_text;
                                        has_final_result = true;
                                        tracing::info!(
                                            "豆包输入法 ASR (HTTP): ✓ 最终结果: {}",
                                            final_text
                                        );
                                    } else if !has_final_result {
                                        final_text = candidate_text;
                                        tracing::debug!(
                                            "豆包输入法 ASR (HTTP): 更新中间结果: {}",
                                            final_text
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(final_text)
        }
    }

    fn resample_linear(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (samples.len() as f64 / ratio) as usize;
        let mut result = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f64 * ratio;
            let idx = src_idx as usize;
            let frac = src_idx - idx as f64;

            if idx + 1 < samples.len() {
                let sample = samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac;
                result.push(sample as i16);
            } else if idx < samples.len() {
                result.push(samples[idx]);
            }
        }

        result
    }

    #[cfg(test)]
    mod tests {
        use super::{
            extract_text_candidate_from_result_json, finalize_unexpected_ws_close_result,
            with_request_timeout, CREDENTIAL_REQUEST_TIMEOUT_SECS,
        };
        use tokio::time::{sleep, Duration};

        #[test]
        fn extract_uses_latest_interim_when_no_final_signal() {
            let result_json = r#"{
                "results": [
                    {"text": "你", "is_interim": true, "is_vad_finished": false, "extra": {"nonstream_result": false}},
                    {"text": "你好", "is_interim": true, "is_vad_finished": false, "extra": {"nonstream_result": false}}
                ]
            }"#;

            let extracted =
                extract_text_candidate_from_result_json(result_json).expect("should extract");
            assert_eq!(extracted.0, "你好");
            assert!(!extracted.1);
        }

        #[test]
        fn extract_prefers_final_when_vad_finished() {
            let result_json = r#"{
                "results": [
                    {"text": "你好", "is_interim": true, "is_vad_finished": false, "extra": {"nonstream_result": false}},
                    {"text": "你好呀", "is_interim": false, "is_vad_finished": true, "extra": {"nonstream_result": false}}
                ]
            }"#;

            let extracted =
                extract_text_candidate_from_result_json(result_json).expect("should extract");
            assert_eq!(extracted.0, "你好呀");
            assert!(extracted.1);
        }

        #[test]
        fn unexpected_ws_close_requires_non_empty_text() {
            let err = finalize_unexpected_ws_close_result("")
                .expect_err("empty final text should be treated as error");
            assert!(
                err.to_string().contains("empty"),
                "error should explain that final text is empty"
            );

            let ok = finalize_unexpected_ws_close_result("hello")
                .expect("non-empty final text should be returned");
            assert_eq!(ok, "hello");
        }

        #[tokio::test]
        async fn request_timeout_fails_for_slow_response() {
            let result = with_request_timeout("timeout-test", async {
                sleep(Duration::from_millis(
                    (CREDENTIAL_REQUEST_TIMEOUT_SECS * 1000) + 50,
                ))
                .await;
                Ok::<_, reqwest::Error>(())
            })
            .await;

            assert!(result.is_err(), "slow request should time out");
        }

        #[tokio::test]
        async fn request_timeout_allows_fast_response() {
            let result = with_request_timeout("fast-test", async {
                Ok::<_, reqwest::Error>("ok")
            })
            .await
            .expect("fast request should succeed");

            assert_eq!(result, "ok");
        }
    }
}

// 当功能启用时，从 implementation 模块重新导出
#[cfg(feature = "doubao-ime")]
pub use implementation::{DoubaoImeClient, DoubaoImeRealtimeClient, DoubaoImeRealtimeSession};
