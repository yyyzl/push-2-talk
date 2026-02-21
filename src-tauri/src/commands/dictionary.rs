use tauri::{AppHandle, Emitter, Manager};

use crate::config::CONFIG_LOCK;
use crate::dictionary_utils::{entries_to_words, remove_entries, upsert_entry};
use crate::{
    emit_config_updated, find_monitor_at_cursor, load_persisted_config,
    mutate_persisted_config_with_result, AppState,
};

/// 添加学习到的词汇到词典
#[tauri::command]
pub async fn add_learned_word(
    app_handle: AppHandle,
    word: String,
    source: String,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    if source.eq_ignore_ascii_case("auto") {
        tracing::info!("macOS 上忽略自动学习词条: {}", word);
        return Ok(());
    }

    tracing::info!("添加学习词汇: {} (来源: {})", word, source);
    let (updated_config, words) = mutate_persisted_config_with_result(|config| {
        // 添加词条（source: "manual" 或 "auto"）
        upsert_entry(&mut config.dictionary, &word, &source);
        Ok(entries_to_words(&config.dictionary))
    })?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    *state.dictionary.lock().unwrap() = words.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(words.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(words.clone());
    }

    // 发送事件通知前端刷新配置和词典
    emit_config_updated(&app_handle, &updated_config);
    app_handle.emit("dictionary_updated", ()).ok();

    tracing::info!("词汇 '{}' 已添加到词典", word);
    Ok(())
}

/// 获取所有词典条目
#[tauri::command]
pub async fn get_dictionary_entries() -> Result<Vec<String>, String> {
    tracing::info!("获取词典条目...");

    let _guard = CONFIG_LOCK
        .lock()
        .map_err(|e| format!("获取配置锁失败: {}", e))?;
    let config = load_persisted_config()?;

    let entries = config.dictionary;
    #[cfg(not(target_os = "windows"))]
    let entries: Vec<String> = entries
        .into_iter()
        .filter(|entry| !entry.ends_with("|auto"))
        .collect();

    tracing::info!("返回 {} 个词典条目", entries.len());
    Ok(entries)
}

/// 删除指定词汇的词典条目（按 word 匹配）
#[tauri::command]
pub async fn delete_dictionary_entries(
    app_handle: AppHandle,
    words: Vec<String>,
) -> Result<(), String> {
    tracing::info!("删除词典条目: {:?}", words);
    let (updated_config, dict_words) = mutate_persisted_config_with_result(|config| {
        // 删除指定词汇（按 word 匹配，不区分来源）
        remove_entries(&mut config.dictionary, &words);
        Ok(entries_to_words(&config.dictionary))
    })?;

    // 热更新运行时词库
    let state = app_handle.state::<AppState>();
    *state.dictionary.lock().unwrap() = dict_words.clone();

    // 更新 ASR 客户端词库
    if let Some(ref mut client) = *state.qwen_client.lock().unwrap() {
        client.update_dictionary(dict_words.clone());
    }
    if let Some(ref mut client) = *state.doubao_client.lock().unwrap() {
        client.update_dictionary(dict_words.clone());
    }

    // 发送事件通知前端刷新配置和词典
    emit_config_updated(&app_handle, &updated_config);
    app_handle.emit("dictionary_updated", ()).ok();

    tracing::info!("词典条目删除完成");
    Ok(())
}

/// 忽略学习建议（暂不实现黑名单，仅关闭通知）
#[tauri::command]
pub async fn dismiss_learning_suggestion(id: String) -> Result<(), String> {
    tracing::debug!("忽略学习建议: {}", id);
    // 当前版本仅关闭通知，不实现黑名单机制
    // 未来可在此添加：将 id 对应的词汇加入黑名单，避免重复建议
    Ok(())
}

/// 显示通知窗口并定位到鼠标所在屏幕的悬浮窗上方
#[tauri::command]
pub async fn show_notification_window(app_handle: AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_handle;
        return Ok(());
    }

    if let Some(notification) = app_handle.get_webview_window("notification") {
        // 使用 overlay 或 main 窗口获取显示器列表（这些窗口已正确初始化）
        // notification 窗口在首次显示前可能没有正确初始化
        let reference_window = app_handle
            .get_webview_window("overlay")
            .or_else(|| app_handle.get_webview_window("main"));

        if let Some(ref_win) = reference_window {
            if let Some(monitor) = find_monitor_at_cursor(&ref_win) {
                let monitor_pos = monitor.position();
                let screen_size = monitor.size();
                let scale_factor = monitor.scale_factor();

                // 通知窗口尺寸（tauri.conf.json 中是逻辑像素，需转换为物理像素）
                let window_width = (360.0 * scale_factor) as i32;
                let window_height = (600.0 * scale_factor) as i32;

                // 悬浮窗底部边距 100px + 悬浮窗高度 80px + 间隔 80px = 260px（逻辑像素）
                // 通知窗口底部距离屏幕底部的距离（物理像素）
                let bottom_offset = (260.0 * scale_factor) as i32;

                // 水平居中
                let x = monitor_pos.x + (screen_size.width as i32 - window_width) / 2;
                // 垂直方向：在悬浮窗上方约 150px
                let y = monitor_pos.y + screen_size.height as i32 - window_height - bottom_offset;

                // 确保不超出屏幕顶部（至少留 50 逻辑像素）
                let top_margin = (50.0 * scale_factor) as i32;
                let y = y.max(monitor_pos.y + top_margin);

                notification
                    .set_position(tauri::PhysicalPosition::new(x, y))
                    .map_err(|e| format!("设置窗口位置失败: {}", e))?;
            }
        }

        // 避免抢占焦点：学习观察依赖前台窗口 hwnd，一旦通知窗口 set_focus 会导致学习误判“失焦”。
        // 通知窗口只需可见即可。
        notification
            .show()
            .map_err(|e| format!("显示窗口失败: {}", e))?;

        Ok(())
    } else {
        Err("通知窗口不存在".to_string())
    }
}

