/**
 * ConfigToggle - 带即时反馈的配置开关
 *
 * 特性：
 * - 乐观更新：Toggle 动画立即响应
 * - 状态指示：旁边的小点显示同步状态
 * - 错误回滚：失败时自动恢复原值
 */

import { useState, useEffect, useRef, type ComponentPropsWithoutRef } from "react";
import { Check, Loader2 } from "lucide-react";

type ToggleSize = "xs" | "sm" | "md";
type ToggleVariant = "blue" | "violet" | "amber" | "orange" | "green";
type SyncStatus = "idle" | "syncing" | "success" | "error";

const SIZE_STYLES: Record<
  ToggleSize,
  { track: string; knob: string; knobTranslateWhenChecked: string }
> = {
  xs: {
    track: "w-11 h-6",
    knob: "w-5 h-5",
    knobTranslateWhenChecked: "translate-x-5",
  },
  sm: {
    track: "w-12 h-6",
    knob: "w-5 h-5",
    knobTranslateWhenChecked: "translate-x-6",
  },
  md: {
    track: "w-14 h-7",
    knob: "w-6 h-6",
    knobTranslateWhenChecked: "translate-x-7",
  },
};

const VARIANT_TRACK_CHECKED: Record<ToggleVariant, string> = {
  blue: "bg-[var(--steel)]",
  violet: "bg-violet-500",
  amber: "bg-amber-500",
  orange: "bg-[var(--crail)]",
  green: "bg-[var(--sage)]",
};

export type ConfigToggleProps = Omit<
  ComponentPropsWithoutRef<"button">,
  "type" | "onChange"
> & {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** 异步提交回调，返回 Promise */
  onCommit?: (checked: boolean) => Promise<void>;
  size?: ToggleSize;
  variant?: ToggleVariant;
  /** 外部控制的同步状态 */
  syncStatus?: SyncStatus;
  /** 是否显示状态指示器 */
  showStatusIndicator?: boolean;
};

export function ConfigToggle({
  checked,
  onCheckedChange,
  onCommit,
  disabled,
  size = "md",
  variant = "blue",
  className,
  syncStatus: externalStatus,
  showStatusIndicator = true,
  ...rest
}: ConfigToggleProps) {
  const [internalStatus, setInternalStatus] = useState<SyncStatus>("idle");
  const [previousValue, setPreviousValue] = useState<boolean>(checked);
  const successTimeoutRef = useRef<number | null>(null);

  const status = externalStatus ?? internalStatus;
  const styles = SIZE_STYLES[size];
  const trackColor = checked ? VARIANT_TRACK_CHECKED[variant] : "bg-[var(--sand)]";

  // 清理 timeout
  useEffect(() => {
    return () => {
      if (successTimeoutRef.current) {
        window.clearTimeout(successTimeoutRef.current);
      }
    };
  }, []);

  const handleToggle = async () => {
    if (disabled || status === "syncing") return;

    const newValue = !checked;

    // 保存旧值用于回滚
    setPreviousValue(checked);

    // 乐观更新
    onCheckedChange(newValue);

    if (onCommit) {
      setInternalStatus("syncing");

      try {
        await onCommit(newValue);
        setInternalStatus("success");

        successTimeoutRef.current = window.setTimeout(() => {
          setInternalStatus("idle");
        }, 1500);
      } catch {
        // 回滚
        onCheckedChange(previousValue);
        setInternalStatus("error");

        successTimeoutRef.current = window.setTimeout(() => {
          setInternalStatus("idle");
        }, 2000);
      }
    }
  };

  const isSyncing = status === "syncing";
  const isSuccess = status === "success";
  const isError = status === "error";
  const isIdle = status === "idle";

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled || isSyncing}
        onClick={() => void handleToggle()}
        className={[
          "relative shrink-0 rounded-full transition-all duration-300",
          styles.track,
          trackColor,
          isError ? "ring-2 ring-red-200" : "",
          isSuccess ? "ring-2 ring-emerald-200" : "",
          disabled || isSyncing
            ? "opacity-50 cursor-not-allowed"
            : "cursor-pointer hover:opacity-90",
          className,
        ]
          .filter(Boolean)
          .join(" ")}
        {...rest}
      >
        <span
          className={[
            "absolute top-0.5 left-0.5 bg-white rounded-full shadow-md transition-all duration-300",
            styles.knob,
            checked ? styles.knobTranslateWhenChecked : "",
          ]
            .filter(Boolean)
            .join(" ")}
        />
      </button>

      {/* 状态指示器 - 轻量级小点/图标 */}
      {showStatusIndicator && !isIdle && (
        <div className="flex items-center justify-center w-4 h-4 animate-in fade-in zoom-in-50 duration-200">
          {isSyncing ? (
            <Loader2 className="w-3 h-3 text-stone-400 animate-spin" />
          ) : isSuccess ? (
            <Check className="w-3 h-3 text-emerald-500" />
          ) : isError ? (
            <span className="w-1.5 h-1.5 bg-red-400 rounded-full animate-pulse" />
          ) : null}
        </div>
      )}
    </div>
  );
}
