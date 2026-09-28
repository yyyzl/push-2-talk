import { Select } from "../common/Select";
import { useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ReasoningEffort } from "../../types";
import { reasoningSelectOptions, type ReasoningContext, type ReasoningOptions } from "../../utils/reasoningOptions";

export type ReasoningEffortSelectProps = {
  value?: ReasoningEffort;
  context: ReasoningContext;
  disabled?: boolean;
  label?: string;
  description?: string;
  onChange: (value: ReasoningEffort | undefined) => void;
};

export function ReasoningEffortSelect({ value, context, disabled, label = "思考模式", description, onChange }: ReasoningEffortSelectProps) {
  const id = useId();
  const [retry, setRetry] = useState(0);
  const [result, setResult] = useState<{ key: string; data?: ReasoningOptions; failed?: boolean }>();
  // Resolve through the same backend methods as requests, including legacy and
  // per-mode overrides. A late response must never expose another model's options.
  const key = JSON.stringify({ context, current: value ?? null });
  const current = result?.key === key ? result : undefined;
  useEffect(() => {
    let cancelled = false;
    setResult(undefined);
    invoke<ReasoningOptions>("get_reasoning_options", JSON.parse(key)).then(
      (data) => { if (!cancelled) setResult({ key, data }); },
      () => { if (!cancelled) setResult({ key, failed: true }); },
    );
    return () => { cancelled = true; };
  }, [key, retry]);

  const options = reasoningSelectOptions(value, current?.data);
  const hint = !current ? "正在读取当前模型的可用选项…"
    : current.failed ? "暂时无法读取可用选项，已有设置未改动。"
    : current.data?.legacy_hint ?? (current.data?.efforts.length === 1
      ? "当前模型暂无已确认的思考选项，沿用已有配置。"
      : "仅显示当前适配支持的选项；默认沿用已有配置。");

  return (
    <div className="space-y-2 min-w-0">
      <label htmlFor={id} className="text-sm font-semibold text-stone-700">{label}</label>
      {current?.data?.model && <p className="text-xs text-stone-600 break-all">{current.data.model}</p>}
      <Select id={id} value={value ?? "default"} disabled={disabled || !current || current.failed}
        aria-describedby={`${id}-hint`} options={options}
        onChange={next => {
          if (options.some(option => option.value === next && !option.disabled)) {
            onChange(next === "default" ? undefined : next);
          }
        }} />
      <p id={`${id}-hint`} className="text-xs text-stone-600 leading-relaxed" role="status">{hint}</p>
      {current?.failed && <button type="button" disabled={disabled} onClick={() => setRetry((n) => n + 1)} className="text-sm text-[var(--steel)] underline underline-offset-4">重新读取</button>}
      {description && <p className="text-xs text-stone-600 leading-relaxed">{description}</p>}
    </div>
  );
}
