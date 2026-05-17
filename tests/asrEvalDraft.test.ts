import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  buildDraftCasesFromHistory,
  buildDraftCasesFromRuntimeDiagnostics,
  runAsrEvalDraftCli,
} from "../scripts/asrEvalDraftCore";

test("history draft 只收成功的普通听写记录并去重", () => {
  const cases = buildDraftCasesFromHistory(
    [
      {
        id: "h1",
        timestamp: 1,
        originalText: "我打开 cloud code",
        polishedText: "我打开 Claude Code",
        mode: "normal",
        success: true,
      },
      {
        id: "h2",
        timestamp: 2,
        originalText: "我打开 cloud code",
        polishedText: "我打开 Claude Code",
        mode: "normal",
        success: true,
      },
      {
        id: "h3",
        timestamp: 3,
        originalText: "助手模式不要进入",
        polishedText: "助手模式不要进入",
        mode: "assistant",
        success: true,
      },
      {
        id: "h4",
        timestamp: 4,
        originalText: "失败记录不要进入",
        polishedText: "失败记录不要进入",
        mode: "normal",
        success: false,
      },
    ],
    { idPrefix: "phase0b" },
  );

  assert.equal(cases.length, 1);
  assert.deepEqual(cases[0], {
    audio_id: "phase0b-history-001",
    audio_wav_path: null,
    provider: "history-draft",
    raw_asr_text: "我打开 cloud code",
    expected_text: "我打开 Claude Code",
    user_final_text: "我打开 Claude Code",
    category: "real_history_draft",
    notes: "history draft: 需要人工确认 expected_text 后再移入正式 cases；history_id=h1",
    diagnostics: null,
  });
});

test("runtime diagnostics draft 只收 changed 或 applied 的记录", () => {
  const cases = buildDraftCasesFromRuntimeDiagnostics(
    [
      {
        schema_version: 1,
        stage: "personalization",
        timestamp_ms: 1000,
        source_text: "我调用 open eye 接口",
        output_text: "我调用 OpenAI 接口",
        changed: true,
        candidate_count: 1,
        applied_count: 1,
      },
      {
        schema_version: 1,
        stage: "personalization",
        timestamp_ms: 1001,
        source_text: "没有变化",
        output_text: "没有变化",
        changed: false,
        candidate_count: 0,
        applied_count: 0,
      },
    ],
    { idPrefix: "phase0b" },
  );

  assert.equal(cases.length, 1);
  assert.equal(cases[0].audio_id, "phase0b-diagnostic-001");
  assert.equal(cases[0].provider, "runtime-diagnostic-draft");
  assert.equal(cases[0].raw_asr_text, "我调用 open eye 接口");
  assert.equal(cases[0].expected_text, "我调用 OpenAI 接口");
  assert.match(cases[0].notes, /需要人工确认/);
});

test("CLI 可以读取 history 文件并写出 draft JSON", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-draft-"));
  try {
    const historyPath = join(dir, "history.json");
    const outPath = join(dir, "draft.json");
    await writeFile(
      historyPath,
      JSON.stringify([
        {
          id: "history-1",
          timestamp: 10,
          originalText: "打开 wind surf",
          polishedText: "打开 Windsurf",
          mode: "normal",
          success: true,
        },
      ]),
      "utf8",
    );

    const result = await runAsrEvalDraftCli([
      "--history",
      historyPath,
      "--out",
      outPath,
      "--prefix",
      "real",
    ]);

    assert.equal(result.count, 1);
    const written = JSON.parse(await readFile(outPath, "utf8"));
    assert.equal(written[0].audio_id, "real-history-001");
    assert.equal(written[0].expected_text, "打开 Windsurf");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("CLI 可以读取 diagnostics 目录并忽略无关 JSON", async () => {
  const dir = await mkdtemp(join(tmpdir(), "asr-diagnostics-"));
  try {
    const outPath = join(dir, "drafts", "diagnostics.json");
    await writeFile(
      join(dir, "personalization-1000-a.json"),
      JSON.stringify({
        schema_version: 1,
        stage: "personalization",
        timestamp_ms: 1000,
        source_text: "测试 open eye key",
        output_text: "测试 OpenAI key",
        changed: true,
        candidate_count: 1,
        applied_count: 1,
      }),
      "utf8",
    );
    await writeFile(
      join(dir, "other.json"),
      JSON.stringify({
        source_text: "不应读取",
        output_text: "不应读取",
        changed: true,
        applied_count: 1,
      }),
      "utf8",
    );

    const result = await runAsrEvalDraftCli([
      "--diagnostics",
      dir,
      "--out",
      outPath,
      "--prefix",
      "real",
    ]);

    assert.equal(result.count, 1);
    const written = JSON.parse(await readFile(outPath, "utf8"));
    assert.equal(written[0].audio_id, "real-diagnostic-001");
    assert.equal(written[0].raw_asr_text, "测试 open eye key");
    assert.equal(written[0].expected_text, "测试 OpenAI key");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
