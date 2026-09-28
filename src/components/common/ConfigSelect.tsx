/**
 * ConfigSelect - 带即时反馈的配置选择器
 *
 * 特性：
 * - 乐观更新：UI 立即响应用户操作
 * - 状态指示：loading → success → idle
 * - 保存失败：保留用户选择，交由配置状态提供重试
 */

import { useState, useEffect, useRef, type ReactNode } from "react";
import { Check, Loader2 } from "lucide-react";

export type ConfigSyncStatus = "idle" | "syncing" | "success" | "error";

export type ConfigSelectProps<T extends string> = {
  id?: string;
  value: T;
  onChange: (value: T) => void;
  onCommit?: (value: T) => Promise<void>;
  options: { value: T; label: ReactNode }[];
  disabled?: boolean;
  className?: string;
  /** 外部控制的同步状态 */
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

  const isSyncing = status === "syncing";
  const isSuccess = status === "success";
  const isError = status === "error";

  return (
    <div className={["relative group", className].filter(Boolean).join(" ")}>
      <select
        id={id}
        value={value}
        disabled={disabled || isSyncing}
        onChange={(e) => void handleChange(e.target.value as T)}
        className={[
          "w-full bg-white border rounded-xl px-3 py-2 pr-9 text-xs font-bold outline-none appearance-none transition-all duration-200",
          isError
            ? "border-red-300 bg-red-50/50"
            : isSuccess
              ? "border-emerald-300 bg-emerald-50/30"
              : "border-[var(--stone)] focus:border-[var(--steel)]",
          disabled || isSyncing
            ? "opacity-60 cursor-not-allowed"
            : "cursor-pointer hover:border-stone-300",
          "shadow-sm",
        ].join(" ")}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* 状态指示器 */}
      <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none flex items-center justify-center w-4 h-4">
        {isSyncing ? (
          <Loader2
            className="w-3.5 h-3.5 text-stone-400 animate-spin"
          />
        ) : isSuccess ? (
          <Check
            className="w-3.5 h-3.5 text-emerald-500 animate-in zoom-in-50 duration-200"
          />
        ) : isError ? (
          <span className="w-1.5 h-1.5 bg-red-400 rounded-full animate-pulse" />
        ) : (
          <svg
            className="w-3.5 h-3.5 text-stone-400 transition-transform duration-200 group-hover:translate-y-0.5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path d="M19 9l-7 7-7-7" strokeWidth="2" />
          </svg>
        )}
      </div>
    </div>
  );
}
