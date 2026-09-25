import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PermissionKind, PlatformStatus } from "../utils/platform";

export function usePlatformStatus() {
  const [platform, setPlatform] = useState<PlatformStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [requesting, setRequesting] = useState<PermissionKind | null>(null);
  const refresh = useCallback(async () => {
    try {
      setPlatform(await invoke<PlatformStatus>("get_platform_status"));
      setError(null);
    } catch {
      setError("无法读取系统权限状态，请在桌面应用中重试。");
    }
  }, []);
  useEffect(() => {
    void refresh();
    const onFocus = () => { void refresh(); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);
  const request = async (permission: PermissionKind) => {
    setRequesting(permission);
    try {
      await invoke("request_platform_permission", { permission });
      await refresh();
    } catch {
      setError("无法打开授权设置，请前往系统设置 → 隐私与安全性。");
    } finally {
      setRequesting(null);
    }
  };
  return { platform, error, requesting, refresh, request };
}
