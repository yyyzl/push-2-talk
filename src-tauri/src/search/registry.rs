use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use crate::config::{SearchConfig, SearchProviderConfig, SearchProviderType};
use crate::search::bocha::BochaSearchService;
use crate::search::searxng::SearxngSearchService;
use crate::search::serper::SerperSearchService;
use crate::search::tavily::TavilySearchService;
use crate::search::{SearchResult, SearchService};

struct ProviderEntry {
    config: SearchProviderConfig,
    service: Arc<dyn SearchService>,
}

pub struct SearchRegistry {
    entries: Vec<ProviderEntry>,
    default_provider_id: Option<String>,
    enable_fallback: bool,
}

impl SearchRegistry {
    pub fn from_config(config: SearchConfig) -> Self {
        let entries = config
            .providers
            .into_iter()
            .filter(|provider| provider.enabled)
            .filter(|provider| provider_runtime_config_error(provider).is_none())
            .map(|provider| {
                let service = service_from_provider(&provider);
                ProviderEntry {
                    config: provider,
                    service,
                }
            })
            .collect();

        Self {
            entries,
            default_provider_id: config.default_provider_id,
            enable_fallback: config.enable_fallback,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has_usable_provider(config: &SearchConfig) -> bool {
        config
            .providers
            .iter()
            .any(|provider| provider.enabled && provider_runtime_config_error(provider).is_none())
    }

    pub fn runtime_unavailable_reason(config: &SearchConfig) -> Option<String> {
        let default_id = config
            .default_provider_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let default_provider =
            default_id.and_then(|id| config.providers.iter().find(|provider| provider.id == id));

        if !Self::has_usable_provider(config) {
            if let Some(default_id) = default_id {
                let Some(default_provider) = default_provider else {
                    return Some(format!(
                        "联网搜索默认引擎「{}」不存在，已改为普通回答",
                        default_id
                    ));
                };
                if !default_provider.enabled {
                    return Some(format!(
                        "联网搜索默认引擎「{}」未启用，已改为普通回答",
                        default_provider.display_name
                    ));
                }
                if let Some(reason) = provider_runtime_config_error(default_provider) {
                    return Some(format!(
                        "联网搜索默认引擎「{}」配置无效：{}，已改为普通回答",
                        default_provider.display_name, reason
                    ));
                }
            }
            return Some("联网搜索没有可用引擎，已改为普通回答".to_string());
        }

        let Some(default_provider) = default_provider else {
            return None;
        };

        if !default_provider.enabled {
            return None;
        }

        if let Some(reason) = provider_runtime_config_error(default_provider) {
            tracing::warn!(
                "联网搜索默认引擎「{}」配置无效：{}，将使用其他可用引擎",
                default_provider.display_name,
                reason
            );
        }

        None
    }

    pub async fn search(
        &self,
        query: &str,
        max_results: u32,
        timeout_secs: u32,
    ) -> Result<SearchResult> {
        if query.trim().is_empty() {
            anyhow::bail!("搜索 query 不能为空");
        }

        let timeout = Duration::from_secs(timeout_secs.max(1) as u64);
        let mut order = self.provider_order();
        if !self.enable_fallback && !order.is_empty() {
            order.truncate(1);
        }

        let mut errors = Vec::new();
        for idx in order {
            let entry = &self.entries[idx];
            match entry.service.search(query, max_results, timeout).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    tracing::warn!(
                        "搜索 provider {}({:?}) 失败: {}",
                        entry.config.id,
                        entry.config.provider_type,
                        err
                    );
                    errors.push(format!("{}: {}", entry.config.display_name, err));
                }
            }
        }

        anyhow::bail!("所有搜索引擎均失败: {}", errors.join("; "));
    }

    pub async fn test_provider(config: SearchProviderConfig) -> Result<u32> {
        if !config.enabled {
            anyhow::bail!("搜索引擎未启用");
        }
        service_from_provider(&config).test_connection().await
    }

    fn provider_order(&self) -> Vec<usize> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let mut order = Vec::new();
        if let Some(default_id) = &self.default_provider_id {
            if let Some(idx) = self
                .entries
                .iter()
                .position(|entry| &entry.config.id == default_id)
            {
                order.push(idx);
            }
        }

