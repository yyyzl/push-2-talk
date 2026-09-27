use serde_json::{Map, Value};

use crate::config::{LlmReasoningConfig, ReasoningEffort};

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
