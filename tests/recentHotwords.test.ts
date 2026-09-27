import assert from "node:assert/strict";
import test from "node:test";
import {
  buildRecentHotwordEntries,
  extractRecentHotwordCandidates,
  RECENT_HOTWORD_WINDOW_MS,
} from "../src/utils/recentHotwords";
import type { HistoryRecord } from "../src/types";

const record = (
  overrides: Partial<HistoryRecord> & Pick<HistoryRecord, "timestamp" | "originalText">,
): HistoryRecord => ({
  id: `history-${overrides.timestamp}`,
  timestamp: overrides.timestamp,
  originalText: overrides.originalText,
  polishedText: overrides.polishedText ?? null,
  selectedText: null,
  presetName: null,
  mode: overrides.mode ?? "normal",
  asrTimeMs: 0,
  llmTimeMs: null,
  totalTimeMs: 0,
  success: overrides.success ?? true,
  errorMessage: overrides.errorMessage ?? null,
});

test("recent hotwords 只从 24h 内成功历史生成 recent metadata", () => {
  const now = new Date("2026-05-16T10:00:00+08:00").getTime();
  const entries = buildRecentHotwordEntries([
    record({
      timestamp: now - 1_000,
      originalText: "我刚刚在用 claude code",
      polishedText: "我刚刚在用 Claude Code 和 GPT-5.3-Codex",
    }),
    record({
      timestamp: now - 2_000,
      originalText: "Windsurf 再打开一次",
    }),
    record({
      timestamp: now - RECENT_HOTWORD_WINDOW_MS - 1,
      originalText: "旧的 OldProduct 不应该进入",
    }),
    record({
      timestamp: now - 3_000,
      originalText: "失败的 Kubernetes 不应该进入",
      success: false,
      errorMessage: "ASR failed",
    }),
  ], now);

  assert.ok(entries.includes("Claude Code|recent|generic"));
  assert.ok(entries.includes("GPT-5.3-Codex|recent|code_symbol"));
  assert.ok(entries.includes("Windsurf|recent|generic"));
  assert.doesNotMatch(entries.join("\n"), /OldProduct|Kubernetes/);
});

test("recent hotwords 应去重、按最近顺序保留，并过滤普通句子碎片", () => {
  const now = 1_000_000;
  const entries = buildRecentHotwordEntries([
    record({
      timestamp: now,
      originalText: "Claude Code 很好用，今天我们继续写普通句子",
    }),
    record({
      timestamp: now - 1_000,
      originalText: "claude code 又出现一次，还有 this should not become hotwords",
    }),
  ], now, 5);

  assert.equal(entries.filter((entry) => entry.startsWith("Claude Code|")).length, 1);
  assert.equal(entries[0], "Claude Code|recent|generic");
  assert.doesNotMatch(entries.join("\n"), /ordinary|普通句子|this|should|become/);
});

test("extractRecentHotwordCandidates 应覆盖大小写、数字和连字符技术词", () => {
  const candidates = extractRecentHotwordCandidates(
    "TypeScript、OpenAI、GPT-5.3-Codex、useState 以及 Claude Code 都出现了",
  );

  assert.deepEqual(
    candidates.filter((candidate) =>
      ["TypeScript", "OpenAI", "GPT-5.3-Codex", "useState", "Claude Code"].includes(candidate),
    ),
    ["TypeScript", "OpenAI", "GPT-5.3-Codex", "useState", "Claude Code"],
  );
});
