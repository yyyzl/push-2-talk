use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[test]
fn transport_fallback_never_retries_authentication_quota_or_generic_errors() {
    for status in [401, 403, 429, 500, 503] {
        assert!(!rejects_streaming(status, "stream not supported"));
    }
    for message in [
        "invalid model",
        "stream quota exceeded",
        "invalid tools",
        "rate limit exceeded",
    ] {
        assert!(!rejects_streaming(400, message));
    }
}

#[test]
fn json_compatibility_preserves_tool_calls_and_rejects_error_objects() {
    let payload = serde_json::json!({"choices":[{"message":{"content":null,"tool_calls":[{"id":"call1","type":"function","function":{"name":"search_web","arguments":"{\"query\":\"example\"}"}}]},"finish_reason":"tool_calls"}]});
    let chunk = completion_as_stream_chunk(&payload).unwrap();
    let response = accumulate_stream_chunks(&[chunk]);
    assert_eq!(response.tool_calls[0].id, "call1");
    assert_eq!(
        response.tool_calls[0].function.arguments,
        "{\"query\":\"example\"}"
    );
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    assert!(completion_as_stream_chunk(&serde_json::json!({"error":{"message":"bad"}})).is_err());
}

async fn read_request(socket: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
            assert!(headers.contains("authorization: bearer fixture-key"));
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}

async fn respond(socket: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    socket.write_all(response.as_bytes()).await.unwrap();
}

#[tokio::test]
async fn legacy_json_response_is_displayed_without_changing_the_model_or_sending_twice() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        assert_eq!(request["model"], "my-original-model");
        assert_eq!(request["messages"][0]["content"], "My original prompt");
        respond(
            &mut socket,
            "200 OK",
            "application/json; charset=utf-8",
            r#"{"choices":[{"message":{"content":"旧接口正常回答。"},"finish_reason":"stop"}]}"#,
        )
        .await;
    });
    let client = OpenAiClient::new(
        OpenAiClientConfig::new(endpoint, "fixture-key", "my-original-model").with_timeout_secs(5),
    );
    let mut emitted = String::new();
    let response = client
        .chat_stream(
            &[Message::system("My original prompt")],
            ChatOptions::for_smart_command(),
            None,
            CancellationToken::new(),
            |chunk| emitted.push_str(&chunk.delta_content.unwrap_or_default()),
        )
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(response.content, "旧接口正常回答。");
    assert_eq!(emitted, response.content);
}

#[tokio::test]
async fn explicit_stream_rejection_retries_same_request_without_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut first = read_request(&mut socket).await;
        assert_eq!(first["stream"], true);
        respond(
            &mut socket,
            "400 Bad Request",
            "application/json",
            r#"{"error":{"message":"stream is not supported for this model"}}"#,
        )
        .await;
        drop(socket);
        let (mut socket, _) = listener.accept().await.unwrap();
        let second = read_request(&mut socket).await;
        first["stream"] = serde_json::json!(false);
        assert_eq!(second, first);
        respond(
            &mut socket,
            "200 OK",
            "application/json",
            r#"{"choices":[{"message":{"content":"compatible"},"finish_reason":"stop"}]}"#,
        )
        .await;
    });
    let client = OpenAiClient::new(
        OpenAiClientConfig::new(endpoint, "fixture-key", "old-model").with_timeout_secs(5),
    );
    let result = client
        .chat_stream(
            &[Message::user("test")],
            ChatOptions::for_smart_command(),
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await;
    assert_eq!(result.unwrap().content, "compatible");
    server.await.unwrap();
}

#[tokio::test]
async fn chinese_sse_text_survives_network_chunks_inside_utf8_characters() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/chat/completions", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").await.unwrap();
        let event = "data: {\"choices\":[{\"delta\":{\"content\":\"你好🙂\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        let split = event.find('你').unwrap() + 1;
        for part in [&event.as_bytes()[..split], &event.as_bytes()[split..]] {
            socket
                .write_all(format!("{:x}\r\n", part.len()).as_bytes())
                .await
                .unwrap();
            socket.write_all(part).await.unwrap();
            socket.write_all(b"\r\n").await.unwrap();
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        socket.write_all(b"0\r\n\r\n").await.unwrap();
    });
    let client = OpenAiClient::new(
        OpenAiClientConfig::new(endpoint, "fixture-key", "old-model").with_timeout_secs(5),
    );
    let result = client
        .chat_stream(
            &[Message::user("test")],
            ChatOptions::for_smart_command(),
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(result.content, "你好🙂");
}
