use super::*;
use serde_json::json;

fn fixture() -> serde_json::Value {
    json!({
        "dashscope_api_key": "fixture-root-qwen",
        "siliconflow_api_key": "fixture-root-sensevoice",
        "asr_config": {
            "credentials": {"qwen_api_key": "fixture-nested-qwen", "sensevoice_api_key": "fixture-nested-sensevoice", "doubao_app_id": "fixture-app", "doubao_access_token": "fixture-token", "doubao_ime_device_id": "fixture-device", "doubao_ime_token": "fixture-ime-token", "doubao_ime_cdid": "fixture-cdid"},
            "selection": {"active_provider": "qwen", "enable_fallback": true, "fallback_provider": "siliconflow"},
            "language_mode": "zh"
        },
        "use_realtime_asr": false,
        "enable_llm_post_process": true,
        "enable_dictionary_enhancement": false,
        "llm_config": {"shared": {"providers": [{"id":"local", "name":"Local", "endpoint":"https://example.invalid/v1", "api_key":"fixture-llm", "default_model":"saved-model"}], "default_provider_id":"local"}, "presets": [{"id":"custom", "name":"Custom", "system_prompt":"Keep my prompt"}], "active_preset_id":"custom"},
        "assistant_config": {"qa_system_prompt":"Keep my question prompt", "text_processing_system_prompt":"Keep my edit prompt"},
        "learning_config": {"enabled":true, "observation_duration_secs":30},
        "tnl_config": {"enabled":false},
        "dual_hotkey_config": {"dictation":{"keys":["f8"]}, "assistant":{"keys":["f9"]}},
        "dictionary": ["Original Term|manual|phrase"],
        "builtin_dictionary_domains": ["programming"],
        "close_action":"minimize", "enable_mute_other_apps":true, "theme":"dark"
    })
}

fn write_fixture(path: &Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn main_config_upgrade_preserves_provider_credentials_modes_and_custom_settings() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    let original = fixture();
    write_fixture(&path, &original);
    let (config, _) = AppConfig::load_from_path(&path).unwrap();
    let value = serde_json::to_value(&config).unwrap();
    for field in ["credentials", "selection", "language_mode"] {
        assert_eq!(value["asr_config"][field], original["asr_config"][field]);
    }
    for field in [
        "dashscope_api_key",
        "siliconflow_api_key",
        "use_realtime_asr",
        "enable_llm_post_process",
        "enable_dictionary_enhancement",
        "theme",
        "close_action",
        "enable_mute_other_apps",
        "dictionary",
        "builtin_dictionary_domains",
    ] {
        assert_eq!(value[field], original[field], "changed {field}");
    }
    assert_eq!(config.asr_config.qwen_profile, QwenAsrProfile::Qwen3Legacy);
    assert!(!config.tnl_config.enabled);
    assert_eq!(
        config.tnl_config.disfluency_mode,
        crate::tnl::DisfluencyMode::Off
    );
    assert!(!config.assistant_config.enable_web_search);
    assert_eq!(config.llm_config.shared.providers[0].api_key, "fixture-llm");
    assert_eq!(config.llm_config.presets[0].system_prompt, "Keep my prompt");
    assert_eq!(
        config.dual_hotkey_config.dictation.keys,
        vec![HotkeyKey::F8]
    );
    assert_eq!(
        config.assistant_config.text_processing_system_prompt,
        "Keep my edit prompt"
    );
    config.save_to_path(&path).unwrap();
    let (reloaded, migrated) = AppConfig::load_from_path(&path).unwrap();
    assert!(!migrated, "migration must be idempotent");
    assert_eq!(serde_json::to_value(&reloaded).unwrap(), value);
}

