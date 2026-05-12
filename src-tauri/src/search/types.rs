use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResultItem {
    pub index: u32,
    pub id: String,
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub query: String,
    pub provider_id: String,
    pub provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub items: Vec<SearchResultItem>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCallSummary {
    pub id: String,
    pub name: String,
    pub query: String,
    pub status: String,
    pub results_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
    pub round: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantToolCall {
    pub id: String,
    pub name: String,
    pub query: String,
    pub status: String,
    pub results: Vec<SearchResultItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
    pub round: u32,
}

impl AssistantToolCall {
    pub fn summary(&self) -> ToolCallSummary {
        ToolCallSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            query: self.query.clone(),
            status: self.status.clone(),
            results_count: self.results.len(),
            error: self.error.clone(),
            elapsed_ms: self.elapsed_ms,
            round: self.round,
        }
    }
}

pub(crate) fn random_citation_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

pub(crate) fn source_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

pub(crate) fn truncate_items(
    mut items: Vec<SearchResultItem>,
    max_results: u32,
) -> Vec<SearchResultItem> {
    let max = max_results.max(1) as usize;
    items.truncate(max);
    for (idx, item) in items.iter_mut().enumerate() {
        item.index = (idx + 1) as u32;
        if item.id.is_empty() {
            item.id = random_citation_id();
        }
        if item.source.is_none() {
            item.source = source_from_url(&item.url);
        }
    }
    items
}
