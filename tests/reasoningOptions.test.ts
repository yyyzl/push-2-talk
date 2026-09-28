import assert from "node:assert/strict";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ReasoningEffortSelect } from "../src/components/llm/ReasoningEffortSelect";
import { DEFAULT_LLM_CONFIG } from "../src/constants";

test("能力尚未加载时保留旧值，不展示未经确认的选项或触发保存", () => {
  let changes = 0;
  const html = renderToStaticMarkup(createElement(ReasoningEffortSelect, {
    value: "xhigh",
    context: { kind: "polishing", config: DEFAULT_LLM_CONFIG },
    onChange: () => { changes += 1; },
  }));
  assert.equal((html.match(/<option /g) ?? []).length, 2);
  assert.match(html, /value="xhigh"[^>]*selected/);
  assert.doesNotMatch(html, /value="low"/);
  assert.equal(changes, 0);
});

test("切换模型后旧值仍可见，只能选后端提供的新值", async () => {
  const { reasoningSelectOptions } = await import("../src/utils/reasoningOptions");
  const capabilities = { model: "qwen3-max", efforts: ["default", "none", "auto"] as const, legacy_hint: null };
  const options = reasoningSelectOptions("high", { ...capabilities, efforts: [...capabilities.efforts] });
  assert.deepEqual(options.filter((option) => !option.disabled).map((option) => option.value), ["default", "none", "auto"]);
  assert.equal(options.find((option) => option.value === "high")?.disabled, true);
  assert.equal(reasoningSelectOptions(undefined).length, 1);
});
