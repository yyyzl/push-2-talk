use tauri::AppHandle;

use crate::config::{self, AppConfig, CONFIG_LOCK};
use crate::{
    emit_config_updated, lock_hotwords_or_recover, load_persisted_config,
    merge_asr_config_for_save, mutate_persisted_config, mutate_persisted_config_with_result,
    AppState, ConfigFieldPatch,
};
use crate::usage_stats::UsageStats;

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    enable_dictionary_enhancement: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    smart_command_config: Option<config::SmartCommandConfig>,
    close_action: Option<String>,
    asr_config: Option<config::AsrConfig>,
    hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
    learning_config: Option<config::LearningConfig>,
    enable_mute_other_apps: Option<bool>,
    dictionary: Option<Vec<String>>,
    builtin_dictionary_domains: Option<Vec<String>>,
    theme: Option<String>,
) -> Result<String, String> {
    let config = mutate_persisted_config_with_result(|existing| {
        tracing::info!("保存配置...");

        // 智能合并 llm_config：如果传入的 presets 为空，保留旧值
        let final_llm_config = match llm_config {
            Some(mut cfg) if cfg.presets.is_empty() && !existing.llm_config.presets.is_empty() => {
                tracing::warn!("检测到空 presets，保留旧配置");
                cfg.presets = existing.llm_config.presets.clone();
                cfg.active_preset_id = existing.llm_config.active_preset_id.clone();
                cfg
            }
            Some(cfg) => cfg,
            None => existing.llm_config.clone(),
        };

        // 智能合并 assistant_config：如果传入的配置无效，保留旧值
        let final_assistant_config = match assistant_config {
            Some(cfg)
                if !cfg.is_valid_with_shared(&final_llm_config.shared)
                    && existing
                        .assistant_config
                        .is_valid_with_shared(&final_llm_config.shared) =>
            {
                tracing::warn!("检测到无效 assistant_config，保留旧配置");
                existing.assistant_config.clone()
            }
            Some(cfg) => cfg,
            None => existing.assistant_config.clone(),
        };

        // 智能合并 dictionary：如果传入空数组，保留旧值
        let final_dictionary = match dictionary {
            Some(dict) if dict.is_empty() && !existing.dictionary.is_empty() => {
                tracing::warn!("检测到空 dictionary，保留旧配置");
                existing.dictionary.clone()
            }
            Some(dict) => {
                // 前端传入的格式：纯词汇 "word" 或带来源 "word|auto"
                // 直接使用传入的数组，不再合并（前端已经是完整的词典状态）
                dict
            }
            None => existing.dictionary.clone(),
        };

        // 智能合并 dual_hotkey_config：如果传入空 keys，保留旧值
        let final_dual_hotkey_config = match dual_hotkey_config {
            Some(cfg) if cfg.dictation.keys.is_empty() || cfg.assistant.keys.is_empty() => {
                tracing::warn!("检测到空快捷键配置，保留旧配置");
                existing.dual_hotkey_config.clone()
            }
            Some(cfg) => cfg,
            None => existing.dual_hotkey_config.clone(),
        };

        let final_asr_config = merge_asr_config_for_save(
            asr_config,
            &existing.asr_config,
            &api_key,
            &fallback_api_key,
        );

        *existing = AppConfig {
            dashscope_api_key: final_asr_config.credentials.qwen_api_key.clone(),
            siliconflow_api_key: final_asr_config.credentials.sensevoice_api_key.clone(),
            asr_config: final_asr_config,
            use_realtime_asr: use_realtime.unwrap_or(existing.use_realtime_asr),
            enable_llm_post_process: enable_post_process
                .unwrap_or(existing.enable_llm_post_process),
            enable_dictionary_enhancement: enable_dictionary_enhancement
                .unwrap_or(existing.enable_dictionary_enhancement),
            llm_config: final_llm_config,
            smart_command_config: smart_command_config
                .unwrap_or_else(|| existing.smart_command_config.clone()),
            assistant_config: final_assistant_config,
            learning_config: learning_config.unwrap_or_else(|| existing.learning_config.clone()),
            tnl_config: existing.tnl_config.clone(),
            close_action: close_action.or_else(|| existing.close_action.clone()),
            hotkey_config: hotkey_config.or_else(|| existing.hotkey_config.clone()),
            dual_hotkey_config: final_dual_hotkey_config,
            transcription_mode: existing.transcription_mode,
            enable_mute_other_apps: enable_mute_other_apps
                .unwrap_or(existing.enable_mute_other_apps),
            dictionary: final_dictionary,
            builtin_dictionary_domains: builtin_dictionary_domains
                .unwrap_or_else(|| existing.builtin_dictionary_domains.clone()),
            theme: theme.unwrap_or_else(|| existing.theme.clone()),
        };

        Ok(())
    })?
    .0;

    emit_config_updated(&app, &config);

    tracing::info!("[save_config] 配置已保存, theme={}", config.theme);

    Ok("配置已保存".to_string())
}

#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    tracing::info!("加载配置...");
    let _guard = CONFIG_LOCK
        .lock()
        .map_err(|e| format!("获取配置锁失败: {}", e))?;
    load_persisted_config()
}

#[tauri::command]
pub fn get_builtin_domains_raw(state: tauri::State<'_, AppState>) -> String {
    lock_hotwords_or_recover(&state.builtin_hotwords_raw).clone()
}

#[tauri::command]
pub async fn patch_config_fields(
    app: AppHandle,
    patch: ConfigFieldPatch,
) -> Result<String, String> {
    let updated_config = mutate_persisted_config(|config| {
        if let Some(enabled) = patch.learning_enabled {
            config.learning_config.enabled = enabled;
        }

        if let Some(theme) = patch.theme {
            let theme = theme.trim();
            if matches!(theme, "light" | "dark") {
                config.theme = theme.to_string();
            }
        }

        if let Some(enabled) = patch.enable_mute_other_apps {
            config.enable_mute_other_apps = enabled;
        }

        if let Some(close_action_patch) = patch.close_action {
            match close_action_patch {
                Some(action) => {
                    let action = action.trim();
                    if matches!(action, "close" | "minimize") {
                        config.close_action = Some(action.to_string());
                    }
                }
                None => {
                    config.close_action = None;
                }
            }
        }

        Ok(())
    })?;

    emit_config_updated(&app, &updated_config);

    Ok("配置字段已更新".to_string())
}

#[tauri::command]
pub async fn load_usage_stats() -> Result<UsageStats, String> {
    tracing::info!("加载使用统计数据...");
    UsageStats::load().map_err(|e| format!("加载统计数据失败: {}", e))
}

