export type DesktopOs = "windows" | "macos";
export type PermissionState = "granted" | "not_determined" | "denied" | "restricted";
export type PermissionKind = "microphone" | "accessibility" | "input_monitoring";
export interface PlatformStatus {
  os: DesktopOs;
  microphone: PermissionState;
  accessibility: PermissionState;
  input_monitoring: PermissionState;
  other_app_mute: boolean;
  text_observation: boolean;
}
export function permissionRequirements(status: PlatformStatus): PermissionKind[] {
  return (["microphone", "accessibility", "input_monitoring"] as PermissionKind[])
    .filter((permission) => status[permission] !== "granted");
}
export function platformKeyLabels(os: DesktopOs) {
  if (os === "macos") return { meta_left: "Cmd(左)", meta_right: "Cmd(右)", alt_left: "Option(左)", alt_right: "Option(右)" };
  return { meta_left: "Win(左)", meta_right: "Win(右)", alt_left: "Alt(左)", alt_right: "Alt(右)" };
}
// Cosmetic labels only; permissions/capabilities always come from the Rust backend.
export const desktopOs: DesktopOs = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "macos" : "windows";
