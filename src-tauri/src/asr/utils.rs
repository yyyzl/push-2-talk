use reqwest::Client;
use std::time::Duration;

/// 创建标准配置的 HTTP 客户端（30s 超时，禁用代理）
pub fn create_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .no_proxy()
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// 去除转录结果末尾的标点符号
pub fn strip_trailing_punctuation(text: &mut String) {
    const PUNCTUATION: &[char] = &[
        '。', '，', '！', '？', '、', '；', '：', '"', '"', '\'', '\'', '.', ',', '!', '?', ';',
        ':',
    ];

    while let Some(last_char) = text.chars().last() {
        if PUNCTUATION.contains(&last_char) {
            text.pop();
        } else {
            break;
        }
    }
}
