import { Eye, EyeOff } from "lucide-react";

export type ApiKeyInputProps = {
  value: string;
  onChange: (value: string) => void;
  show: boolean;
  onToggleShow: () => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  inputClassName?: string;
  buttonClassName?: string;
};

export function ApiKeyInput({
  value,
  onChange,
  show,
  onToggleShow,
  placeholder,
  disabled,
  className,
  inputClassName,
  buttonClassName,
}: ApiKeyInputProps) {
  return (
    <div className={["relative", className].filter(Boolean).join(" ")}>
      <input
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        className={[
          "w-full px-3 py-2 pr-10 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-[rgba(106,155,204,0.20)] focus:border-[var(--steel)] transition-all disabled:opacity-60 disabled:cursor-not-allowed",
          inputClassName,
        ]
          .filter(Boolean)
          .join(" ")}
        placeholder={placeholder}
      />
      <button
        type="button"
        onClick={onToggleShow}
        disabled={disabled}
        className={[
          "absolute inset-y-0 right-0 pr-3 flex items-center text-stone-400 hover:text-[var(--ink)] disabled:hover:text-stone-400",
          buttonClassName,
        ]
          .filter(Boolean)
          .join(" ")}
        aria-label={show ? "隐藏" : "显示"}
      >
        {show ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  );
}
