import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

type JsonRecord = Record<string, unknown>;
type OutputFormat = "text" | "json";

export interface EvalReadinessSummary {
  casesPath: string;
  totalCases: number;
  minimumCases: number;
  missingCases: number;
  ready: boolean;
  caseFiles: string[];
  recommendation: string;
}

export interface EvalReadinessOptions {
  minCases?: number;
}

export interface EvalReadinessCliResult {
  summary: EvalReadinessSummary;
  outputFormat: OutputFormat;
  outPath?: string;
}

interface CliArgs {
  targetPath: string;
  minCases: number;
  allowNotReady: boolean;
  outputFormat: OutputFormat;
  outPath?: string;
}

const DEFAULT_SUITE_PATH = "tests/asr_eval";
const DEFAULT_MIN_CASES = 80;

export async function assessEvalReadiness(
  inputPath: string = DEFAULT_SUITE_PATH,
  options: EvalReadinessOptions = {},
): Promise<EvalReadinessSummary> {
  const minimumCases = options.minCases ?? DEFAULT_MIN_CASES;
  assertPositiveInteger(minimumCases, "--min-cases");
  const casesPath = await resolveCasesPath(inputPath);
  const caseFiles = await listCaseFiles(casesPath);
  let totalCases = 0;

  for (const fileName of caseFiles) {
    const input = JSON.parse(await readFile(join(casesPath, fileName), "utf8"));
    totalCases += extractCaseArray(input).filter(isFormalEvalCase).length;
  }

  const missingCases = Math.max(0, minimumCases - totalCases);
  const ready = missingCases === 0;

  return {
    casesPath,
    totalCases,
    minimumCases,
    missingCases,
    ready,
    caseFiles,
    recommendation: ready
      ? "正式 ASR eval case 数量已达到 Phase 0B readiness 门槛，可以继续结合质量指标评估后续阶段。"
      : "继续从 history 或 runtime diagnostics 生成 draft，人工确认 expected_text 后再 promote 到正式 cases。",
  };
}

export async function runAsrEvalReadinessCli(
  argv: string[],
): Promise<EvalReadinessCliResult> {
  const args = parseCliArgs(argv);
  const summary = await assessEvalReadiness(args.targetPath, { minCases: args.minCases });

  if (!summary.ready && !args.allowNotReady) {
    throw new Error(
      [
        `ASR eval readiness 未达标: ${summary.totalCases}/${summary.minimumCases} cases`,
        `仍缺 ${summary.missingCases} 条`,
        summary.recommendation,
      ].join("；"),
    );
  }

  if (args.outPath) {
    await writeReadinessOutput(args.outPath, formatReadiness(summary, args.outputFormat));
  }

  return {
    summary,
    outputFormat: args.outputFormat,
    outPath: args.outPath,
  };
}

export function formatReadinessText(summary: EvalReadinessSummary): string {
  return [
    "# ASR Eval Readiness",
    "",
    `- cases_path: ${summary.casesPath}`,
    `- total_cases: ${summary.totalCases}`,
    `- minimum_cases: ${summary.minimumCases}`,
    `- missing_cases: ${summary.missingCases}`,
    `- ready: ${summary.ready}`,
    `- case_files: ${summary.caseFiles.join(", ") || "(none)"}`,
    `- recommendation: ${summary.recommendation}`,
  ].join("\n");
}

export function formatReadinessJson(summary: EvalReadinessSummary): string {
  return `${JSON.stringify(summary, null, 2)}\n`;
}

export function formatReadiness(summary: EvalReadinessSummary, format: OutputFormat): string {
  return format === "json" ? formatReadinessJson(summary) : `${formatReadinessText(summary)}\n`;
}

function parseCliArgs(argv: string[]): CliArgs {
  let targetPath = DEFAULT_SUITE_PATH;
  let sawSuite = false;
  let sawCases = false;
  let minCases = DEFAULT_MIN_CASES;
  let allowNotReady = false;
  let outputFormat: OutputFormat = "text";
  let outPath: string | undefined;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--suite":
        targetPath = requiredValue(argv, index, arg);
        sawSuite = true;
        index += 1;
        break;
      case "--cases":
        targetPath = requiredValue(argv, index, arg);
        sawCases = true;
        index += 1;
        break;
      case "--min-cases":
        minCases = parsePositiveInteger(requiredValue(argv, index, arg), arg);
        index += 1;
        break;
      case "--allow-not-ready":
        allowNotReady = true;
        break;
      case "--json":
        outputFormat = "json";
        break;
      case "--out":
        outPath = requiredValue(argv, index, arg);
        index += 1;
        break;
      case "--help":
        throw new Error(usage());
      default:
        throw new Error(`未知参数：${arg}\n\n${usage()}`);
    }
  }

  if (sawSuite && sawCases) {
    throw new Error("--suite 不能和 --cases 混用");
  }

  return {
    targetPath,
    minCases,
    allowNotReady,
    outputFormat,
    outPath,
  };
}

async function writeReadinessOutput(path: string, content: string): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, content, "utf8");
}

async function resolveCasesPath(inputPath: string): Promise<string> {
  const metadata = await stat(inputPath);
  if (!metadata.isDirectory()) {
    throw new Error(`ASR eval readiness 需要目录路径: ${inputPath}`);
  }

  const nestedCasesPath = join(inputPath, "cases");
  if (await isDirectory(nestedCasesPath)) {
    return nestedCasesPath;
  }

  return inputPath;
}

async function listCaseFiles(casesPath: string): Promise<string[]> {
  const entries = await readdir(casesPath, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
    .map((entry) => entry.name)
    .sort((a, b) => a.localeCompare(b));
}

function extractCaseArray(input: unknown): unknown[] {
  if (Array.isArray(input)) {
    return input;
  }

  if (isRecord(input) && Array.isArray(input.cases)) {
    return input.cases;
  }

  return [];
}

function isFormalEvalCase(value: unknown): boolean {
  if (!isRecord(value) || "review_status" in value || "review_notes" in value) {
    return false;
  }

  return Boolean(
    cleanText(value.audio_id) &&
      cleanText(value.raw_asr_text) &&
      cleanText(value.expected_text),
  );
}

async function isDirectory(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isDirectory();
  } catch {
    return false;
  }
}

function requiredValue(argv: string[], index: number, flag: string): string {
  const value = argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} 缺少参数值`);
  }
  return value;
}

function parsePositiveInteger(value: string, flag: string): number {
  const number = Number(value);
  assertPositiveInteger(number, flag);
  return number;
}

function assertPositiveInteger(value: number, label: string): void {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${label} 必须是正整数`);
  }
}

function cleanText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function usage(): string {
  const scriptName = basename("scripts/asr-eval-readiness.ts");
  return [
    "Usage:",
    `  npx tsx scripts/${scriptName} [--suite tests/asr_eval] [--min-cases 80]`,
    `  npx tsx scripts/${scriptName} --cases tests/asr_eval/cases --json --allow-not-ready`,
    "",
    "Options:",
    "  --suite <dir>          ASR eval suite 目录，默认 tests/asr_eval",
    "  --cases <dir>          直接指定正式 cases 目录",
    "  --min-cases <n>        最小正式 case 数，默认 80",
    "  --allow-not-ready      未达标时仍以 0 退出，适合生成报告",
    "  --json                 输出 JSON 摘要",
    "  --out <path>           将摘要写入文件，格式跟随 --json/text",
  ].join("\n");
}
