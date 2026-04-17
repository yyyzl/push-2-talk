import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function NotificationWindow() {
  const [message, setMessage] = useState("等待识别完成...");
  const [visible, setVisible] = useState(false);
  const notificationWindow = useRef(getCurrentWindow()).current;

  useEffect(() => {
    const unlistenFns: UnlistenFn[] = [];
    let hideTimer: number | null = null;

    const showTemporary = (nextMessage: string) => {
      setMessage(nextMessage);
      setVisible(true);
      invoke("show_notification_window").catch(console.error);
      if (hideTimer !== null) {
        window.clearTimeout(hideTimer);
      }
      hideTimer = window.setTimeout(() => {
        setVisible(false);
        notificationWindow.hide().catch(console.error);
      }, 2200);
    };

    const setup = async () => {
      unlistenFns.push(await listen("transcription_cancelled", () => {
        showTemporary("已取消本次转写");
      }));
      unlistenFns.push(await listen("error", () => {
        showTemporary("本次转写失败");
      }));
    };

    void setup();

    return () => {
      if (hideTimer !== null) {
        window.clearTimeout(hideTimer);
      }
      unlistenFns.forEach((fn) => fn());
    };
  }, []);

  return (
    <div
      className="fixed inset-0 flex items-center justify-center pointer-events-none"
      role="region"
      aria-live="polite"
      aria-label="识别通知区域"
    >
      <div
        className={[
          "pointer-events-auto rounded-full border px-4 py-2 text-sm font-semibold shadow-lg transition-all duration-200",
          visible
            ? "bg-white/95 border-[var(--stone)] text-[var(--ink)] opacity-100 translate-y-0"
            : "bg-white/0 border-transparent text-transparent opacity-0 translate-y-2",
        ].join(" ")}
      >
        {message}
      </div>
    </div>
  );
}
