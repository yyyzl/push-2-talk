import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("HC: lib 应接入 platform 抽象并隔离 win32_input 模块声明", async () => {
  const source = await readSource("src-tauri/src/lib.rs");
  const lifecycle = await readSource("src-tauri/src/commands/lifecycle.rs");

  assert.match(source, /mod platform;/);
  assert.match(source, /#\[cfg\(target_os = "windows"\)\]\s*mod win32_input;/);
  assert.match(source, /target_window:\s*Arc<Mutex<Option<WindowId>>>/);
  assert.match(lifecycle, /crate::platform::input::get_foreground_window\(\)/);
});

test("HC: pipeline focus/clipboard/text_inserter 应改用 platform::input", async () => {
  const focus = await readSource("src-tauri/src/pipeline/focus.rs");
  const clipboard = await readSource("src-tauri/src/clipboard_manager.rs");
  const inserter = await readSource("src-tauri/src/text_inserter.rs");

  assert.match(focus, /use crate::platform::input;/);
  assert.match(clipboard, /use crate::platform::input;/);
  assert.match(inserter, /use crate::platform::input;/);
});

test("FC: 应新增 usePlatform 并在 constants/页面中使用", async () => {
  const platformHook = await readSource("src/hooks/usePlatform.ts");
  const constants = await readSource("src/constants/index.ts");
  const llmPage = await readSource("src/pages/LlmPage.tsx");
  const preferences = await readSource("src/pages/PreferencesPage.tsx");

  assert.match(platformHook, /import \{ platform \} from "@tauri-apps\/plugin-os";/);
  assert.match(platformHook, /export const features = \{/);
  assert.match(constants, /getKeyDisplayNames\(/);
  assert.match(llmPage, /modifierKey/);
  assert.match(llmPage, /metaKeyName/);
  assert.match(preferences, /features\.muteOtherApps/);
  assert.match(preferences, /features\.autoLearning/);
});

test("FC/BC: tauri-plugin-os 与 capability 权限应接入", async () => {
  const cargo = await readSource("src-tauri/Cargo.toml");
  const pkg = await readSource("package.json");
  const capability = await readSource("src-tauri/capabilities/default.json");
  const lib = await readSource("src-tauri/src/lib.rs");

  assert.match(cargo, /tauri-plugin-os\s*=\s*"2"/);
  assert.match(pkg, /"@tauri-apps\/plugin-os"/);
  assert.match(capability, /"os:default"/);
  assert.match(lib, /\.plugin\(tauri_plugin_os::init\(\)\)/);
});

test("BC: tauri.conf 与 release workflow 应具备 macOS 与密钥安全配置", async () => {
  const tauriConf = await readSource("src-tauri/tauri.conf.json");
  const release = await readSource(".github/workflows/release.yml");

  assert.match(tauriConf, /"targets":\s*\[\s*"nsis"\s*,\s*"dmg"\s*\]/);
  assert.match(tauriConf, /"macOS":\s*\{/);
  assert.match(tauriConf, /"icons\/icon\.icns"/);
  assert.match(release, /TAURI_SIGNING_PRIVATE_KEY:\s*\$\{\{\s*secrets\.TAURI_SIGNING_PRIVATE_KEY\s*\}\}/);
  assert.match(release, /macos-latest/);
});

test("SC-7: 应提供 check_permissions 命令并在 App 启动接入权限引导弹窗", async () => {
  const systemCommands = await readSource("src-tauri/src/commands/system.rs");
  const lib = await readSource("src-tauri/src/lib.rs");
  const app = await readSource("src/App.tsx");

  assert.match(systemCommands, /async fn check_permissions\(\) -> PermissionStatus/);
  assert.match(systemCommands, /input_monitoring:\s*check_input_monitoring\(\)/);
  assert.match(systemCommands, /accessibility:\s*check_accessibility\(\)/);
  assert.match(lib, /commands::system::check_permissions/);
  assert.match(app, /invoke<PermissionStatus>\("check_permissions"\)/);
  assert.match(app, /<PermissionGuideModal/);
});

test("SC-10: macOS 下 add_learned_word 对 auto 来源应 no-op，词典查询需过滤 auto", async () => {
  const dictionaryCommands = await readSource("src-tauri/src/commands/dictionary.rs");
  const lib = await readSource("src-tauri/src/lib.rs");

  assert.match(dictionaryCommands, /#\[cfg\(not\(target_os = "windows"\)\)\]\s*if source\.eq_ignore_ascii_case\("auto"\)\s*\{/);
  assert.match(dictionaryCommands, /macOS 上忽略自动学习词条/);
  assert.match(dictionaryCommands, /filter\(\|entry\| !entry\.ends_with\("\|auto"\)\)/);
  assert.match(dictionaryCommands, /async fn show_notification_window/);
  assert.match(dictionaryCommands, /let _ = app_handle;\s*return Ok\(\(\)\);/);
  assert.match(lib, /commands::dictionary::add_learned_word/);
  assert.match(lib, /commands::dictionary::show_notification_window/);
});

test("SC-2: macOS input 不应是纯 Ok stub，需包含原生键盘事件实现", async () => {
  const input = await readSource("src-tauri/src/platform/macos/input.rs");

  assert.match(input, /CGEvent/);
  assert.doesNotMatch(
    input,
    /pub fn send_copy\(\) -> Result<\(\)> \{\s*Ok\(\(\)\)\s*\}/,
  );
  assert.doesNotMatch(
    input,
    /pub fn send_paste\(\) -> Result<\(\)> \{\s*Ok\(\(\)\)\s*\}/,
  );
});

test("SC-4: macOS cursor 不应返回固定坐标 (0,0)", async () => {
  const cursor = await readSource("src-tauri/src/platform/macos/cursor.rs");
  const lib = await readSource("src-tauri/src/lib.rs");

  assert.doesNotMatch(cursor, /Some\(\(0,\s*0\)\)/);
  assert.match(lib, /monitor_from_point\(/);
});

test("SC-8: 后端默认热键应按平台分支，macOS 默认 Option+Cmd", async () => {
  const config = await readSource("src-tauri/src/config.rs");

  assert.match(config, /#\[cfg\(target_os = "macos"\)\]\s*fn default_dictation_keys/);
  assert.match(
    config,
    /#\[cfg\(target_os = "macos"\)\][\s\S]*HotkeyKey::AltLeft[\s\S]*HotkeyKey::MetaLeft/,
  );
});

test("SC-7: 输入监控权限检测不应在 macOS 下恒返回 true", async () => {
  const tray = await readSource("src-tauri/src/tray.rs");

  assert.doesNotMatch(
    tray,
    /#\[cfg\(target_os = "macos"\)\][\s\S]*pub\(crate\) fn check_input_monitoring\(\) -> bool \{\s*\/\/[^\n]*\n\s*true\s*\}/,
  );
});

test("SC-8: 热键页面应提示 macOS F2 需要标准功能键设置", async () => {
  const hotkeys = await readSource("src/pages/HotkeysPage.tsx");

  assert.match(hotkeys, /(标准功能键|Fn\+F2)/);
});

test("FC: usePlatform 在 plugin-os 不可用时应回退到浏览器 UA 探测", async () => {
  const platformHook = await readSource("src/hooks/usePlatform.ts");

  assert.match(platformHook, /navigator\.platform/);
  assert.match(platformHook, /catch\s*\{[\s\S]*detectFromNavigator\(\)/);
});

test("SC: hide_to_tray 返回文案应按平台分支", async () => {
  const windowCommands = await readSource("src-tauri/src/commands/window.rs");

  assert.match(
    windowCommands,
    /#\[cfg\(target_os = "macos"\)\][\s\S]*"已隐藏到菜单栏"/,
  );
  assert.match(
    windowCommands,
    /#\[cfg\(not\(target_os = "macos"\)\)\][\s\S]*"已最小化到托盘"/,
  );
});

test("SC: macOS audio_mute stub 应维护 enabled 状态", async () => {
  const audioMute = await readSource("src-tauri/src/platform/macos/audio_mute.rs");

  assert.match(audioMute, /AtomicBool/);
  assert.match(audioMute, /set_enabled\(&self,\s*enabled:\s*bool\)[\s\S]*store\(enabled/);
  assert.match(audioMute, /is_enabled\(&self\)\s*->\s*bool[\s\S]*load\(Ordering::Relaxed\)/);
});
