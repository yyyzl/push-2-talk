use tauri::{AppHandle, Manager};

use crate::{
    check_accessibility, check_input_monitoring, check_microphone_access, AppState,
    ConfigFieldPatch, PermissionStatus,
};
use crate::commands::config::patch_config_fields;
use crate::config;
use crate::openai_client::{ChatOptions, Message, OpenAiClient, OpenAiClientConfig};

/// 设置开机自启动
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<String, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(if enabled {
        "已启用开机自启"
    } else {
        "已禁用开机自启"
    }
    .to_string())
}

/// 获取开机自启动状态
#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        microphone: check_microphone_access(),
        input_monitoring: check_input_monitoring(),
        accessibility: check_accessibility(),
    }
}

/// 重置热键状态（用于手动修复状态卡死问题）
#[tauri::command]
pub async fn reset_hotkey_state(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    state.hotkey_service.reset_state();
    Ok("热键状态已重置".to_string())
}

/// 获取热键服务是否激活
#[tauri::command]
pub async fn get_hotkey_service_active(app_handle: AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<AppState>();
    Ok(state.hotkey_service.is_service_active())
}

/// 设置热键服务是否激活（用于录制快捷键时临时屏蔽）
#[tauri::command]
pub async fn set_hotkey_service_active(
    app_handle: AppHandle,
    active: bool,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    if active {
        state.hotkey_service.resume();
    } else {
        state.hotkey_service.deactivate();
    }
    Ok(())
}

#[tauri::command]
pub async fn set_learning_enabled(app: AppHandle, enabled: bool) -> Result<String, String> {
    patch_config_fields(
        app,
        ConfigFieldPatch {
            learning_enabled: Some(enabled),
            ..ConfigFieldPatch::default()
        },
    )
    .await?;
    tracing::info!("自动学习已{}", if enabled { "开启" } else { "关闭" });
    Ok("ok".to_string())
}

/// 获取热键调试信息
#[tauri::command]
pub async fn get_hotkey_debug_info(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    Ok(state.hotkey_service.get_debug_info())
}

/// 测试 LLM Provider 配置是否可用
///
/// 发送一个非常短的 Chat Completions 请求来验证：
/// - endpoint 是否可达
/// - api_key 是否有效
/// - model 是否可用
///
/// 备注：endpoint 可传 base URL 或 full URL；最终会被 normalize 为 `/chat/completions`。
#[tauri::command]
pub async fn test_llm_provider(
    endpoint: String,
    api_key: String,
    model: String,
) -> Result<String, String> {
    let resolved_endpoint = config::normalize_chat_completions_endpoint(&endpoint);

    if resolved_endpoint.trim().is_empty() {
        return Err("Endpoint 不能为空".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    if model.trim().is_empty() {
        return Err("Model 不能为空".to_string());
    }

    let client = OpenAiClient::new(OpenAiClientConfig::new(resolved_endpoint, api_key, model));
    let messages = vec![
        Message::system("You are a connectivity test. Reply with: OK"),
        Message::user("OK"),
    ];

    client
        .chat(
            &messages,
            ChatOptions {
                max_tokens: 4,
                temperature: 0.0,
            },
        )
        .await
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("测试请求失败: {e}"))
}
