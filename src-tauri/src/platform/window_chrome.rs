//! Main-window presentation stays behind the native platform boundary.
pub fn configure(config: &mut tauri::utils::config::Config) {
    #[cfg(target_os = "macos")]
    if let Some(main) = config
        .app
        .windows
        .iter_mut()
        .find(|window| window.label == "main")
    {
        use tauri::utils::{config::LogicalPosition, TitleBarStyle};
        main.title_bar_style = TitleBarStyle::Overlay;
        main.hidden_title = true;
        // Native traffic lights, vertically centered in the 64pt web toolbar.
        main.traffic_light_position = Some(LogicalPosition { x: 20.0, y: 30.0 });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = config;
}