        for idx in 0..self.entries.len() {
            if !order.contains(&idx) {
                order.push(idx);
            }
        }
        order
    }
}

fn service_from_provider(provider: &SearchProviderConfig) -> Arc<dyn SearchService> {
    match provider.provider_type {
        SearchProviderType::Tavily => Arc::new(TavilySearchService::new(provider.clone())),
        SearchProviderType::Bocha => Arc::new(BochaSearchService::new(provider.clone())),
        SearchProviderType::Serper => Arc::new(SerperSearchService::new(provider.clone())),
        SearchProviderType::Searxng => Arc::new(SearxngSearchService::new(provider.clone())),
    }
}

fn provider_runtime_config_error(provider: &SearchProviderConfig) -> Option<&'static str> {
    match provider.provider_type {
        SearchProviderType::Tavily => {
            required_non_empty(provider.api_key.as_deref()).then_some("Tavily API Key 不能为空")
        }
        SearchProviderType::Bocha => {
            required_non_empty(provider.api_key.as_deref()).then_some("Bocha API Key 不能为空")
        }
        SearchProviderType::Serper => {
            required_non_empty(provider.api_key.as_deref()).then_some("Serper API Key 不能为空")
        }
        SearchProviderType::Searxng => {
            required_non_empty(provider.endpoint.as_deref()).then_some("SearXNG endpoint 不能为空")
        }
    }
}

fn required_non_empty(value: Option<&str>) -> bool {
    value.map(str::trim).unwrap_or_default().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, provider_type: SearchProviderType) -> SearchProviderConfig {
        SearchProviderConfig {
            id: id.to_string(),
            provider_type,
            display_name: id.to_string(),
            enabled: true,
            endpoint: None,
            api_key: Some("key".to_string()),
            basic_auth_username: None,
            basic_auth_password: None,
            serper_gl: None,
            serper_hl: None,
            serper_tbs: None,
            searxng_language: None,
            searxng_time_range: None,
        }
    }

    #[test]
    fn default_provider_is_first_in_order() {
        let registry = SearchRegistry::from_config(SearchConfig {
            providers: vec![
                provider("a", SearchProviderType::Tavily),
                provider("b", SearchProviderType::Serper),
            ],
            default_provider_id: Some("b".to_string()),
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        });

        let order = registry.provider_order();

        assert_eq!(registry.entries[order[0]].config.id, "b");
        assert_eq!(registry.entries[order[1]].config.id, "a");
    }

    #[test]
    fn runtime_unavailable_when_default_provider_has_missing_required_config() {
        let mut tavily = provider("default", SearchProviderType::Tavily);
        tavily.api_key = Some("   ".to_string());

        let reason = SearchRegistry::runtime_unavailable_reason(&SearchConfig {
            providers: vec![tavily],
            default_provider_id: Some("default".to_string()),
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        });

        assert!(reason.is_some_and(|message| message.contains("API Key")));
    }

    #[test]
    fn runtime_unavailable_when_default_provider_is_disabled() {
        let mut tavily = provider("default", SearchProviderType::Tavily);
        tavily.enabled = false;

        let reason = SearchRegistry::runtime_unavailable_reason(&SearchConfig {
            providers: vec![tavily],
            default_provider_id: Some("default".to_string()),
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        });

        assert!(reason.is_some_and(|message| message.contains("未启用")));
    }

    #[test]
    fn runtime_available_when_usable_provider_exists_without_default() {
        let tavily = provider("fallback", SearchProviderType::Tavily);

        let reason = SearchRegistry::runtime_unavailable_reason(&SearchConfig {
            providers: vec![tavily],
            default_provider_id: None,
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        });

        assert!(reason.is_none());
    }

    #[test]
    fn runtime_available_when_default_provider_is_invalid_but_other_provider_is_usable() {
        let mut broken = provider("broken", SearchProviderType::Tavily);
        broken.api_key = Some(" ".to_string());
        let usable = provider("usable", SearchProviderType::Serper);

        let reason = SearchRegistry::runtime_unavailable_reason(&SearchConfig {
            providers: vec![broken, usable],
            default_provider_id: Some("broken".to_string()),
            max_results: 5,
            timeout_secs: 6,
            enable_fallback: true,
        });

        assert!(reason.is_none());
    }
}