#[test]
fn all_main_asr_providers_load_without_changing_selection_or_keys() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    for provider in ["qwen", "doubao", "doubao_ime", "siliconflow"] {
        let mut value = fixture();
        value["asr_config"]["selection"]["active_provider"] = json!(provider);
        write_fixture(&path, &value);
        let (config, _) = AppConfig::load_from_path(&path).unwrap();
        assert_eq!(
            serde_json::to_value(config.asr_config.selection).unwrap(),
            value["asr_config"]["selection"]
        );
        assert_eq!(
            serde_json::to_value(config.asr_config.credentials).unwrap(),
            value["asr_config"]["credentials"]
        );
    }
}

#[test]
fn invalid_new_field_cannot_reset_other_settings_or_overwrite_the_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    for (section, field, invalid) in [
        (
            "asr_config",
            "qwen_profile",
            json!("future_unsupported_profile"),
        ),
        ("tnl_config", "disfluency_mode", json!("invalid")),
    ] {
        let mut value = fixture();
        value[section][field] = invalid;
        write_fixture(&path, &value);
        let before = std::fs::read(&path).unwrap();
        assert!(
            AppConfig::load_from_path(&path).is_err(),
            "must not return reset defaults on parse failure"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn old_root_keys_restore_qwen_provider_and_original_model() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    write_fixture(
        &path,
        &json!({"dashscope_api_key":"fixture-old-key", "siliconflow_api_key":"fixture-backup-key"}),
    );
    let (config, _) = AppConfig::load_from_path(&path).unwrap();
    assert_eq!(
        config.asr_config.selection.active_provider,
        AsrProvider::Qwen
    );
    assert_eq!(
        config.asr_config.credentials.qwen_api_key,
        "fixture-old-key"
    );
    assert_eq!(config.asr_config.qwen_profile, QwenAsrProfile::Qwen3Legacy);
}

#[test]
fn missing_config_recovers_legacy_backup_instead_of_resetting_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    write_fixture(&path.with_extension("json.bak"), &fixture());
    let (config, migrated) = AppConfig::load_from_path(&path).unwrap();
    assert_eq!(
        config.asr_config.credentials.qwen_api_key,
        "fixture-nested-qwen"
    );
    assert!(migrated);
}

#[test]
fn config_save_failure_keeps_existing_directory_in_place() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("sentinel"), "preserve").unwrap();
    assert!(AppConfig::new().save_to_path(&path).is_err());
    assert!(path.is_dir());
    assert_eq!(
        std::fs::read_to_string(path.join("sentinel")).unwrap(),
        "preserve"
    );
}

#[test]
fn explicit_qwen_models_survive_load_and_unrelated_settings_save() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    for http_model in ["qwen3-asr-flash-2026-02-10", "future-model-kept-for-repair"] {
        let mut value = fixture();
        value["asr_config"]["qwen_models"] =
            json!({"http":http_model,"realtime":"qwen-audio-3.1-asr-flash-message"});
        write_fixture(&path, &value);
        let (mut config, _) = AppConfig::load_from_path(&path).unwrap();
        config.theme = "light".into();
        config.save_to_path(&path).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved["asr_config"]["qwen_models"],
            value["asr_config"]["qwen_models"]
        );
    }
}

