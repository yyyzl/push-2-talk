import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  assessEvalReadiness,
  formatReadinessJson,
  formatReadinessText,
  runAsrEvalReadinessCli,
} from "../scripts/asrEvalReadinessCore";

const caseA = {
  audio_id: "phase0b-history-001",
  audio_wav_path: null,
  provider: "history-draft",
  raw_asr_text: "打开 cloud code",
  expected_text: "打开 Claude Code",
  user_final_text: "打开 Claude Code",
  category: "real_history_draft",
  notes: "人工确认",
  diagnostics: null,
};

const caseB = {
  ...caseA,
  audio_id: "phase0b-history-002",
  raw_asr_text: "打开 wind surf",
  expected_text: "打开 Windsurf",
  user_final_text: "打开 Windsurf",
};

test("readiness 统计 cases 目录中的正式 case 并达到门槛", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-ready-"));
  try {
    const casesDir = join(dir, "cases");
    await mkdir(casesDir);
    await writeFile(join(casesDir, "a.json"), JSON.stringify([caseA]), "utf8");
    await writeFile(join(casesDir, "b.json"), JSON.stringify([caseB]), "utf8");

    const summary = await assessEvalReadiness(casesDir, { minCases: 2 });

    assert.equal(summary.ready, true);
    assert.equal(summary.totalCases, 2);
    assert.equal(summary.minimumCases, 2);
    assert.equal(summary.missingCases, 0);
    assert.deepEqual(summary.caseFiles, ["a.json", "b.json"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness 可从 suite 目录解析 cases 子目录并报告 not ready", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-suite-"));
  try {
    const casesDir = join(dir, "tests", "asr_eval", "cases");
    await mkdir(casesDir, { recursive: true });
    await writeFile(join(casesDir, "mini.json"), JSON.stringify([caseA]), "utf8");

    const summary = await assessEvalReadiness(join(dir, "tests", "asr_eval"), { minCases: 80 });

    assert.equal(summary.ready, false);
    assert.equal(summary.totalCases, 1);
    assert.equal(summary.minimumCases, 80);
    assert.equal(summary.missingCases, 79);
    assert.match(summary.recommendation, /继续从 history 或 runtime diagnostics 生成 draft/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness 忽略非正式 case 和非 JSON 文件", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-invalid-"));
  try {
    await writeFile(join(dir, "valid.json"), JSON.stringify([caseA]), "utf8");
    await writeFile(
      join(dir, "draft-like.json"),
      JSON.stringify([{ ...caseB, review_status: "needs_review" }]),
      "utf8",
    );
    await writeFile(join(dir, "notes.md"), "ignore", "utf8");

    const summary = await assessEvalReadiness(dir, { minCases: 2 });

    assert.equal(summary.ready, false);
    assert.equal(summary.totalCases, 1);
    assert.deepEqual(summary.caseFiles, ["draft-like.json", "valid.json"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness CLI 默认 not ready 时失败，允许报告模式可返回摘要", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-cli-"));
  try {
    await writeFile(join(dir, "mini.json"), JSON.stringify([caseA]), "utf8");

    await assert.rejects(
      runAsrEvalReadinessCli(["--cases", dir, "--min-cases", "2"]),
      /ASR eval readiness 未达标/,
    );

    const result = await runAsrEvalReadinessCli([
      "--cases",
      dir,
      "--min-cases",
      "2",
      "--allow-not-ready",
      "--json",
    ]);

    assert.equal(result.summary.ready, false);
    assert.equal(result.outputFormat, "json");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness CLI 拒绝非法 min-cases", async () => {
  await assert.rejects(
    runAsrEvalReadinessCli(["--min-cases", "0"]),
    /--min-cases 必须是正整数/,
  );
});

test("readiness CLI 可写入 JSON 报告并自动创建父目录", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-out-json-"));
  try {
    await writeFile(join(dir, "mini.json"), JSON.stringify([caseA]), "utf8");
    const outPath = join(dir, "reports", "readiness.json");

    const result = await runAsrEvalReadinessCli([
      "--cases",
      dir,
      "--min-cases",
      "2",
      "--allow-not-ready",
      "--json",
      "--out",
      outPath,
    ]);

    assert.equal(result.outPath, outPath);
    const written = JSON.parse(await readFile(outPath, "utf8"));
    assert.equal(written.totalCases, 1);
    assert.equal(written.minimumCases, 2);
    assert.equal(written.ready, false);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness CLI 可写入文本报告", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-out-text-"));
  try {
    await writeFile(join(dir, "ready.json"), JSON.stringify([caseA, caseB]), "utf8");
    const outPath = join(dir, "reports", "readiness.md");

    const result = await runAsrEvalReadinessCli([
      "--cases",
      dir,
      "--min-cases",
      "2",
      "--out",
      outPath,
    ]);

    assert.equal(result.summary.ready, true);
    assert.equal(result.outPath, outPath);
    const written = await readFile(outPath, "utf8");
    assert.match(written, /# ASR Eval Readiness/);
    assert.match(written, /ready: true/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness CLI 未达标且不允许报告时不写 out 文件", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-readiness-out-fail-"));
  try {
    await writeFile(join(dir, "mini.json"), JSON.stringify([caseA]), "utf8");
    const outPath = join(dir, "reports", "readiness.json");

    await assert.rejects(
      runAsrEvalReadinessCli(["--cases", dir, "--min-cases", "2", "--json", "--out", outPath]),
      /ASR eval readiness 未达标/,
    );
    await assert.rejects(readFile(outPath, "utf8"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("readiness 格式化输出包含关键字段", async () => {
  const summary = {
    casesPath: "tests/asr_eval/cases",
    totalCases: 26,
    minimumCases: 80,
    missingCases: 54,
    ready: false,
    caseFiles: ["claude_code_mvp.json", "tech_terms_mini.json"],
    recommendation: "继续从 history 或 runtime diagnostics 生成 draft，人工确认后再 promote。",
  };

  assert.match(formatReadinessText(summary), /ready: false/);
  assert.match(formatReadinessText(summary), /missing_cases: 54/);
  assert.equal(JSON.parse(formatReadinessJson(summary)).minimumCases, 80);
});
