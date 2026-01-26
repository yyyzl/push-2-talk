import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

export type ModalProps = {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  className?: string;
};

export function Modal({ open, onClose, children, className = "" }: ModalProps) {
  const [active, setActive] = useState(false);

  useEffect(() => {
    // 延迟一点点设置 active，以触发 transition
    if (open) {
      requestAnimationFrame(() => setActive(true));
    } else {
      setActive(false);
    }
  }, [open]);

  // 当关闭时，等待动画结束后再卸载（这里简单处理，实际上直接卸载也行，或者用 AnimatePresence）
  if (!open && !active) return null;

  return createPortal(
    <div
      className={`fixed inset-0 z-50 flex items-center justify-center p-4 transition-all duration-300 ${active ? "visible" : "invisible"
        }`}
    >
      {/* Backdrop with Glassmorphism */}
      <div
        className={`absolute inset-0 bg-stone-900/20 backdrop-blur-sm transition-opacity duration-300 ${active ? "opacity-100" : "opacity-0"
          }`}
        onClick={onClose}
      />

      {/* Content Container */}
      <div
        className={`relative z-10 w-full max-w-2xl transform transition-all duration-300 ${active ? "scale-100 opacity-100 translate-y-0" : "scale-95 opacity-0 translate-y-4"
          } ${className}`}
      >
        {children}
      </div>
    </div>,
    document.body
  );
}
