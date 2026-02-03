import type { MouseEvent, RefObject } from "react";
import type { HistoryRecord } from "../types";
import { TranscriptDisplay } from "../components/live/TranscriptDisplay";
import { RecentActivity } from "../components/live/RecentActivity";

export type DashboardPageProps = {
  transcript: string;
  originalTranscript: string | null;
  currentMode: string | null;
  asrTime: number | null;
  llmTime: number | null;
  totalTime: number | null;
  activePresetName: string | null;
  transcriptEndRef: RefObject<HTMLDivElement>;
  onCopyText: (text: string, e?: MouseEvent) => void;

  history: HistoryRecord[];
  onOpenHistory: () => void;

  enablePostProcess: boolean;
  enableDictionaryEnhancement: boolean;
};

export function DashboardPage({
  transcript,
  originalTranscript,
  currentMode,
  asrTime,
  llmTime,
  totalTime,
  activePresetName,
  transcriptEndRef,
  onCopyText,
  history,
  onOpenHistory,
  enablePostProcess,
  enableDictionaryEnhancement,
}: DashboardPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <TranscriptDisplay
        transcript={transcript}
        originalTranscript={originalTranscript}
        currentMode={currentMode}
        asrTime={asrTime}
        llmTime={llmTime}
        totalTime={totalTime}
        activePresetName={activePresetName}
        transcriptEndRef={transcriptEndRef}
        onCopy={onCopyText}
        variant="compact"
        enablePostProcess={enablePostProcess}
        enableDictionaryEnhancement={enableDictionaryEnhancement}
      />

      <RecentActivity history={history} onCopyText={onCopyText} onOpenHistory={onOpenHistory} />
    </div>
  );
}

