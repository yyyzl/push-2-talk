//! Partial edits are applied to the latest persisted document, never a UI snapshot.
use super::AppConfig;
use serde_json::Value;

pub(crate) fn apply_patch(config: &mut AppConfig, patch: &Value) -> Result<(), String> {
    let fields = patch.as_object().ok_or("配置修改必须是对象")?;
    // Dictionary ownership stays in the SQLite service. Legacy credential aliases are derived.
    for field in [
        "dictionary",
        "dashscope_api_key",
        "siliconflow_api_key",
        "hotkey_config",
        "transcription_mode",
    ] {
        if fields.contains_key(field) {
            return Err(format!("字段 {field} 不支持直接修改"));
        }
    }
    let mut document = serde_json::to_value(&*config).map_err(|e| e.to_string())?;
    merge_known_fields(&mut document, patch, "config")?;
    let mut next: AppConfig =
        serde_json::from_value(document).map_err(|e| format!("配置格式错误: {e}"))?;
    if fields.contains_key("theme") && !matches!(next.theme.as_str(), "light" | "dark") {
        return Err("主题必须为 light 或 dark".into());
    }
    if fields.contains_key("close_action")
        && !matches!(
            next.close_action.as_deref(),
            None | Some("close" | "minimize")
        )
    {
        return Err("关闭行为必须为 close、minimize 或 null".into());
    }
    if fields.contains_key("dual_hotkey_config") {
        next.dual_hotkey_config
            .validate()
            .map_err(|e| e.to_string())?;
    }
    if fields.contains_key("asr_config") || fields.contains_key("use_realtime_asr") {
        next.asr_config
            .validate_models(next.use_realtime_asr)
            .map_err(|e| e.to_string())?;
    }
    next.dashscope_api_key = next.asr_config.credentials.qwen_api_key.clone();
    next.siliconflow_api_key = next.asr_config.credentials.sensevoice_api_key.clone();
    *config = next;
    Ok(())
}

fn merge_known_fields(target: &mut Value, patch: &Value, path: &str) -> Result<(), String> {
    if let (Value::Object(target), Value::Object(patch)) = (&mut *target, patch) {
        for (key, value) in patch {
            let child_path = format!("{path}.{key}");
            let current = target
                .get_mut(key)
                .ok_or_else(|| format!("未知配置字段: {child_path}"))?;
            merge_known_fields(current, value, &child_path)?;
        }
    } else {
        *target = patch.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> AppConfig {
        serde_json::from_str(include_str!("../../../tests/fixtures/config/v1.6.1.json")).unwrap()
    }

    #[test]
    fn nested_edits_preserve_credentials_models_and_other_domains() {
        let mut config = fixture();
        let before = serde_json::to_value(&config).unwrap();
        apply_patch(
            &mut config,
            &json!({"asr_config":{"language_mode":"auto"}, "theme":"light"}),
        )
        .unwrap();
        let after = serde_json::to_value(&config).unwrap();
        assert_eq!(
            after["asr_config"]["credentials"],
            before["asr_config"]["credentials"]
        );
        assert_eq!(
            after["asr_config"]["qwen_profile"],
            before["asr_config"]["qwen_profile"]
        );
        assert_eq!(after["llm_config"], before["llm_config"]);
        assert_eq!(after["dictionary"], before["dictionary"]);
        assert_eq!(config.theme, "light");
    }

    #[test]
    fn explicit_null_clears_close_choice_but_omission_preserves_it() {
        let mut config = fixture();
        apply_patch(&mut config, &json!({"theme":"light"})).unwrap();
        assert_eq!(config.close_action.as_deref(), Some("minimize"));
        apply_patch(&mut config, &json!({"close_action":null})).unwrap();
        assert!(config.close_action.is_none());
    }

    #[test]
    fn invalid_and_unknown_edits_fail_without_partial_changes() {
        for patch in [
            json!({"theme":"typo"}),
            json!({"theme":"light", "unknown":true}),
            json!({"asr_config":{"credentials":{"qwen_api_kye":"typo"}}}),
            json!({"use_realtime_asr":"false"}),
            json!({"dictionary":[]}),
            json!({"close_action":"typo"}),
            json!({"dual_hotkey_config":{"dictation":{"keys":[]}}}),
        ] {
            let mut config = fixture();
            let before = serde_json::to_value(&config).unwrap();
            assert!(
                apply_patch(&mut config, &patch).is_err(),
                "accepted {patch}"
            );
            assert_eq!(serde_json::to_value(config).unwrap(), before);
        }
    }

    #[test]
    fn editing_credential_updates_legacy_alias_without_clearing_other_keys() {
        let mut config = fixture();
        apply_patch(
            &mut config,
            &json!({"asr_config":{"credentials":{"qwen_api_key":"changed"}}}),
        )
        .unwrap();
        assert_eq!(config.dashscope_api_key, "changed");
        assert_eq!(
            config.asr_config.credentials.sensevoice_api_key,
            "fixture-v161-sensevoice"
        );
    }
}
