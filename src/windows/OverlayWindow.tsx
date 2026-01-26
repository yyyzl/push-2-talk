import { AppConfig } from "../types";
import { useState, useEffect, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// 音频级别事件 payload 类型
interface AudioLevelPayload {
  level: number;
}

// 状态类型
type OverlayStatus = "recording" | "transcribing";

// 静音阈值常量（与后端 NOISE_FLOOR 对齐）
const SILENCE_THRESHOLD = 0.005;

// Hook 返回类型
interface AudioVisualizationState {
  level: number;
  time: number;
}

// ==========================================
// 核心：高性能音频可视化 Hook
// ==========================================
const useSmoothAudioLevel = (isRecording: boolean): AudioVisualizationState => {
  const [state, setState] = useState<AudioVisualizationState>({ level: 0, time: 0 });
  const targetRef = useRef(0);
  const currentRef = useRef(0);
  const timeRef = useRef(0);
  const animationRef = useRef<number>(0);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const setup = async () => {
      unlisten = await listen<AudioLevelPayload>("audio_level_update", (event) => {
        targetRef.current = Math.min(Math.pow(event.payload.level, 0.45), 1.0);
      });
    };
    setup();
    return () => { if (unlisten) unlisten(); };
  }, []);

  useEffect(() => {
    if (!isRecording) {
      cancelAnimationFrame(animationRef.current);
      currentRef.current = 0;
      targetRef.current = 0;
      timeRef.current = 0;
      setState({ level: 0, time: 0 });
      return;
    }

    const animate = () => {
      const target = targetRef.current;
      const current = currentRef.current;
      const speed = target > current ? 0.5 : 0.06;
      currentRef.current += (target - current) * speed;

      if (currentRef.current < SILENCE_THRESHOLD) {
        currentRef.current = 0;
      }

      timeRef.current = (timeRef.current + 0.08) % (Math.PI * 100);

      setState({
        level: currentRef.current,
        time: timeRef.current
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationRef.current);
  }, [isRecording]);

  return state;
};

// ==========================================
// 组件部分
// ==========================================

function WaveBar({ height }: { height: number }) {
  return (
    <div
      className="wave-bar"
      style={{ height: `${height}px` }}
    />
  );
}

function WaveformBars({ level, time }: { level: number; time: number }) {
  const minHeight = 3;
  const maxHeight = 28;
  const baseScales = [0.3, 0.45, 0.65, 0.85, 1.0, 0.85, 0.65, 0.45, 0.3];

  return (
    <div className="wave-container">
      {baseScales.map((baseScale, i) => {
        if (level === 0) {
          return <WaveBar key={i} height={minHeight} />;
        }
        const distanceFromCenter = Math.abs(i - 4);
        const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;
        const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;
        const dynamicScale = baseScale + flowWave * level + turbulence;
        const height = minHeight + level * dynamicScale * (maxHeight - minHeight);
        return <WaveBar key={i} height={Math.max(minHeight, Math.min(maxHeight, height))} />;
      })}
    </div>
  );
}

function LoadingIndicator() {
  return (
    <div className="loading-container">
      <div className="dots-container">
        {[...Array(9)].map((_, i) => (
          <div key={i} className="dot" style={{ animationDelay: `${i * 0.1}s` }} />
        ))}
      </div>
      <div className="spinner-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="4" />
          <line x1="12" y1="2" x2="12" y2="6" />
          <line x1="12" y1="18" x2="12" y2="22" />
          <line x1="2" y1="12" x2="6" y2="12" />
          <line x1="18" y1="12" x2="22" y2="12" />
          <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
          <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
          <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" />
          <line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
        </svg>
      </div>
    </div>
  );
}

function LockedControls({
  onFinish,
  onCancel,
  level,
  time,
  disabled
}: {
  onFinish: () => void;
  onCancel: () => void;
  level: number;
  time: number;
  disabled: boolean;
}) {
  const minHeight = 3;
  const maxHeight = 26;
  const baseScales = [0.35, 0.55, 0.8, 1.0, 0.8, 0.55, 0.35];

  return (
    <div className="locked-controls">
      <button
        onClick={onCancel}
        disabled={disabled}
        className={`locked-btn locked-btn-cancel ${disabled ? 'opacity-50' : ''}`}
        title="取消 (Esc)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>

      <div className="locked-wave-mini">
        {baseScales.map((baseScale, i) => {
          if (level === 0) {
            return (
              <div key={i} className="wave-bar-mini" style={{ height: `${minHeight}px` }} />
            );
          }
          const distanceFromCenter = Math.abs(i - 3);
          const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;
          const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;
          const dynamicScale = baseScale + flowWave * level + turbulence;
          const height = minHeight + level * dynamicScale * (maxHeight - minHeight);

          return (
            <div
              key={i}
              className="wave-bar-mini"
              style={{ height: `${Math.max(minHeight, Math.min(maxHeight, height))}px` }}
            />
          );
        })}
      </div>

      <button
        onClick={onFinish}
        disabled={disabled}
        className={`locked-btn locked-btn-finish ${disabled ? 'opacity-50' : ''}`}
        title="发送 (Enter)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      </button>
    </div>
  );
}

// 主悬浮窗组件
export default function OverlayWindow() {
  const [status, setStatus] = useState<OverlayStatus>("recording");
  const [isLocked, setIsLocked] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [theme, setTheme] = useState("light");

  const { level: audioLevel, time: animationTime } = useSmoothAudioLevel(status === "recording");

  useEffect(() => {
    invoke<AppConfig>("get_config").then(config => {
      setTheme(config.theme || "light");
    }).catch(console.error);
  }, []);

  useEffect(() => {
    const unlistenFns: UnlistenFn[] = [];
    let cancelled = false;

    const setup = async () => {
      const registerListener = async (
        event: string,
        handler: (event: any) => void,
      ): Promise<boolean> => {
        const unlisten = await listen(event, handler);
        if (cancelled) {
          unlisten();
          return false;
        }
        unlistenFns.push(unlisten);
        return true;
      };

      if (!(await registerListener("config_updated", (event) => {
        const config = event.payload as AppConfig;
        console.log("[OverlayWindow] 收到 config_updated 事件, theme=", config.theme);
        setTheme(config.theme || "light");
      }))) return;

      if (!(await registerListener("recording_started", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("recording_locked", () => {
        console.log("进入松手模式");
        setIsLocked(true);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("recording_stopped", () => {
        setStatus("transcribing");
      }))) return;

      if (!(await registerListener("transcribing", () => {
        setStatus("transcribing");
      }))) return;

      if (!(await registerListener("transcription_complete", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("error", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;

      if (!(await registerListener("transcription_cancelled", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      }))) return;
    };

    setup();

    return () => {
      cancelled = true;
      unlistenFns.forEach(fn => fn());
    };
  }, []);

  useEffect(() => {
    if (status === "transcribing") {
      const timeout = setTimeout(async () => {
        console.warn("转写超时 15 秒，强制调用隐藏悬浮窗");
        try {
          await invoke("hide_overlay");
          setStatus("recording");
          setIsLocked(false);
          setIsSubmitting(false);
        } catch (e) {
          console.error("强制隐藏悬浮窗失败:", e);
        }
      }, 15000);
      return () => clearTimeout(timeout);
    }
  }, [status]);

  useEffect(() => {
    if (isLocked && !isSubmitting) {
      const timeout = setTimeout(async () => {
        console.warn("松手模式超时 60 秒，自动取消");
        setIsSubmitting(true);
        try {
          await invoke("cancel_locked_recording");
        } catch (e) {
          console.error("取消锁定录音失败:", e);
        }
      }, 60000);
      return () => clearTimeout(timeout);
    }
  }, [isLocked, isSubmitting]);

  const handleFinish = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await invoke("finish_locked_recording");
    } catch (e) {
      console.error("完成录音失败:", e);
      setIsSubmitting(false);
    }
  };

  const handleCancel = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await invoke("cancel_locked_recording");
    } catch (e) {
      console.error("取消录音失败:", e);
      setIsSubmitting(false);
    }
  };

  return (
    <div className={`overlay-root ${theme === "dark" ? "theme-dark" : "theme-light"}`}>
      <div className={`overlay-pill ${isLocked ? 'overlay-pill-locked' : ''}`}>
        {status === "recording" ? (
          isLocked ? (
            <LockedControls
              onFinish={handleFinish}
              onCancel={handleCancel}
              level={audioLevel}
              time={animationTime}
              disabled={isSubmitting}
            />
          ) : (
            <WaveformBars level={audioLevel} time={animationTime} />
          )
        ) : (
          <LoadingIndicator />
        )}
      </div>
    </div>
  );
}
