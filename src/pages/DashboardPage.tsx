import type { MouseEvent, RefObject } from "react";
import type { HistoryRecord } from "../types";
import { TranscriptDisplay } from "../components/live/TranscriptDisplay";
import { RecentActivity } from "../components/live/RecentActivity";

export type DashboardPageProps = {
  transcript: string;
  originalTranscript: string | null;
  asrTime: number | null;
  totalTime: number | null;
  transcriptEndRef: RefObject<HTMLDivElement>;
  onCopyText: (text: string, e?: MouseEvent) => void;

  history: HistoryRecord[];
  onOpenHistory: () => void;
};

export function DashboardPage({
  transcript,
  originalTranscript,
  asrTime,
  totalTime,
  transcriptEndRef,
  onCopyText,
  history,
  onOpenHistory,
}: DashboardPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <TranscriptDisplay
        transcript={transcript}
        originalTranscript={originalTranscript}
        asrTime={asrTime}
        totalTime={totalTime}
        transcriptEndRef={transcriptEndRef}
        onCopy={onCopyText}
        variant="compact"
      />

      <RecentActivity history={history} onCopyText={onCopyText} onOpenHistory={onOpenHistory} />
    </div>
  );
}

