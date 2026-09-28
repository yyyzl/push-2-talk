//! Characterization tests for the deployed v1.6.1 format, before moving storage.
use super::*;
use serde_json::{json, Value};

pub(super) fn release_fixture() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/config/v1.6.1.json")).unwrap()
}

fn assert_saved_fields(actual: &Value, expected: &Value, path: &str) {
    if path.ends_with(".dictionary") {
        let actual = actual.as_array().unwrap();
        let expected = expected.as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            let saved: Vec<_> = expected.as_str().unwrap().split('|').collect();
            let loaded: Vec<_> = actual.as_str().unwrap().split('|').collect();
            // A missing category may be inferred; words, source and explicit categories stay.
            assert_eq!(&loaded[..saved.len()], &saved, "changed dictionary entry");
        }
        return;
    }
    match expected {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert_saved_fields(&actual[key], value, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "changed deployed setting {path}"),
    }
}

#[test]
fn deployed_release_load_edit_save_restart_preserves_all_other_settings() {
    for provider in ["qwen", "doubao", "doubao_ime", "siliconflow"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut expected = release_fixture();
        expected["asr_config"]["selection"]["active_provider"] = json!(provider);
        std::fs::write(&path, serde_json::to_vec(&expected).unwrap()).unwrap();
        let source = std::fs::read(&path).unwrap();
        let (mut config, _) = AppConfig::load_from_path(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), source, "read must not write");
        assert_saved_fields(&serde_json::to_value(&config).unwrap(), &expected, "config");
        config.dual_hotkey_config.validate().unwrap();
        config
            .asr_config
            .validate_models(config.use_realtime_asr)
            .unwrap();
        assert_eq!(
            config.llm_config.resolve_polishing().model,
            "saved-polishing"
        );
        assert!(!config.assistant_config.enable_web_search);
        assert!(!config.tnl_config.enable_context_hotwords);
        assert!(!config.tnl_config.enable_personalization_exact_text_pass);
        for theme in ["light", "dark"] {
            config.theme = theme.into();
            expected["theme"] = json!(theme);
            config.save_to_path(&path).unwrap();
            let (restarted, migrated) = AppConfig::load_from_path(&path).unwrap();
            assert!(
                !migrated,
                "saved current config must load without another migration"
            );
            assert_saved_fields(
                &serde_json::to_value(&restarted).unwrap(),
                &expected,
                "restart",
            );
            config = restarted;
        }
    }
}

#[test]
fn failed_deployed_config_read_preserves_the_only_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let bytes = b"{\"dashscope_api_key\":\"fixture-v161-qwen\", broken";
    std::fs::write(&path, bytes).unwrap();
    assert!(AppConfig::load_from_path(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
