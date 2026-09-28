use serde_json::{Map, Value};

use crate::config::{LlmReasoningConfig, ReasoningEffort};

// This is the supported subset of the current Chat Completions adapter, not a
// claim that every similarly named or future model has the same capabilities.
// Legacy request mappings below remain unchanged until the user edits a value.
pub fn selectable_efforts(model: &str) -> Vec<ReasoningEffort> {
    use ReasoningEffort::*;
    let model = model.to_ascii_lowercase();
    if model.starts_with("qwen3-max-20") && model.as_str() < "qwen3-max-2026-01-23" {
        return vec![Default];
    }
    if model_variant(
        &model,
        &[
            "qwen3-max",
            "qwen3-max-preview",
            "qwen3.5-plus",
            "qwen3.5-flash",
            "qwen3.6-plus",
            "qwen3.6-flash",
            "qwen3.6-max-preview",
            "qwen3.7-plus",
            "qwen3.7-flash",
            "qwen3.8-max",
            "qwen3.8-flash",
            "qwen3-235b-a22b",
            "qwen3-32b",
            "qwen3-30b-a3b",
            "qwen3-14b",
            "qwen3-8b",
        ],
    ) {
        vec![Default, None, Auto]
    } else if model_variant(
        &model,
        &["deepseek-v4", "deepseek-v4-pro", "deepseek-v4-flash"],
    ) {
        // medium maps to high at the provider; xhigh is our existing max mapping.
        vec![Default, None, Low, High, Xhigh]
    } else if model_variant(
        &model,
        &[
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5.1",
            "gpt-5.2",
            "gpt-5.4",
            "gpt-5.5",
            "o1",
            "o3",
            "o3-mini",
            "o4-mini",
        ],
    ) {
        // The existing adapter maps xhigh to high and does not implement none.
        vec![Default, Low, Medium, High]
    } else if model_variant(&model, &["gemini-2.5-flash", "gemini-2.5-flash-lite"]) {
        vec![Default, None]
    } else {
        vec![Default]
    }
}

