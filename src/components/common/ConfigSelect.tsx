/**
 * ConfigSelect - 带即时反馈的配置选择器
 *
 * 特性：
 * - 乐观更新：UI 立即响应用户操作
 * - 状态指示：loading → success → idle
 * - 保存失败：保留用户选择，交由配置状态提供重试
 */

import { useState, useEffect, useRef } from "react";
import { Select, type SelectProps } from "./Select";

export type ConfigSyncStatus = "idle" | "syncing" | "success" | "error";

export type ConfigSelectProps<T extends string> = Omit<SelectProps<T>, "status"> & {
  onCommit?: (value: T) => Promise<void>;
  syncStatus?: ConfigSyncStatus;
};

export function ConfigSelect<T extends string>({
  id,
  value,
  onChange,
  onCommit,
  options,
  disabled,
  className,
  syncStatus: externalStatus,
  ...selectProps
}: ConfigSelectProps<T>) {
  const [internalStatus, setInternalStatus] = useState<ConfigSyncStatus>("idle");
  const successTimeoutRef = useRef<number | null>(null);

  const status = externalStatus ?? internalStatus;

  // 清理 timeout
  useEffect(() => {
    return () => {
      if (successTimeoutRef.current) {
        window.clearTimeout(successTimeoutRef.current);
      }
    };
  }, []);

  const handleChange = async (newValue: T) => {
    if (disabled || status === "syncing") return;

    if (successTimeoutRef.current) window.clearTimeout(successTimeoutRef.current);

    // 乐观更新
    onChange(newValue);

    if (onCommit) {
      setInternalStatus("syncing");

      try {
        await onCommit(newValue);
        setInternalStatus("success");

        // 1.5s 后回到 idle
        successTimeoutRef.current = window.setTimeout(() => {
          setInternalStatus("idle");
        }, 1500);
      } catch {
        setInternalStatus("error");

        // 2s 后回到 idle
        successTimeoutRef.current = window.setTimeout(() => {
          setInternalStatus("idle");
        }, 2000);
      }
    }
  };

  return (
    <Select {...selectProps} id={id} value={value} options={options} disabled={disabled}
      className={className} status={status} onChange={next => void handleChange(next)} />
  );
}
