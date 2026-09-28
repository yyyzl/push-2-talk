#[path = "../src/platform/window_chrome.rs"]
mod window_chrome;

#[test]
fn native_chrome_preserves_window_identity_and_auxiliary_windows() {
    let mut config: tauri::utils::config::Config =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let before = serde_json::to_value(&config).unwrap();
    window_chrome::configure(&mut config);
    let after = serde_json::to_value(&config).unwrap();

    #[cfg(target_os = "windows")]
    assert_eq!(
        before, after,
        "Windows must keep its native window configuration"
    );

    #[cfg(target_os = "macos")]
    {
        let main = &config.app.windows[0];
        assert_eq!(main.label, "main");
        assert_eq!(
            main.title, "PushToTalk",
            "Keep a useful accessibility/window-menu name"
        );
        assert!(main.hidden_title);
        assert_eq!(after["app"]["windows"][0]["titleBarStyle"], "Overlay");
        assert!(main.decorations && main.resizable && main.maximizable && main.minimizable);
        assert_eq!(main.visible, false, "Do not break start-minimized behavior");
        assert_eq!(
            after["app"]["windows"][0]["width"],
            before["app"]["windows"][0]["width"]
        );
        assert_eq!(
            after["app"]["windows"][0]["minWidth"],
            before["app"]["windows"][0]["minWidth"]
        );
        assert_eq!(
            after["app"]["windows"].as_array().unwrap()[1..],
            before["app"]["windows"].as_array().unwrap()[1..]
        );
    }
}
