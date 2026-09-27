//! Platform-specific tray presentation; menus and actions stay in shared startup.
use tauri::{tray::TrayIconBuilder, Runtime};

pub fn builder<R: Runtime>() -> TrayIconBuilder<R> {
    let builder = TrayIconBuilder::new();
    #[cfg(target_os = "macos")]
    {
        // tray-icon renders this 44×36px image at 22×18pt. AppKit owns the light/dark
        // and selected-state tint; only the transparent 05A silhouette matters.
        builder
            .icon(tauri::include_image!("icons/tray-template@2x.png"))
            .icon_as_template(true)
    }
    #[cfg(target_os = "windows")]
    {
        builder.icon(tauri::include_image!("icons/tray-windows.png"))
    }
}
