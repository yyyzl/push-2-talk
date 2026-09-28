use std::time::{Duration, Instant};

use anyhow::Result;
use reqwest::Client;
use serde_json::{Map, Value};

use crate::config::SearchProviderConfig;
use crate::search::types::{truncate_items, SearchResult, SearchResultItem};
use crate::search::{send_with_body_timeout, SearchService};

const DEFAULT_ENDPOINT: &str = "https://google.serper.dev/search";

pub struct SerperSearchService {
    config: SearchProviderConfig,
    client: Client,
}

impl SerperSearchService {
    pub fn new(config: SearchProviderConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl SearchService for SerperSearchService {
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
            .ok_or_else(|| anyhow::anyhow!("Serper API Key 不能为空"))?;
        let endpoint = self.config.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let started = Instant::now();

        let mut body = Map::new();
        body.insert("q".to_string(), Value::String(query.to_string()));
        body.insert("num".to_string(), Value::Number(max_results.max(1).into()));
        if let Some(gl) = self
            .config
            .serper_gl
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            body.insert("gl".to_string(), Value::String(gl.clone()));
        }
        if let Some(hl) = self
            .config
            .serper_hl
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            body.insert("hl".to_string(), Value::String(hl.clone()));
        }
        if let Some(tbs) = self
            .config
            .serper_tbs
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            body.insert("tbs".to_string(), Value::String(tbs.clone()));
        }

        let request = self
            .client
            .post(endpoint)
            .header("X-API-KEY", api_key)
            .json(&Value::Object(body));

        let (status, body) = send_with_body_timeout(request, timeout, "Serper 搜索超时").await?;
        if !status.is_success() {
            anyhow::bail!("Serper 搜索失败 ({}): {}", status, body);
        }
        let payload: Value = serde_json::from_str(&body)?;
        let mut result = parse_serper_response(&self.config.id, query, &payload, max_results)?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }

    async fn test_connection(&self) -> Result<u32> {
        let started = Instant::now();
        self.search("test", 1, Duration::from_secs(6)).await?;
        Ok(started.elapsed().as_millis() as u32)
    }
}

pub(crate) fn parse_serper_response(
    provider_id: &str,
    query: &str,
    payload: &Value,
    max_results: u32,
) -> Result<SearchResult> {
    let organic = payload["organic"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Serper 响应缺少 organic[]"))?;

    let items = organic
        .iter()
        .map(|item| SearchResultItem {
            index: 0,
            id: String::new(),
            title: item["title"].as_str().unwrap_or("Untitled").to_string(),
            url: item["link"].as_str().unwrap_or_default().to_string(),
            snippet: item["snippet"].as_str().unwrap_or_default().to_string(),
            source: None,
        })
        .filter(|item| !item.url.trim().is_empty())
        .collect();

    Ok(SearchResult {
        query: query.to_string(),
        provider_id: provider_id.to_string(),
        provider_type: "serper".to_string(),
        answer: None,
        items: truncate_items(items, max_results),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_serper_organic_results() {
        let payload = serde_json::json!({
            "organic": [{"title": "A", "link": "https://a.test", "snippet": "sa"}]
        });

        let result = parse_serper_response("sp", "q", &payload, 5).unwrap();

        assert_eq!(result.items[0].title, "A");
        assert_eq!(result.items[0].source.as_deref(), Some("a.test"));
    }
}
