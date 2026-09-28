export type ConfigSyncWindowSource = "initial_load" | "external_config_updated";

export function getSyncWindowNoticeMessage(
  source: ConfigSyncWindowSource | null,
): string | null {
  if (source === "initial_load") {
    return "正在加载初始配置";
  }

  if (source === "external_config_updated") {
    return "正在同步外部配置";
  }

  return null;
}
