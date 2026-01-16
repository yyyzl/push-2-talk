import type { ComponentPropsWithoutRef } from "react";

type ToggleSize = "xs" | "sm" | "md";
type ToggleVariant = "blue" | "violet" | "amber" | "orange" | "green" | "indigo";

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
  indigo: "bg-[var(--crail)]",
};

export type ToggleProps = Omit<
  ComponentPropsWithoutRef<"button">,
  "type" | "onChange"
> & {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  size?: ToggleSize;
  variant?: ToggleVariant;
};

export function Toggle({
  checked,
  onCheckedChange,
  disabled,
  size = "md",
  variant = "blue",
  className,
  ...rest
}: ToggleProps) {
  const styles = SIZE_STYLES[size];
  const trackColor = checked ? VARIANT_TRACK_CHECKED[variant] : "bg-[var(--sand)]";

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={[
        "relative rounded-full transition-all duration-300",
        styles.track,
        trackColor,
        disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer hover:opacity-90",
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
  );
}
