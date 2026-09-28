use std::time::{Duration, Instant};

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

use crate::config::SearchProviderConfig;
use crate::search::types::{truncate_items, SearchResult, SearchResultItem};
use crate::search::{send_with_body_timeout, SearchService};

const DEFAULT_ENDPOINT: &str = "https://api.bochaai.com/v1/web-search";

pub struct BochaSearchService {
    config: SearchProviderConfig,
    client: Client,
}

impl BochaSearchService {
    pub fn new(config: SearchProviderConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl SearchService for BochaSearchService {
    async fn search(
        &self,
        query: &str,
        max_results: u32,
        timeout: Duration,
    ) -> Result<SearchResult> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Bocha API Key 不能为空"))?;
        let endpoint = self.config.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let started = Instant::now();

        let request = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "query": query,
                "summary": true,
                "count": max_results.max(1)
            }));

        let (status, body) = send_with_body_timeout(request, timeout, "Bocha 搜索超时").await?;
        if !status.is_success() {
            anyhow::bail!("Bocha 搜索失败 ({}): {}", status, body);
        }
        let payload: Value = serde_json::from_str(&body)?;
        let mut result = parse_bocha_response(&self.config.id, query, &payload, max_results)?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }

    async fn test_connection(&self) -> Result<u32> {
        let started = Instant::now();
        self.search("test", 1, Duration::from_secs(6)).await?;
        Ok(started.elapsed().as_millis() as u32)
    }
}

pub(crate) fn parse_bocha_response(
    provider_id: &str,
    query: &str,
    payload: &Value,
    max_results: u32,
) -> Result<SearchResult> {
    let web_pages = payload
        .pointer("/webPages/value")
        .or_else(|| payload.pointer("/data/webPages/value"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Bocha 响应缺少 webPages.value[]"))?;

    let items = web_pages
        .iter()
        .map(|item| SearchResultItem {
            index: 0,
            id: String::new(),
            title: item["name"].as_str().unwrap_or("Untitled").to_string(),
            url: item["url"].as_str().unwrap_or_default().to_string(),
            snippet: item["summary"]
                .as_str()
                .or_else(|| item["snippet"].as_str())
                .unwrap_or_default()
                .to_string(),
            source: item["siteName"].as_str().map(ToString::to_string),
        })
        .filter(|item| !item.url.trim().is_empty())
        .collect();

    Ok(SearchResult {
        query: query.to_string(),
        provider_id: provider_id.to_string(),
        provider_type: "bocha".to_string(),
        answer: None,
        items: truncate_items(items, max_results),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_top_level_and_wrapped_bocha_results() {
        let top = serde_json::json!({
            "webPages": {"value": [{"name": "A", "url": "https://a.test", "summary": "sa"}]}
        });
        let wrapped = serde_json::json!({
            "data": {"webPages": {"value": [{"name": "B", "url": "https://b.test", "snippet": "sb"}]}}
        });

        assert_eq!(
            parse_bocha_response("bo", "q", &top, 5).unwrap().items[0].title,
            "A"
        );
        assert_eq!(
            parse_bocha_response("bo", "q", &wrapped, 5).unwrap().items[0].title,
            "B"
        );
    }
}
