import { Mic, Type } from "lucide-react";
import type { AppStatus, UsageStats } from "../../types";

export type TopStatusBarProps = {
  status: AppStatus;
  recordingTime: number;
  formatTime: (seconds: number) => string;
  usageStats?: UsageStats;
};

export function TopStatusBar({
  status,
  recordingTime,
  formatTime,
  usageStats,
}: TopStatusBarProps) {
  const isRecording = status === "recording";
  const isTranscribing = status === "transcribing";
  const isPolishing = status === "polishing";
  const isAssistantProcessing = status === "assistant_processing";
  const isProcessing = isTranscribing || isPolishing || isAssistantProcessing;

  return (
    <div className="px-6 py-3 border-b border-[var(--stone)] flex items-center justify-between bg-[var(--paper)] font-sans">
      <div
        className={`flex items-center gap-2 px-4 py-1.5 rounded-full border text-sm font-medium transition-all duration-300 ${
          isRecording
            ? "bg-[rgba(217,119,87,0.12)] border-[rgba(217,119,87,0.22)] text-[var(--crail)]"
            : isProcessing
              ? "bg-[rgba(106,155,204,0.12)] border-[rgba(106,155,204,0.22)] text-[var(--steel)]"
              : status === "running"
                ? "bg-[rgba(120,140,93,0.12)] border-[rgba(120,140,93,0.22)] text-[var(--sage)]"
                : "bg-[var(--paper)] border-[var(--stone)] text-[var(--stone-dark)]"
        }`}
      >
        <span className="relative flex h-2.5 w-2.5">
          {(isRecording || isProcessing || status === "running") && (
            <span
              className={`animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 ${
                isRecording
                  ? "bg-[var(--crail)]"
                  : isProcessing
                    ? "bg-[var(--steel)]"
                    : "bg-[var(--sage)]"
              }`}
            />
          )}
          <span
            className={`relative inline-flex rounded-full h-2.5 w-2.5 ${
              isRecording
                ? "bg-[var(--crail)]"
                : isProcessing
                  ? "bg-[var(--steel)]"
                  : status === "running"
                    ? "bg-[var(--sage)]"
                    : "bg-[var(--stone-dark)]"
            }`}
          />
        </span>
        <span>
          {isRecording
            ? `正在录音 ${formatTime(recordingTime)}`
              : isTranscribing
                ? "语音识别中..."
              : isPolishing
                ? "大模型增强中..."
                : isAssistantProcessing
                  ? "AI 助手处理中..."
                  : status === "running"
                    ? "运行中"
                    : "已停止"}
        </span>
      </div>

      <div className="flex items-center gap-2">
        {usageStats && (
          <div className="mr-2 flex items-center gap-2 text-xs tabular-nums">
            <div
              title="总录音时长 / 总录音条数"
              className={[
                "flex items-center gap-2 rounded-full px-3 py-1.5 border shadow-sm",
                "border-[rgba(176,174,165,0.55)]",
                "bg-[linear-gradient(135deg,rgba(217,119,87,0.12),rgba(250,249,245,0.92))]",
              ].join(" ")}
            >
              <span className="w-6 h-6 rounded-full flex items-center justify-center bg-[rgba(217,119,87,0.14)] text-[var(--crail)]">
                <Mic size={14} />
              </span>
              <span className="flex items-baseline gap-3">
                <span className="flex items-baseline gap-2">
                  <span className="text-[10px] font-bold tracking-widest text-[rgba(20,20,19,0.45)]">
                    时长
                  </span>
                  <span className="font-bold text-[var(--ink)]">
                    {Math.floor(usageStats.totalRecordingMs / 60000)}min
                  </span>
                </span>
                <span className="w-px h-4 bg-[rgba(20,20,19,0.12)]" aria-hidden="true" />
                <span className="flex items-baseline gap-2">
                  <span className="text-[10px] font-bold tracking-widest text-[rgba(20,20,19,0.45)]">
                    条数
                  </span>
                  <span className="font-bold text-[var(--ink)]">
                    {usageStats.totalRecordingCount.toLocaleString()}
                  </span>
                </span>
              </span>
            </div>

            <div
              title="总识别字数"
              className={[
                "flex items-center gap-2 rounded-full px-3 py-1.5 border shadow-sm",
                "border-[rgba(176,174,165,0.55)]",
                "bg-[linear-gradient(135deg,rgba(106,155,204,0.12),rgba(250,249,245,0.92))]",
              ].join(" ")}
            >
              <span className="w-6 h-6 rounded-full flex items-center justify-center bg-[rgba(106,155,204,0.14)] text-[var(--steel)]">
                <Type size={14} />
              </span>
              <span className="flex items-baseline gap-2">
                <span className="text-[10px] font-bold tracking-widest text-[rgba(20,20,19,0.45)]">
                  识别
                </span>
                <span className="font-bold text-[var(--ink)]">
                  {usageStats.totalRecognizedChars.toLocaleString()}
                </span>
              </span>
            </div>
          </div>
        )}
        {/* {canStart && (
          <button
            onClick={onStartService}
            disabled={startServiceDisabled}
            className={[
              "px-4 py-2 rounded-2xl text-sm font-bold transition-colors",
              "border shadow-sm bg-[var(--crail)] border-[var(--crail)] text-[var(--paper)] hover:opacity-90",
              startServiceDisabled ? "opacity-50 cursor-not-allowed" : "",
            ].join(" ")}
          >
            <span className="flex items-center gap-2">
              <Sparkles size={16} /> 启动助手
            </span>
          </button>
        )} */}

      </div>
    </div>
  );
}
