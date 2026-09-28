use super::*;
use crate::asr::qwen_models::catalogue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn each_selected_http_model_reaches_the_server_with_its_own_protocol() {
    for model in catalogue().iter().filter(|m| m.mode == QwenMode::Http) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let body = loop {
                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0, "request unexpectedly closed");
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(header_end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= header_end + 4 + len {
                        assert!(headers.contains("authorization: bearer fixture-key"));
                        break serde_json::from_slice::<serde_json::Value>(
                            &bytes[header_end + 4..header_end + 4 + len],
                        )
                        .unwrap();
                    }
                }
            };
            assert_eq!(body["model"], model.id);
            match model.protocol {
                QwenProtocol::Qwen3 => {
                    assert!(body["input"]["messages"][1]["content"][0]["audio"]
                        .as_str()
                        .unwrap()
                        .starts_with("data:audio/wav;base64,"));
                    assert_eq!(body["parameters"]["asr_options"]["enable_itn"], true);
                }
                QwenProtocol::Audio3 => {
                    assert_eq!(
                        body["input"]["messages"][0]["content"][0]["type"],
                        "input_audio"
                    );
                    assert_eq!(body["parameters"]["vocabulary"]["Rust"], 4);
                    assert_eq!(
                        body["parameters"]["language_hints"],
                        serde_json::json!(["zh"])
                    );
                }
                QwenProtocol::Message => panic!("message must not appear in HTTP catalogue"),
            }
            let response = if model.protocol == QwenProtocol::Qwen3 {
                serde_json::json!({"output":{"choices":[{"message":{"content":[{"text":"识别成功。"}]}}]}})
            } else { serde_json::json!({"output":{"text":"识别成功。"}}) }.to_string();
            let http = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response);
            socket.write_all(http.as_bytes()).await.unwrap();
        });
        let mut client = QwenASRClient::new_with_model_and_correction_pairs(
            "fixture-key".into(),
            vec!["Rust".into()],
            vec![],
            AsrLanguageMode::Zh,
            model,
        );
        client.api_url = format!("http://{address}/recognition");
        let text = tokio::time::timeout(
            Duration::from_secs(5),
            client.transcribe_from_memory(b"fixture-wave"),
        )
        .await
        .unwrap()
        .unwrap();
        server.await.unwrap();
        assert_eq!(text, "识别成功");
    }
}
