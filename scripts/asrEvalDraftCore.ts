import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

type JsonRecord = Record<string, unknown>;
type ReviewStatus = "needs_review" | "approved" | "rejected";

export interface EvalDraftCase {
  audio_id: string;
  audio_wav_path: null;
  provider: "history-draft" | "runtime-diagnostic-draft";
  raw_asr_text: string;
  expected_text: string;
  user_final_text: string;
  category: "real_history_draft" | "runtime_personalization_draft";
  notes: string;
  review_status: ReviewStatus;
  review_notes: string;
  diagnostics: null;
}

export interface FormalEvalCase {
  audio_id: string;
  audio_wav_path: string | null;
  provider: string;
  raw_asr_text: string;
  expected_text: string;
  user_final_text: string;
  category: string;
  notes: string;
  diagnostics: null;
}

export interface DraftOptions {
  idPrefix?: string;
  limit?: number;
}

export interface CliResult {
  count: number;
  outPath: string;
}

interface CliArgs {
  historyPath?: string;
  diagnosticsPath?: string;
  promotePath?: string;
  outPath: string;
  idPrefix: string;
  limit?: number;
}

const DEFAULT_ID_PREFIX = "phase0b";

export function buildDraftCasesFromHistory(input: unknown, options: DraftOptions = {}): EvalDraftCase[] {
  const idPrefix = options.idPrefix ?? DEFAULT_ID_PREFIX;
  const records = extractInputArray(input, ["records", "history", "pushtotalk_history"]);
  const cases: EvalDraftCase[] = [];
  const seen = new Set<string>();

  for (const value of records) {
    if (!isRecord(value)) {
      continue;
    }

    if (value.success !== true || value.mode !== "normal") {
      continue;
    }

    const rawText = cleanText(value.originalText);
    if (!rawText) {
      continue;
    }

    const expectedText = cleanText(value.polishedText) || rawText;
    const key = dedupeKey(rawText, expectedText);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);

    const historyId = cleanText(value.id) || "unknown";
    const index = cases.length + 1;
    cases.push({
      audio_id: `${idPrefix}-history-${formatIndex(index)}`,
      audio_wav_path: null,
      provider: "history-draft",
      raw_asr_text: rawText,
      expected_text: expectedText,
      user_final_text: expectedText,
      category: "real_history_draft",
      notes: `history draft: 需要人工确认 expected_text 后再移入正式 cases；history_id=${historyId}`,
      review_status: "needs_review",
      review_notes: "",
      diagnostics: null,
    });

    if (options.limit !== undefined && cases.length >= options.limit) {
      break;
    }
  }

  return cases;
}

export function buildDraftCasesFromRuntimeDiagnostics(
  input: unknown,
  options: DraftOptions = {},
): EvalDraftCase[] {
  const idPrefix = options.idPrefix ?? DEFAULT_ID_PREFIX;
  const records = extractInputArray(input, ["diagnostics", "records", "cases"]);
  const cases: EvalDraftCase[] = [];
  const seen = new Set<string>();

  for (const value of records) {
    if (!isRecord(value)) {
      continue;
    }

    const rawText = cleanText(value.source_text);
    const expectedText = cleanText(value.output_text);
    if (!rawText || !expectedText) {
      continue;
    }

    const changed = value.changed === true;
    const appliedCount = numberValue(value.applied_count);
    if (!changed && appliedCount <= 0) {
      continue;
    }

    const key = dedupeKey(rawText, expectedText);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);

    const timestamp = cleanText(value.timestamp_ms) || "unknown";
    const index = cases.length + 1;
    cases.push({
      audio_id: `${idPrefix}-diagnostic-${formatIndex(index)}`,
      audio_wav_path: null,
      provider: "runtime-diagnostic-draft",
      raw_asr_text: rawText,
      expected_text: expectedText,
      user_final_text: expectedText,
      category: "runtime_personalization_draft",
      notes: `runtime diagnostic draft: 需要人工确认 expected_text 后再移入正式 cases；timestamp_ms=${timestamp}`,
      review_status: "needs_review",
      review_notes: "",
      diagnostics: null,
    });

    if (options.limit !== undefined && cases.length >= options.limit) {
      break;
    }
  }

  return cases;
}

export function promoteReviewedDraftCases(input: unknown, options: DraftOptions = {}): FormalEvalCase[] {
  const records = extractInputArray(input, ["drafts", "cases", "records"]);
  const promoted: FormalEvalCase[] = [];
  const seen = new Set<string>();

  for (const value of records) {
    if (!isRecord(value) || value.review_status !== "approved") {
      continue;
    }

    const audioId = cleanText(value.audio_id);
    const rawText = cleanText(value.raw_asr_text);
    const expectedText = cleanText(value.expected_text);
    if (!audioId || !rawText || !expectedText) {
      continue;
    }

    const key = dedupeKey(rawText, expectedText);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);

    promoted.push({
      audio_id: audioId,
      audio_wav_path: cleanAudioPath(value.audio_wav_path),
      provider: cleanText(value.provider) || "reviewed-draft",
      raw_asr_text: rawText,
      expected_text: expectedText,
      user_final_text: cleanText(value.user_final_text) || expectedText,
      category: cleanText(value.category) || "phase0b_reviewed",
      notes: cleanText(value.notes) || "promoted from reviewed draft",
      diagnostics: null,
    });

    if (options.limit !== undefined && promoted.length >= options.limit) {
      break;
    }
  }

  return promoted;
}

