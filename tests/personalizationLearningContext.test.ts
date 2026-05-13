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
