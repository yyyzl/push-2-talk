use super::*;
use crate::asr::qwen_models::catalogue;

#[tokio::test]
async fn every_realtime_model_completes_a_real_websocket_exchange() {
    for model in catalogue().iter().filter(|m| m.mode == QwenMode::Realtime) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_hdr_async(
                socket,
                |req: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(req.headers()["authorization"], "Bearer fixture-key");
                    if model.protocol == QwenProtocol::Qwen3 {
                        assert_eq!(
                            req.uri().query(),
                            Some(format!("model={}", model.id).as_str())
                        );
                        assert_eq!(req.headers()["openai-beta"], "realtime=v1");
                    }
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let first: serde_json::Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            if model.protocol == QwenProtocol::Qwen3 {
                assert_eq!(first["type"], "session.update");
                assert_eq!(
                    first["session"]["input_audio_transcription"]["language"],
                    "zh"
                );
                assert_eq!(first["session"]["turn_detection"], serde_json::Value::Null);
            } else {
                assert_eq!(first["header"]["action"], "run-task");
                assert_eq!(first["payload"]["model"], model.id);
                assert_eq!(first["payload"]["parameters"]["sample_rate"], 16000);
                assert_eq!(first["payload"]["parameters"]["vocabulary"]["Rust"], 4);
                if model.protocol == QwenProtocol::Message {
                    assert!(first["payload"]["parameters"]
                        .get("language_hints")
                        .is_none());
                }
                ws.send(Message::Text(
                    serde_json::json!({"header":{"event":"task-started"},"payload":{}}).to_string(),
                ))
                .await
                .unwrap();
            }
            let audio = ws.next().await.unwrap().unwrap();
            if model.protocol == QwenProtocol::Qwen3 {
                let audio: serde_json::Value =
                    serde_json::from_str(audio.to_text().unwrap()).unwrap();
                assert_eq!(audio["type"], "input_audio_buffer.append");
                assert!(!audio["audio"].as_str().unwrap().is_empty());
            } else {
                assert!(matches!(audio, Message::Binary(_)));
            }
            let commit: serde_json::Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            if model.protocol == QwenProtocol::Qwen3 {
                assert_eq!(commit["type"], "input_audio_buffer.commit");
                ws.send(Message::Text(serde_json::json!({"type":"conversation.item.input_audio_transcription.completed","transcript":"第一句第二句"}).to_string())).await.unwrap();
            } else {
                assert_eq!(commit["header"]["action"], "finish-task");
                for (text, final_sentence) in [
                    ("不会拼接的中间结果", false),
                    ("第一句", true),
                    ("第二句", true),
                ] {
                    ws.send(Message::Text(serde_json::json!({"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"text":text,"sentence_end":final_sentence}}}}).to_string())).await.unwrap();
                }
                ws.send(Message::Text(
                    serde_json::json!({"header":{"event":"task-finished"},"payload":{}})
                        .to_string(),
                ))
                .await
                .unwrap();
            }
        });
        let pool = ConnectionPool::new_with_model_and_correction_pairs(
            "fixture-key".into(),
            vec!["Rust".into()],
            vec![],
            AsrLanguageMode::Zh,
            model,
        );
        let url = if model.protocol == QwenProtocol::Qwen3 {
            format!("ws://{address}/realtime?model={}", model.id)
        } else {
            format!("ws://{address}/inference")
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut session = pool.create_session_at(&url).await.unwrap();
            session.send_audio_chunk(&[1, 2, 3, 4]).await.unwrap();
            session.commit_audio().await.unwrap();
            assert_eq!(session.wait_for_result().await.unwrap(), "第一句第二句");
            server.await.unwrap();
        })
        .await
        .unwrap();
    }
}
