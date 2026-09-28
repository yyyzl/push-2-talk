import { ChevronDown, ExternalLink, Globe2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { HistoryRecord } from "../../types";

const CITATION_MARKER_PATTERN = /\[citation\]\((\d+):[a-z0-9]+\)/gi;

export function formatAssistantAnswerPreview(text: string): string {
  return text.replace(CITATION_MARKER_PATTERN, "[$1]");
}

function getCitationCount(record: HistoryRecord): number {
  return record.citations?.length ?? 0;
}

export function SearchStatusChip({ record }: { record: HistoryRecord }) {
  if (!record.webSearched) return null;

  const citationCount = getCitationCount(record);
  return (
    <span className="inline-flex items-center gap-1 text-[10px] bg-[rgba(59,130,246,0.12)] text-blue-700 px-1.5 py-0.5 rounded">
      <Globe2 size={10} />
      {citationCount > 0 ? `引用 ${citationCount}` : "已联网"}
    </span>
  );
}

export function SearchSourcesList({
  record,
  compact = false,
}: {
  record: HistoryRecord;
  compact?: boolean;
}) {
  const citations = record.citations ?? [];
  if (!record.webSearched || citations.length === 0) return null;

  return (
    <details className="rounded-xl border border-blue-100 bg-blue-50/40 px-3 py-2">
      <summary className="flex cursor-pointer list-none items-center justify-between text-xs font-semibold text-blue-700">
        <span>参考来源 · {citations.length} 条</span>
        <ChevronDown size={14} />
      </summary>
      <div className={compact ? "mt-2 space-y-1.5" : "mt-2 space-y-2"}>
        {citations.map((citation) => (
          <button
            key={`${citation.index}:${citation.id}`}
            type="button"
            onClick={() => void openUrl(citation.url)}
            className="block w-full rounded-lg border border-blue-100 bg-white/70 px-2.5 py-2 text-left transition-colors hover:border-blue-200 hover:bg-white"
          >
            <div className="flex items-start gap-2">
              <span className="mt-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-blue-100 text-[10px] font-bold text-blue-700">
                {citation.index}
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-center gap-1 text-xs font-semibold text-stone-800">
                  <span className="truncate">{citation.title}</span>
                  <ExternalLink size={11} className="shrink-0 text-blue-600" />
                </span>
                <span className="mt-0.5 block truncate text-[11px] text-blue-700">
                  {citation.source || citation.url}
                </span>
                {!compact && (
                  <span className="mt-1 line-clamp-2 block text-[11px] leading-relaxed text-stone-500">
                    {citation.snippet}
                  </span>
                )}
              </span>
            </div>
          </button>
        ))}
      </div>
    </details>
  );
}
