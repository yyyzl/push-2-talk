import { platform } from "@tauri-apps/plugin-os";

let currentPlatform = "windows";

const detectFromNavigator = (): string | null => {
  if (typeof navigator === "undefined") {
    return null;
  }
  const ua = `${navigator.platform ?? ""} ${navigator.userAgent ?? ""}`.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("win")) return "windows";
  if (ua.includes("linux")) return "linux";
  return null;
};

try {
  const detected = platform() as unknown;
  if (typeof detected === "string" && detected.trim()) {
    currentPlatform = detected;
  } else {
    const fallback = detectFromNavigator();
    if (fallback) {
      currentPlatform = fallback;
    }
  }
} catch {
  // 在非 Tauri 环境（如浏览器预览）回退到 UA 探测，避免误判成 windows
  const fallback = detectFromNavigator();
  if (fallback) {
    currentPlatform = fallback;
  }
}

export const isMacos = currentPlatform === "macos";
export const isWindows = currentPlatform === "windows";

export const metaKeyName = isMacos ? "Cmd" : "Win";
export const modifierKey = isMacos ? "Cmd" : "Ctrl";
export const altKeyName = isMacos ? "Option" : "Alt";

export const features = {
  muteOtherApps: isWindows,
  autoLearning: isWindows,
  notificationToast: isWindows,
};

export const macKeyNames: Record<string, string> = {
  meta_left: "Cmd(⌘)",
  meta_right: "Cmd(⌘)",
  alt_left: "Option(⌥)",
  alt_right: "Option(⌥)",
  control_left: "Control(⌃)",
  control_right: "Control(⌃)",
  backspace: "Delete",
  return: "Return",
};