export async function runAsrEvalDraftCli(argv: string[]): Promise<CliResult> {
  const args = parseCliArgs(argv);
  if (args.promotePath) {
    const draftInput = await readJsonFile(args.promotePath);
    const promoted = promoteReviewedDraftCases(draftInput, { limit: args.limit });
    if (promoted.length === 0) {
      throw new Error("没有可提升的 approved draft case");
    }
    await writeJsonOutput(args.outPath, promoted);
    return {
      count: promoted.length,
      outPath: args.outPath,
    };
  }

  const cases: EvalDraftCase[] = [];

  if (args.historyPath) {
    const historyInput = await readJsonFile(args.historyPath);
    cases.push(...buildDraftCasesFromHistory(historyInput, { idPrefix: args.idPrefix }));
  }

  if (args.diagnosticsPath) {
    const diagnosticsInput = await readDiagnosticsInput(args.diagnosticsPath);
    cases.push(...buildDraftCasesFromRuntimeDiagnostics(diagnosticsInput, { idPrefix: args.idPrefix }));
  }

  const deduped = applyLimit(dedupeCases(cases), args.limit);
  await writeJsonOutput(args.outPath, deduped);

  return {
    count: deduped.length,
    outPath: args.outPath,
  };
}

async function readDiagnosticsInput(path: string): Promise<unknown[]> {
  const metadata = await stat(path);
  if (!metadata.isDirectory()) {
    return [await readJsonFile(path)];
  }

  const names = (await readdir(path))
    .filter((name) => name.startsWith("personalization-") && name.endsWith(".json"))
    .sort((a, b) => a.localeCompare(b));

  const values: unknown[] = [];
  for (const name of names) {
    values.push(await readJsonFile(join(path, name)));
  }

  return values;
}

async function readJsonFile(path: string): Promise<unknown> {
  return JSON.parse(await readFile(path, "utf8"));
}

function parseCliArgs(argv: string[]): CliArgs {
  let historyPath: string | undefined;
  let diagnosticsPath: string | undefined;
  let promotePath: string | undefined;
  let outPath: string | undefined;
  let idPrefix = DEFAULT_ID_PREFIX;
  let limit: number | undefined;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--history":
        historyPath = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--diagnostics":
        diagnosticsPath = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--promote":
        promotePath = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--out":
        outPath = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--prefix":
        idPrefix = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--limit":
        limit = parseLimit(requiredValue(argv, index, arg));
        index += 1;
        break;
      case "--help":
        throw new Error(usage());
      default:
        throw new Error(`未知参数：${arg}\n\n${usage()}`);
    }
  }

  if (promotePath && (historyPath || diagnosticsPath)) {
    throw new Error("--promote 不能和 --history 或 --diagnostics 混用");
  }
  if (!promotePath && !historyPath && !diagnosticsPath) {
    throw new Error(`必须提供 --history、--diagnostics 或 --promote。\n\n${usage()}`);
  }
  if (!outPath) {
    throw new Error(`必须提供 --out。\n\n${usage()}`);
  }

  return {
    historyPath,
    diagnosticsPath,
    promotePath,
    outPath,
    idPrefix,
    limit,
  };
}

function requiredValue(argv: string[], index: number, flag: string): string {
  const value = argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} 缺少参数值`);
  }
  return value;
}

function parseLimit(value: string): number {
  const limit = Number(value);
  if (!Number.isInteger(limit) || limit <= 0) {
    throw new Error("--limit 必须是正整数");
  }
  return limit;
}

function usage(): string {
  return [
    "Usage:",
    "  npx tsx scripts/asr-eval-draft.ts --history <history.json> --out tests/asr_eval/drafts/history.json",
    "  npx tsx scripts/asr-eval-draft.ts --diagnostics <file-or-dir> --out tests/asr_eval/drafts/diagnostics.json",
    "  npx tsx scripts/asr-eval-draft.ts --promote tests/asr_eval/drafts/reviewed.json --out tests/asr_eval/cases/phase0b-real.json",
    "",
    "Options:",
    "  --prefix <id-prefix>  audio_id 前缀，默认 phase0b",
    "  --limit <n>           限制输出条数",
  ].join("\n");
}

function extractInputArray(input: unknown, keys: string[]): unknown[] {
  if (Array.isArray(input)) {
    return input;
  }

  if (!isRecord(input)) {
    return [];
  }

  for (const key of keys) {
    const value = input[key];
    if (Array.isArray(value)) {
      return value;
    }
  }

  return [];
}

function dedupeCases(cases: EvalDraftCase[]): EvalDraftCase[] {
  const seen = new Set<string>();
  const deduped: EvalDraftCase[] = [];
  for (const item of cases) {
    const key = dedupeKey(item.raw_asr_text, item.expected_text);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    deduped.push(item);
  }
  return deduped;
}

function applyLimit(cases: EvalDraftCase[], limit: number | undefined): EvalDraftCase[] {
  return limit === undefined ? cases : cases.slice(0, limit);
}

async function writeJsonOutput(path: string, value: unknown): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function dedupeKey(rawText: string, expectedText: string): string {
  return `${rawText}\u0000${expectedText}`;
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cleanText(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }
  return "";
}

function cleanAudioPath(value: unknown): string | null {
  const text = cleanText(value);
  return text || null;
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function formatIndex(index: number): string {
  return String(index).padStart(3, "0");
}