fn model_variant(model: &str, names: &[&str]) -> bool {
    names.iter().any(|name| {
        model == *name
            || model
                .strip_prefix(&format!("{name}-"))
                .is_some_and(|suffix| {
                    suffix.len() == 10
                        && suffix.bytes().enumerate().all(|(i, ch)| {
                            if i == 4 || i == 7 {
                                ch == b'-'
                            } else {
                                ch.is_ascii_digit()
                            }
                        })
                })
    })
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningContext {
    Polishing {
        config: crate::config::LlmConfig,
    },
    Assistant {
        config: crate::config::AssistantConfig,
        shared: crate::config::SharedLlmConfig,
        text_processing: bool,
    },
}

#[derive(Debug, serde::Serialize)]
pub struct ReasoningOptions {
    model: String,
    efforts: Vec<ReasoningEffort>,
    legacy_hint: Option<String>,
}

#[tauri::command]
pub fn get_reasoning_options(
    context: ReasoningContext,
    current: Option<ReasoningEffort>,
) -> ReasoningOptions {
    let model = match context {
        ReasoningContext::Polishing { config } => config.resolve_polishing().model,
        ReasoningContext::Assistant {
            config,
            shared,
            text_processing,
        } => {
            if text_processing {
                config.resolve_text_processing_llm(&shared).model
            } else {
                config.resolve_qa_llm(&shared).model
            }
        }
    };
    let efforts = selectable_efforts(&model);
    let legacy_hint = current
        .filter(|effort| !efforts.contains(effort))
        .map(|effort| {
            let patch = reasoning_patch(&model, &effort);
            if patch.is_empty() {
                "这项旧设置未产生额外思考参数，已保留。可选择默认或当前可用选项。".to_string()
            } else if let Some(equivalent) = efforts
                .iter()
                .find(|candidate| reasoning_patch(&model, candidate) == patch)
            {
                let label = match equivalent {
                    ReasoningEffort::Auto => "开启",
                    ReasoningEffort::High => "高",
                    _ => "已有模式",
                };
                format!("这项旧设置实际按「{label}」执行，已保留；可改选对应选项。")
            } else {
                "当前适配未将这项旧设置列为可选项；原请求参数已保留，也可改选默认。".to_string()
            }
        });
    ReasoningOptions {
        model,
        efforts,
        legacy_hint,
    }
}

const BLOCKED_CUSTOM_BODY_KEYS: &[&str] = &[
    "model",
    "messages",
    "stream",
    "tools",
    "tool_choice",
    "authorization",
    "headers",
    "api_key",
    "endpoint",
    "url",
];

pub fn merge_request_options(
    body: &mut Value,
    model: &str,
    reasoning: Option<&LlmReasoningConfig>,
    custom_body: Option<&Value>,
) {
    if let Some(reasoning) = reasoning {
        merge_object(body, reasoning_patch(model, &reasoning.effort));
    }

    if let Some(custom_body) = custom_body {
        merge_object(body, filter_custom_body(custom_body));
    }
}

pub fn reasoning_patch(model: &str, effort: &ReasoningEffort) -> Map<String, Value> {
    let model = model.to_ascii_lowercase();
    let mut patch = Map::new();

    match effort {
        ReasoningEffort::Default => {}
        ReasoningEffort::None => {
            if is_deepseek_reasoning_model(&model) {
                patch.insert(
                    "thinking".to_string(),
                    serde_json::json!({ "type": "disabled" }),
                );
            } else if is_qwen_thinking_model(&model) {
                patch.insert("enable_thinking".to_string(), Value::Bool(false));
            } else if is_gemini_flash_model(&model) {
                patch.insert(
                    "extra_body".to_string(),
                    serde_json::json!({
                        "google": {
                            "thinking_config": {
                                "thinking_budget": 0
                            }
                        }
                    }),
                );
            }
        }
        ReasoningEffort::Auto => {
            if is_deepseek_reasoning_model(&model) {
                patch.insert(
                    "thinking".to_string(),
                    serde_json::json!({ "type": "enabled" }),
                );
            } else if is_qwen_thinking_model(&model) {
                patch.insert("enable_thinking".to_string(), Value::Bool(true));
            }
        }
        ReasoningEffort::Low | ReasoningEffort::Medium | ReasoningEffort::High => {
            let value = match effort {
                ReasoningEffort::Low => "low",
                ReasoningEffort::Medium => "medium",
                ReasoningEffort::High => "high",
                _ => unreachable!(),
            };

            if is_deepseek_reasoning_model(&model) {
                patch.insert(
                    "thinking".to_string(),
                    serde_json::json!({ "type": "enabled" }),
                );
                patch.insert(
                    "reasoning_effort".to_string(),
                    Value::String(value.to_string()),
                );
            } else if is_qwen_thinking_model(&model) {
                patch.insert("enable_thinking".to_string(), Value::Bool(true));
            } else if is_openai_reasoning_model(&model) {
                patch.insert(
                    "reasoning_effort".to_string(),
                    Value::String(value.to_string()),
                );
            }
        }
        ReasoningEffort::Xhigh => {
            if is_deepseek_reasoning_model(&model) {
                patch.insert(
                    "thinking".to_string(),
                    serde_json::json!({ "type": "enabled" }),
                );
                patch.insert(
                    "reasoning_effort".to_string(),
                    Value::String("max".to_string()),
                );
            } else if is_openai_reasoning_model(&model) {
                patch.insert(
                    "reasoning_effort".to_string(),
                    Value::String("high".to_string()),
                );
            }
        }
    }

    patch
}

pub fn filter_custom_body(custom_body: &Value) -> Map<String, Value> {
    let mut filtered = Map::new();
    let Some(object) = custom_body.as_object() else {
        return filtered;
    };

    for (key, value) in object {
        if BLOCKED_CUSTOM_BODY_KEYS
            .iter()
            .any(|blocked| key.eq_ignore_ascii_case(blocked))
        {
            tracing::warn!("忽略不允许覆盖的 LLM 自定义请求字段: {}", key);
            continue;
        }
        filtered.insert(key.clone(), value.clone());
    }

    filtered
}

fn merge_object(target: &mut Value, patch: Map<String, Value>) {
    let Some(target) = target.as_object_mut() else {
        return;
    };

    for (key, value) in patch {
        merge_value(target, key, value);
    }
}

fn merge_value(target: &mut Map<String, Value>, key: String, value: Value) {
    match (target.get_mut(&key), value) {
        (Some(Value::Object(existing)), Value::Object(incoming)) => {
            for (child_key, child_value) in incoming {
                merge_value(existing, child_key, child_value);
            }
        }
        (_, value) => {
            target.insert(key, value);
        }
    }
}

fn is_deepseek_reasoning_model(model: &str) -> bool {
    model.contains("deepseek") && (model.contains("v4") || model.contains("reasoner"))
}

fn is_qwen_thinking_model(model: &str) -> bool {
    model.contains("qwen") && (model.contains("3") || model.contains("thinking"))
}

fn is_gemini_flash_model(model: &str) -> bool {
    model.contains("gemini") && model.contains("flash")
}

fn is_openai_reasoning_model(model: &str) -> bool {
    model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
        || model.starts_with("gpt-5")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_only_offers_effective_distinct_choices() {
        use ReasoningEffort::*;
        assert_eq!(selectable_efforts("qwen3-max"), vec![Default, None, Auto]);
        assert_eq!(
            selectable_efforts("gpt-5"),
            vec![Default, Low, Medium, High]
        );
        assert_eq!(
            selectable_efforts("deepseek-v4-pro"),
            vec![Default, None, Low, High, Xhigh]
        );
        for model in [
            "unknown",
            "qwen3-asr-flash",
            "qwen3-235b-a22b-thinking-2507",
            "gpt-5-pro",
            "o1-preview",
            "gemini-3-flash",
        ] {
            assert_eq!(selectable_efforts(model), vec![Default], "{model}");
        }
    }

    #[test]
    fn selector_uses_effective_model_and_preserves_legacy_request_mapping() {
        let mut config = crate::config::LlmConfig::default();
        config.feature_override.use_shared = false;
        config.feature_override.model = Some("gpt-5".to_string());
        let options = get_reasoning_options(
            ReasoningContext::Polishing { config },
            Some(ReasoningEffort::Xhigh),
        );
        assert_eq!(options.model, "gpt-5");
        assert!(!options.efforts.contains(&ReasoningEffort::Xhigh));
        assert!(options.legacy_hint.unwrap().contains("高"));
        assert_eq!(
            reasoning_patch("gpt-5", &ReasoningEffort::Xhigh)["reasoning_effort"],
            "high"
        );

        let mut assistant = crate::config::AssistantConfig::default();
        assistant.llm.use_shared = false;
        assistant.llm.model = Some("qwen3-max".to_string());
        assistant.qa_llm = Some(crate::config::LlmFeatureConfig {
            reasoning: Some(LlmReasoningConfig {
                effort: ReasoningEffort::Low,
            }),
            ..Default::default()
        });
        assistant.text_processing_llm = Some(crate::config::LlmFeatureConfig {
            use_shared: false,
            model: Some("gpt-5".to_string()),
            ..Default::default()
        });
        let qa = get_reasoning_options(
            ReasoningContext::Assistant {
                config: assistant.clone(),
                shared: Default::default(),
                text_processing: false,
            },
            Some(ReasoningEffort::Low),
        );
        assert_eq!(qa.model, "qwen3-max");
        assert!(qa.legacy_hint.unwrap().contains("开启"));
        let text = get_reasoning_options(
            ReasoningContext::Assistant {
                config: assistant,
                shared: Default::default(),
                text_processing: true,
            },
            None,
        );
        assert_eq!(text.model, "gpt-5");
        assert!(text.efforts.contains(&ReasoningEffort::Medium));
    }

    #[test]
    fn selector_reports_noop_legacy_settings_without_losing_them() {
        let mut config = crate::config::LlmConfig::default();
        config.feature_override.use_shared = false;
        config.feature_override.model = Some("unknown-proxy-model".to_string());
        let options = get_reasoning_options(
            ReasoningContext::Polishing { config },
            Some(ReasoningEffort::High),
        );
        assert_eq!(options.efforts, vec![ReasoningEffort::Default]);
        assert!(options.legacy_hint.unwrap().contains("未产生"));
        for value in ["default", "none", "auto", "low", "medium", "high", "xhigh"] {
            let saved = serde_json::json!({ "effort": value });
            let parsed: LlmReasoningConfig = serde_json::from_value(saved.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), saved);
        }
    }

    #[test]
    fn selectable_efforts_produce_distinct_nonempty_payloads() {
        for model in [
            "qwen3-max",
            "gpt-5",
            "gpt-5.2",
            "deepseek-v4-pro",
            "gemini-2.5-flash",
        ] {
            let mut patches = Vec::new();
            for effort in selectable_efforts(model).into_iter().skip(1) {
                let patch = reasoning_patch(model, &effort);
                assert!(!patch.is_empty(), "{model}: {effort:?}");
                assert!(!patches.contains(&patch), "duplicate {model}: {effort:?}");
                patches.push(patch);
            }
        }
    }

    #[test]
    fn default_effort_returns_empty_patch() {
        assert!(reasoning_patch("gpt-4o-mini", &ReasoningEffort::Default).is_empty());
    }

    #[test]
    fn deepseek_none_disables_thinking() {
        let patch = reasoning_patch("deepseek-v4", &ReasoningEffort::None);
        assert_eq!(patch["thinking"]["type"], "disabled");
    }

    #[test]
    fn deepseek_xhigh_maps_to_max() {
        let patch = reasoning_patch("deepseek-v4", &ReasoningEffort::Xhigh);
        assert_eq!(patch["thinking"]["type"], "enabled");
        assert_eq!(patch["reasoning_effort"], "max");
    }

    #[test]
    fn qwen_none_disables_enable_thinking() {
        let patch = reasoning_patch("qwen3-max", &ReasoningEffort::None);
        assert_eq!(patch["enable_thinking"], false);
    }

    #[test]
    fn openai_reasoning_effort_uses_chat_completions_field() {
        let patch = reasoning_patch("gpt-5", &ReasoningEffort::High);
        assert_eq!(patch["reasoning_effort"], "high");
        assert!(!patch.contains_key("reasoningEffort"));
    }

    #[test]
    fn unknown_model_effort_is_noop() {
        assert!(reasoning_patch("glm-4-flash", &ReasoningEffort::High).is_empty());
        assert!(reasoning_patch("some-chat-model", &ReasoningEffort::Xhigh).is_empty());
    }

    #[test]
    fn custom_body_filters_system_fields() {
        let body = serde_json::json!({
            "model": "bad",
            "messages": [],
            "top_p": 0.8,
            "extra_body": { "foo": true }
        });

        let filtered = filter_custom_body(&body);
        assert!(!filtered.contains_key("model"));
        assert!(!filtered.contains_key("messages"));
        assert_eq!(filtered["top_p"], 0.8);
        assert_eq!(filtered["extra_body"]["foo"], true);
    }
}
