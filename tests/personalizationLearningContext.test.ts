import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("学习建议接受时应把上下文传给个性化纠错对", async () => {
  const source = await readFile("src/components/learning/VocabularyLearningToast.tsx", "utf8");

  assert.match(
    source,
    /invoke\("add_learned_word",\s*\{[\s\S]*context:\s*suggestion\.context/,
  );
  assert.match(source, /suggestion\.context,/);
});

test("已有词库词的学习建议应展示为保存纠错", async () => {
  const source = await readFile("src/components/learning/VocabularyLearningToast.tsx", "utf8");
  const typeSource = await readFile("src/types/index.ts", "utf8");

  assert.match(typeSource, /already_in_dictionary\?:\s*boolean/);
  assert.match(source, /suggestion\.already_in_dictionary/);
  assert.match(source, /保存纠错/);
  assert.match(source, /纠错建议/);
});

test("学习建议 category 应支持完整词库分类并保留旧分类兼容", async () => {
  const source = await readFile("src/components/learning/VocabularyLearningToast.tsx", "utf8");
  const typeSource = await readFile("src/types/index.ts", "utf8");

  assert.match(typeSource, /export type LearningSuggestionCategory =/);
  assert.match(typeSource, /\|\s*DictionaryCategory/);
  assert.match(typeSource, /\|\s*"proper_noun"/);
  assert.match(source, /code_symbol:\s*"代码"/);
  assert.match(source, /domain_term:\s*"术语"/);
  assert.match(source, /proper_noun:\s*"专有名词"/);
});

test("纠错建议通知去重应包含原文和修正文", async () => {
  const source = await readFile("src/windows/NotificationWindow.tsx", "utf8");

  assert.match(source, /s\.word === suggestion\.word/);
  assert.match(source, /s\.original === suggestion\.original/);
  assert.match(source, /s\.corrected === suggestion\.corrected/);
});
