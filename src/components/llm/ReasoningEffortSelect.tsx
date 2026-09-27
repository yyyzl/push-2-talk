import type { ReasoningEffort } from "../../types";

export type ReasoningEffortSelectProps = {
  value?: ReasoningEffort;
  disabled?: boolean;
  label?: string;
  description?: string;
  onChange: (value: ReasoningEffort | undefined) => void;
};

const OPTIONS: Array<{ value: ReasoningEffort; label: string }> = [
  { value: "default", label: "默认（不额外传参）" },
  { value: "none", label: "关闭（适合润色/翻译）" },
  { value: "auto", label: "自动" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "极高" },
];

export function ReasoningEffortSelect({
  value,
  disabled,
  label = "思考模式",
  description,
  onChange,
}: ReasoningEffortSelectProps) {
  const selectedValue = value ?? "default";

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <label className="text-xs font-bold text-stone-500 uppercase tracking-widest">
          {label}
        </label>
        <span className="text-[11px] text-stone-400">默认不改变旧行为</span>
      </div>
      <select
        value={selectedValue}
        disabled={disabled}
        onChange={(event) => {
          const next = event.target.value as ReasoningEffort;
          onChange(next === "default" ? undefined : next);
        }}
        className="w-full px-4 py-3 bg-white border border-[var(--stone)] rounded-2xl text-sm font-semibold focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
      >
        {OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {description && <p className="text-xs text-stone-500 leading-relaxed">{description}</p>}
    </div>
  );
}
