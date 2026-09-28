// Browser acceptance runs the real React tree with an in-memory desktop boundary.
// No microphone, user config, real credentials, vendor requests or native permissions.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { initialConfig, normalizeConfig } from "../../src/state/appConfig";
import releaseConfig from "../fixtures/config/v1.6.1.json";

let revision = 0;
let config = normalizeConfig({ ...initialConfig(), ...releaseConfig } as ReturnType<typeof initialConfig>);
const calls: { command: string; args: any }[] = [];
const listeners = new Map<number, { event: string; handler: number }>();
let nextListener = 0;
const browser = window as any;
const merge = (base: any, patch: any): any => {
  const next = { ...base };
  for (const [key, value] of Object.entries(patch)) {
    next[key] = value && typeof value === "object" && !Array.isArray(value) ? merge(base?.[key], value) : value;
  }
  return next;
};
const emit = (event: string, payload: unknown) => {
  for (const [id, listener] of listeners) {
    if (listener.event === event) browser.__TAURI_INTERNALS__.runCallback(listener.handler, { id, event, payload });
  }
};
browser.testDesktop = {
  calls, failWrites: false,
  emitConfig: (patch: unknown) => {
    config = merge(config, patch);
    emit("config_snapshot_updated", { revision: ++revision, config: structuredClone(config) });
  },
};
mockIPC(async (command, args: any) => {
  calls.push({ command, args: structuredClone(args) });
  if (command === "plugin:event|listen") {
    const id = ++nextListener; listeners.set(id, args); return id;
  }
  if (command === "plugin:event|unlisten") { listeners.delete(args.eventId); return; }
  if (command === "plugin:event|emit") { emit(args.event, args.payload); return; }
  if (command === "get_config_snapshot") return { revision, config: structuredClone(config) };
  if (command === "update_config") {
    if (browser.testDesktop.failWrites) throw new Error("模拟配置写入失败");
    config = merge(config, args.patch);
    const snapshot = { revision: ++revision, config: structuredClone(config) };
    emit("config_snapshot_updated", snapshot);
    return snapshot;
  }
  if (command === "get_dictionary_entries") return config.dictionary;
  if (command === "get_builtin_domains_raw") return "【开发测试】:[TypeScript,OpenAI]";
  if (command === "load_usage_stats") return { totalRecordingMs: 0, totalRecordingCount: 0, totalRecognizedChars: 0 };
  if (command === "get_platform_status") return { os: "macos", microphone: "granted", accessibility: "granted", input_monitoring: "granted", other_app_mute: false, text_observation: true };
  if (command === "get_autostart") return false;
  if (command === "plugin:app|version") return "1.6.3";
  if (command === "plugin:updater|check") return null;
  if (command === "start_app" && new URLSearchParams(location.search).has("idle")) throw new Error("测试服务保持停止");
  return null;
});
mockWindows("main");