#[test]
fn legacy_flat_llm_and_hotkey_migration_preserves_custom_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    write_fixture(
        &path,
        &json!({
            "dashscope_api_key":"fixture-old-key",
            "llm_config":{"endpoint":"https://example.invalid/v1","api_key":"fixture-flat-llm","model":"old-custom-model","presets":[{"id":"mine","name":"Mine","system_prompt":"Keep exact prompt"}],"active_preset_id":"mine"},
            "hotkey_config":{"keys":["f7"],"mode":"toggle"},
            "assistant_config":{"qa_system_prompt":"My QA","text_processing_system_prompt":"My edit"},
            "use_realtime_asr":false,"theme":"dark"
        }),
    );
    let (config, migrated) = AppConfig::load_from_path(&path).unwrap();
    assert!(migrated);
    let resolved = config.llm_config.resolve_polishing();
    assert_eq!(
        resolved.endpoint,
        "https://example.invalid/v1/chat/completions"
    );
    assert_eq!(resolved.api_key, "fixture-flat-llm");
    assert_eq!(resolved.model, "old-custom-model");
    assert_eq!(
        config.dual_hotkey_config.dictation.keys,
        vec![HotkeyKey::F7]
    );
    assert_eq!(config.dual_hotkey_config.dictation.mode, HotkeyMode::Toggle);
    assert_eq!(config.assistant_config.qa_system_prompt, "My QA");
    assert_eq!(
        config.llm_config.presets[0].system_prompt,
        "Keep exact prompt"
    );
    assert!(config.assistant_config.qa_llm.is_none());
    assert!(config.assistant_config.text_processing_llm.is_none());
    assert!(config.llm_config.presets[0].custom_body.is_none());
    assert!(config.llm_config.presets[0].reasoning.is_none());
    config.save_to_path(&path).unwrap();
    let (reloaded, migrated_again) = AppConfig::load_from_path(&path).unwrap();
    assert!(!migrated_again);
    assert_eq!(
        serde_json::to_value(config).unwrap(),
        serde_json::to_value(reloaded).unwrap()
    );
}

#[test]
fn malformed_primary_is_not_replaced_by_stale_backup_or_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    std::fs::write(&path, "{truncated").unwrap();
    write_fixture(&path.with_extension("json.bak"), &fixture());
    assert!(AppConfig::load_from_path(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{truncated");
}

#[test]
fn fresh_install_defaults_are_distinct_from_existing_main_config() {
    let temp = tempfile::tempdir().unwrap();
    let (config, migrated) = AppConfig::load_from_path(&temp.path().join("config.json")).unwrap();
    assert!(!migrated);
    assert_eq!(config.asr_config.qwen_profile, QwenAsrProfile::QwenAudio3_1);
    assert_eq!(
        config.tnl_config.disfluency_mode,
        crate::tnl::DisfluencyMode::Conservative
    );
}

#[test]
#[ignore = "requires explicitly supplied local config; copies to a temporary directory and never writes to the source"]
fn local_config_copy_upgrades_without_losing_asr_credentials_or_selection() {
    let source = std::env::var("PTT_COMPAT_CONFIG_PATH").expect("set PTT_COMPAT_CONFIG_PATH");
    let original_bytes = std::fs::read(&source).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&original_bytes).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    std::fs::write(&path, &original_bytes).unwrap();
    let (config, _) = AppConfig::load_from_path(&path).unwrap();
    let migrated = serde_json::to_value(&config).unwrap();
    for (key, value) in original["asr_config"]["credentials"].as_object().unwrap() {
        // Do not put secret values in assertion output.
        assert!(
            migrated["asr_config"]["credentials"][key] == *value,
            "credential changed: {key}"
        );
    }
    assert!(migrated["asr_config"]["selection"] == original["asr_config"]["selection"]);
    config.save_to_path(&path).unwrap();
    let (reloaded, migrated_again) = AppConfig::load_from_path(&path).unwrap();
    assert!(!migrated_again);
    assert!(serde_json::to_value(reloaded).unwrap() == migrated);
    assert!(std::fs::read(&source).unwrap() == original_bytes);
}

#[test]
fn provider_capabilities_prevent_legacy_flags_from_routing_keys_to_another_provider() {
    for requested in [true, false] {
        assert!(
            !AsrProvider::SiliconFlow.realtime_enabled(requested),
            "SenseVoice cannot be sent to the Qwen websocket with a SiliconFlow key"
        );
        assert!(AsrProvider::DoubaoIme.realtime_enabled(requested));
        assert_eq!(AsrProvider::Qwen.realtime_enabled(requested), requested);
        assert_eq!(AsrProvider::Doubao.realtime_enabled(requested), requested);
    }
}
