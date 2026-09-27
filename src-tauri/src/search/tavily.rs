use std::time::{Duration, Instant};

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

use crate::config::SearchProviderConfig;
use crate::search::types::{truncate_items, SearchResult, SearchResultItem};
use crate::search::{send_with_body_timeout, SearchService};

const DEFAULT_ENDPOINT: &str = "https://api.tavily.com/search";

pub struct TavilySearchService {
    config: SearchProviderConfig,
    client: Client,
}

impl TavilySearchService {
    pub fn new(config: SearchProviderConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl SearchService for TavilySearchService {
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
            .ok_or_else(|| anyhow::anyhow!("Tavily API Key 不能为空"))?;
        let endpoint = self.config.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let started = Instant::now();

        let request = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "query": query,
                "max_results": max_results.clamp(1, 20),
                "include_answer": true,
                "search_depth": "basic"
            }));

        let (status, body) = send_with_body_timeout(request, timeout, "Tavily 搜索超时").await?;
        if !status.is_success() {
            anyhow::bail!("Tavily 搜索失败 ({}): {}", status, body);
        }
        let payload: Value = serde_json::from_str(&body)?;
        let mut result = parse_tavily_response(&self.config.id, query, &payload, max_results)?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }

    async fn test_connection(&self) -> Result<u32> {
        let started = Instant::now();
        self.search("test", 1, Duration::from_secs(6)).await?;
        Ok(started.elapsed().as_millis() as u32)
    }
}

pub(crate) fn parse_tavily_response(
    provider_id: &str,
    query: &str,
    payload: &Value,
    max_results: u32,
) -> Result<SearchResult> {
    let answer = payload["answer"].as_str().map(ToString::to_string);
    let results = payload["results"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Tavily 响应缺少 results[]"))?;

    let items = results
        .iter()
        .map(|item| SearchResultItem {
            index: 0,
            id: String::new(),
            title: item["title"].as_str().unwrap_or("Untitled").to_string(),
            url: item["url"].as_str().unwrap_or_default().to_string(),
            snippet: item["content"]
                .as_str()
                .or_else(|| item["snippet"].as_str())
                .unwrap_or_default()
                .to_string(),
            source: None,
        })
        .filter(|item| !item.url.trim().is_empty())
        .collect();

    Ok(SearchResult {
        query: query.to_string(),
        provider_id: provider_id.to_string(),
        provider_type: "tavily".to_string(),
        answer,
        items: truncate_items(items, max_results),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tavily_answer_and_results() {
        let payload = serde_json::json!({
            "answer": "摘要",
            "results": [
                {"title": "A", "url": "https://example.com/a", "content": "Alpha"},
                {"title": "B", "url": "https://example.com/b", "content": "Beta"}
            ]
        });

        let result = parse_tavily_response("tv", "q", &payload, 1).unwrap();

        assert_eq!(result.answer.as_deref(), Some("摘要"));
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].index, 1);
        assert_eq!(result.items[0].id.len(), 12);
    }
}
