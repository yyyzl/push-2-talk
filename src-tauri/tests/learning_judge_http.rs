//! Exercise the real judge/client boundary with provider-shaped HTTP responses.
#![allow(dead_code)]
#[path = "../src/learning/llm_judge.rs"]
mod llm_judge;
#[path = "../src/openai_client.rs"]
mod openai_client;

use llm_judge::LlmJudge;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn provider(
    reply: impl FnOnce(Value) -> Value + Send + 'static,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let body = loop {
            let mut buffer = [0; 4096];
            let count = socket.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0, "client closed before sending request");
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break serde_json::from_slice(&request[end + 4..end + 4 + length]).unwrap();
                }
            }
        };
        let response = reply(body).to_string();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).as_bytes()).await.unwrap();
    });
    (endpoint, task)
}

fn completed() -> Value {
    json!({"choices": [{"finish_reason": "stop", "message": {"content":
        r#"{"should_learn":true,"word":"Kubernetes","category":"term","reason":"容器编排术语"}"#
    }}]})
}

#[tokio::test]
async fn reasoning_tokens_leave_room_for_learning_json() {
    let (endpoint, server) = provider(|request| {
        assert_eq!(request["model"], "reasoning-model");
        assert_eq!(request["messages"][0]["role"], "system");
        assert!(request["messages"][1]["content"].as_str().unwrap().contains("Kubernetes"));
        // Captured failure shape: the provider uses the completion budget for
        // reasoning, then truncates before generating any visible JSON.
        if request["max_tokens"].as_u64().unwrap() < 477 {
            json!({"choices": [{"finish_reason":"length", "message": {
                "content":"", "reasoning_content":"internal reasoning"
            }}], "usage":{"completion_tokens":256,"completion_tokens_details":{"reasoning_tokens":256}}})
        } else {
            completed()
        }
    }).await;
    let result = LlmJudge::new(&endpoint, "test-only", "reasoning-model")
        .judge("库伯内特斯", "Kubernetes", "使用 Kubernetes 管理容器")
        .await;
    server.await.unwrap();
    let result =
        result.expect("reasoning-capable provider should still produce a learning suggestion");
    assert!(result.should_learn);
    assert_eq!(result.word, "Kubernetes");
}

#[tokio::test]
async fn ordinary_model_keeps_short_json_contract() {
    let (endpoint, server) = provider(|_| completed()).await;
    let result = LlmJudge::new(&endpoint, "test-only", "ordinary-model")
        .judge("库伯内特斯", "Kubernetes", "使用 Kubernetes 管理容器")
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(result.category, "term");
}

#[tokio::test]
async fn reasoning_content_is_never_treated_as_a_learning_result() {
    let (endpoint, server) = provider(|_| {
        json!({"choices":[{"finish_reason":"length", "message":{
            "content":"", "reasoning_content": completed()["choices"][0]["message"]["content"]
        }}]})
    })
    .await;
    let result = LlmJudge::new(&endpoint, "test-only", "reasoning-model")
        .judge("库伯内特斯", "Kubernetes", "使用 Kubernetes 管理容器")
        .await;
    server.await.unwrap();
    assert!(
        result.is_err(),
        "incomplete reasoning must not add a vocabulary entry"
    );
}

#[tokio::test]
#[ignore = "Calls an explicitly configured live provider; needs PTT_TEST_CONFIG and PTT_TEST_PROVIDER_ID"]
async fn live_learning_provider_returns_a_suggestion() {
    let config: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("PTT_TEST_CONFIG").expect("PTT_TEST_CONFIG")).unwrap(),
    )
    .unwrap();
    let id = std::env::var("PTT_TEST_PROVIDER_ID").expect("PTT_TEST_PROVIDER_ID");
    let provider = config["llm_config"]["shared"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("configured provider");
    let started = std::time::Instant::now();
    let result = LlmJudge::new(
        provider["endpoint"].as_str().unwrap(),
        provider["api_key"].as_str().unwrap(),
        provider["default_model"].as_str().unwrap(),
    )
    .judge(
        "库伯内特斯",
        "Kubernetes",
        "这次测试使用 Kubernetes 管理容器，服务运行正常。",
    )
    .await
    .expect("live learning response");
    assert!(result.should_learn);
    assert!(result.word.to_lowercase().contains("kubernetes"));
    println!(
        "Live learning suggestion received in {:?}; category={}",
        started.elapsed(),
        result.category
    );
}
