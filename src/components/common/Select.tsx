import { useEffect, useState, type ReactNode } from "react";
import * as Primitive from "@radix-ui/react-select";
import { Check, ChevronDown, ChevronUp, Loader2 } from "lucide-react";

export type SelectStatus = "idle" | "syncing" | "success" | "error";
export type SelectOption<T extends string> = {
  value: T;
  label: ReactNode;
  description?: string;
  disabled?: boolean;
};
export type SelectProps<T extends string> = {
  id?: string;
  value: T;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  disabled?: boolean;
  className?: string;
  size?: "default" | "compact";
  status?: SelectStatus;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
};

// Radix owns focus, typeahead, keyboard navigation and popup collision handling.
// Prefix all values so a legitimate empty "inherit" setting remains selectable.
export function Select<T extends string>({
  id, value, onChange, options, disabled, className, size = "default", status = "idle", ...aria
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const unavailable = disabled || status === "syncing" || options.length === 0;
  useEffect(() => { if (unavailable) setOpen(false); }, [unavailable]);
  const items: SelectOption<T>[] = options.some(option => option.value === value)
    ? options
    : [{ value, label: value || "暂无可选项", disabled: true }, ...options];
  return (
    <div className={["min-w-0", className].filter(Boolean).join(" ")} onClick={event => event.stopPropagation()}>
      <Primitive.Root
        value={`v:${value}`}
        onValueChange={next => onChange(next.slice(2) as T)}
        open={open && !unavailable}
        onOpenChange={setOpen}
        disabled={unavailable}
      >
        <Primitive.Trigger id={id} {...aria} className="app-select-trigger" data-size={size}
          data-status={status} aria-busy={status === "syncing"} aria-invalid={status === "error" || undefined}>
          <span className="min-w-0 flex-1 truncate"><Primitive.Value>{items.find(option => option.value === value)?.label}</Primitive.Value></span>
          <Primitive.Icon className="app-select-icon">
            {status === "syncing" ? <Loader2 size={15} className="animate-spin motion-reduce:animate-none" />
              : status === "success" ? <Check size={15} />
                : <ChevronDown size={15} />}
          </Primitive.Icon>
        </Primitive.Trigger>
        <Primitive.Portal>
          <Primitive.Content className="app-select-content" position="popper" sideOffset={6} collisionPadding={12}>
            <Primitive.ScrollUpButton className="app-select-scroll"><ChevronUp size={14} /></Primitive.ScrollUpButton>
            <Primitive.Viewport className="app-select-viewport">
              {items.map(option => (
                <Primitive.Item key={option.value} value={`v:${option.value}`} disabled={option.disabled}
                  className="app-select-item" aria-label={typeof option.label === "string" ? option.label : undefined}>
                  <div className="min-w-0">
                    <Primitive.ItemText>{option.label}</Primitive.ItemText>
                    {option.description && <div className="app-select-description">{option.description}</div>}
                  </div>
                  <Primitive.ItemIndicator className="app-select-check"><Check size={15} /></Primitive.ItemIndicator>
                </Primitive.Item>
              ))}
            </Primitive.Viewport>
            <Primitive.ScrollDownButton className="app-select-scroll"><ChevronDown size={14} /></Primitive.ScrollDownButton>
          </Primitive.Content>
        </Primitive.Portal>
      </Primitive.Root>
      <span className="sr-only" role="status">
        {status === "syncing" ? "正在保存" : status === "success" ? "已保存" : status === "error" ? "保存失败，选择已保留，可重试保存" : ""}
      </span>
    </div>
  );
}
