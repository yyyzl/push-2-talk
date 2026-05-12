pub mod bocha;
pub mod registry;
pub mod searxng;
pub mod serper;
pub mod tavily;
pub mod types;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::{RequestBuilder, StatusCode};

pub use registry::SearchRegistry;
pub use types::{AssistantToolCall, SearchResult, SearchResultItem, ToolCallSummary};

#[async_trait]
pub trait SearchService: Send + Sync {
    async fn search(
        &self,
        query: &str,
        max_results: u32,
        timeout: Duration,
    ) -> Result<SearchResult>;
    async fn test_connection(&self) -> Result<u32>;
}

pub(crate) async fn send_with_body_timeout(
    request: RequestBuilder,
    timeout: Duration,
    timeout_message: &'static str,
) -> Result<(StatusCode, String)> {
    tokio::time::timeout(timeout, async move {
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        Ok::<_, anyhow::Error>((status, body))
    })
    .await
    .map_err(|_| anyhow::anyhow!(timeout_message))?
}
