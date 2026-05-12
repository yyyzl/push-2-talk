use std::time::{Duration, Instant};

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

use crate::config::SearchProviderConfig;
use crate::search::types::{truncate_items, SearchResult, SearchResultItem};
use crate::search::{send_with_body_timeout, SearchService};

pub struct SearxngSearchService {
    config: SearchProviderConfig,
    client: Client,
}

impl SearxngSearchService {
    pub fn new(config: SearchProviderConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    fn endpoint(&self) -> Result<String> {
        let base = self
            .config
            .endpoint
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("SearXNG endpoint 不能为空"))?
            .trim()
            .trim_end_matches('/');
        if base.ends_with("/search") {
            Ok(base.to_string())
        } else {
            Ok(format!("{base}/search"))
        }
    }
}

#[async_trait::async_trait]
impl SearchService for SearxngSearchService {
    async fn search(
        &self,
        query: &str,
        max_results: u32,
        timeout: Duration,
    ) -> Result<SearchResult> {
        let started = Instant::now();
        let endpoint = self.endpoint()?;
        let mut request = self
            .client
            .get(endpoint)
            .query(&[("q", query), ("format", "json")]);

        if let Some(language) = self
            .config
            .searxng_language
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            request = request.query(&[("language", language)]);
        }
        if let Some(time_range) = self
            .config
            .searxng_time_range
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            request = request.query(&[("time_range", time_range)]);
        }
        if let Some(username) = self
            .config
            .basic_auth_username
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        {
            request = request.basic_auth(username, self.config.basic_auth_password.clone());
        }

        let (status, body) = send_with_body_timeout(request, timeout, "SearXNG 搜索超时").await?;
        if status.as_u16() == 403 {
            anyhow::bail!("SearXNG 返回 403，可能未启用 JSON format 或鉴权失败");
        }
        if !status.is_success() {
            anyhow::bail!("SearXNG 搜索失败 ({}): {}", status, body);
        }
        let payload: Value = serde_json::from_str(&body)?;
        let mut result = parse_searxng_response(&self.config.id, query, &payload, max_results)?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }

    async fn test_connection(&self) -> Result<u32> {
        let started = Instant::now();
        self.search("test", 1, Duration::from_secs(6)).await?;
        Ok(started.elapsed().as_millis() as u32)
    }
}

pub(crate) fn parse_searxng_response(
    provider_id: &str,
    query: &str,
    payload: &Value,
    max_results: u32,
) -> Result<SearchResult> {
    let results = payload["results"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("SearXNG 响应缺少 results[]"))?;

    let items = results
        .iter()
        .map(|item| SearchResultItem {
            index: 0,
            id: String::new(),
            title: item["title"].as_str().unwrap_or("Untitled").to_string(),
            url: item["url"].as_str().unwrap_or_default().to_string(),
            snippet: item["content"].as_str().unwrap_or_default().to_string(),
            source: item["engine"].as_str().map(ToString::to_string),
        })
        .filter(|item| !item.url.trim().is_empty())
        .collect();

    Ok(SearchResult {
        query: query.to_string(),
        provider_id: provider_id.to_string(),
        provider_type: "searxng".to_string(),
        answer: None,
        items: truncate_items(items, max_results),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_searxng_results() {
        let payload = serde_json::json!({
            "results": [{"title": "A", "url": "https://a.test", "content": "sa", "engine": "google"}]
        });

        let result = parse_searxng_response("sx", "q", &payload, 5).unwrap();

        assert_eq!(result.items[0].title, "A");
        assert_eq!(result.items[0].source.as_deref(), Some("google"));
    }

    #[test]
    fn endpoint_accepts_base_url_or_search_url() {
        let mut config = SearchProviderConfig {
            id: "sx".to_string(),
            provider_type: crate::config::SearchProviderType::Searxng,
            display_name: "SearXNG".to_string(),
            enabled: true,
            endpoint: Some("http://127.0.0.1:8080".to_string()),
            api_key: None,
            basic_auth_username: None,
            basic_auth_password: None,
            serper_gl: None,
            serper_hl: None,
            serper_tbs: None,
            searxng_language: None,
            searxng_time_range: None,
        };

        let service = SearxngSearchService::new(config.clone());
        assert_eq!(service.endpoint().unwrap(), "http://127.0.0.1:8080/search");

        config.endpoint = Some("http://127.0.0.1:8080/search".to_string());
        let service = SearxngSearchService::new(config);
        assert_eq!(service.endpoint().unwrap(), "http://127.0.0.1:8080/search");
    }
}
