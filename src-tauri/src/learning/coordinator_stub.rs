use tauri::AppHandle;

use crate::config::LearningConfig;
use crate::platform::types::WindowId;

pub fn start_learning_observation(
    _app_handle: AppHandle,
    _asr_text: String,
    _target_window: WindowId,
    _config: LearningConfig,
) {
    // macOS 精简版：自动学习关闭，保留 no-op 以维持跨层调用契约
}
