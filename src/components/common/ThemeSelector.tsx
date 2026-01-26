import { Sun, Moon } from "lucide-react";

export type ThemeSelectorProps = {
  value: string;
  onChange: (theme: string) => void;
  disabled?: boolean;
};

/**
 * 主题选择器 - 双选项卡设计
 *
 * 设计理念：
 * - 这是一个"选项"而非"开关"，因此采用分段控制器 (Segmented Control) 形式
 * - 两个选项并排放置，用户可以直观看到所有选择
 * - 选中项使用填充背景，未选中项保持透明
 * - 与项目整体 Anthropic 美学风格保持一致
 */
export function ThemeSelector({
  value,
  onChange,
  disabled = false,
}: ThemeSelectorProps) {
  const options = [
    {
      id: "light",
      label: "朱砂",
      icon: Sun,
      description: "朱砂暖色",
    },
    {
      id: "dark",
      label: "墨色",
      icon: Moon,
      description: "经典暗黑",
    },
  ] as const;

  return (
    <div
      className={[
        "inline-flex rounded-xl p-1",
        "bg-[var(--sand)] border border-[var(--stone)]",
        disabled ? "opacity-50 cursor-not-allowed" : "",
      ].join(" ")}
    >
      {options.map((option) => {
        const isSelected = value === option.id;
        const Icon = option.icon;

        return (
          <button
            key={option.id}
            type="button"
            disabled={disabled}
            onClick={() => {
              if (!disabled && !isSelected) {
                onChange(option.id);
              }
            }}
            className={[
              // 基础样式
              "relative flex items-center gap-1.5 px-3 py-1.5 rounded-lg",
              "text-xs font-bold transition-all duration-200",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--crail)] focus-visible:ring-offset-1",
              // 选中状态
              isSelected
                ? option.id === "light"
                  ? "bg-white text-[var(--crail)] shadow-sm"
                  : "bg-[var(--ink)] text-[#FAF9F5] shadow-sm"
                : "bg-transparent text-[var(--stone-dark)] hover:text-[var(--ink)]",
              // 禁用状态
              disabled ? "cursor-not-allowed" : "cursor-pointer",
            ].join(" ")}
            title={option.description}
          >
            <Icon
              size={14}
              className={[
                "transition-transform duration-200",
                isSelected ? "scale-110" : "scale-100",
              ].join(" ")}
            />
            <span>{option.label}</span>
          </button>
        );
      })}
    </div>
  );
}
