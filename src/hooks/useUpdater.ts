import { useCallback, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import type { UpdateStatus } from "../types";

export type UpdaterInfo = { version: string; notes?: string };

export type UseUpdaterParams = {
  onToast: (message: string) => void;
  onError: (message: string) => void;
};

export type CheckForUpdatesOptions = {
  openModal?: boolean;
  silentOnNoUpdate?: boolean;
  silentOnError?: boolean;
};

export function useUpdater({ onToast, onError }: UseUpdaterParams) {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateInfo, setUpdateInfo] = useState<UpdaterInfo | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [showUpdateModal, setShowUpdateModal] = useState(false);

  const dismissUpdateModal = useCallback(() => {
    setShowUpdateModal(false);
    setUpdateStatus("idle");
  }, []);

  const checkForUpdates = useCallback(
    async (options: CheckForUpdatesOptions = {}) => {
      const { openModal = false, silentOnNoUpdate = false, silentOnError = false } = options;

      try {
        setUpdateStatus("checking");
        const update = await check();
        if (update) {
          setUpdateInfo({
            version: update.version,
            notes: update.body || undefined,
          });
          setUpdateStatus("available");
          if (openModal) setShowUpdateModal(true);
        } else {
          setUpdateStatus("idle");
          if (!silentOnNoUpdate) onToast("当前已是最新版本");
        }
      } catch (err) {
        console.error("检查更新失败:", err);
        setUpdateStatus("idle");
        if (silentOnError) return;

        const errorStr = String(err).toLowerCase();
        let errorMsg = "检查更新失败，请稍后重试";
        if (errorStr.includes("timeout") || errorStr.includes("timed out")) {
          errorMsg = "检查更新超时，请检查网络连接";
        } else if (
          errorStr.includes("network") ||
          errorStr.includes("fetch") ||
          errorStr.includes("connect")
        ) {
          errorMsg = "网络连接失败，请检查网络设置";
        } else if (errorStr.includes("404") || errorStr.includes("not found")) {
          errorMsg = "未找到更新信息，可能尚未发布新版本";
        } else if (
          errorStr.includes("certificate") ||
          errorStr.includes("ssl") ||
          errorStr.includes("tls")
        ) {
          errorMsg = "安全连接失败，请检查系统时间或网络环境";
        } else if (errorStr.includes("signature") || errorStr.includes("verify")) {
          errorMsg = "更新签名验证失败，请从官方渠道下载";
        }
        onError(errorMsg);
      }
    },
    [onError, onToast],
  );

  const downloadAndInstall = useCallback(async () => {
    try {
      setUpdateStatus("downloading");
      const update = await check();
      if (!update) {
        setUpdateStatus("idle");
        return;
      }

      let downloaded = 0;
      let contentLength = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength || 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (contentLength > 0) {
              setDownloadProgress(Math.round((downloaded / contentLength) * 100));
            }
            break;
          case "Finished":
            setDownloadProgress(100);
            break;
        }
      });

      setUpdateStatus("ready");
      await relaunch();
    } catch (err) {
      console.error("下载更新失败:", err);
      setUpdateStatus("available");

      const errorStr = String(err).toLowerCase();
      let errorMsg = "下载更新失败，请稍后重试";
      if (errorStr.includes("timeout") || errorStr.includes("timed out")) {
        errorMsg = "下载超时，请检查网络连接后重试";
      } else if (
        errorStr.includes("network") ||
        errorStr.includes("fetch") ||
        errorStr.includes("connect")
      ) {
        errorMsg = "网络连接中断，请检查网络后重试";
      } else if (errorStr.includes("space") || errorStr.includes("disk")) {
        errorMsg = "磁盘空间不足，请清理后重试";
      } else if (errorStr.includes("permission") || errorStr.includes("access")) {
        errorMsg = "没有写入权限，请以管理员身份运行";
      } else if (errorStr.includes("signature") || errorStr.includes("verify")) {
        errorMsg = "安装包签名验证失败，请从官方渠道下载";
      }
      onError(errorMsg);
    }
  }, [onError]);

  return {
    updateStatus,
    updateInfo,
    downloadProgress,
    showUpdateModal,
    setShowUpdateModal,
    checkForUpdates,
    downloadAndInstall,
    dismissUpdateModal,
  };
}

