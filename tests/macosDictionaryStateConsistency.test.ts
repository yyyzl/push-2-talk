import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const readSource = (path: string) => readFile(path, "utf8");

test("P2: macOS 平台应提供 auto 词条过滤工具并在词典链路复用", async () => {
  const utilsSource = await readSource("src/utils/dictionaryUtils.ts");
  const controllerSource = await readSource("src/hooks/useAppServiceController.ts");
  const dictionaryHookSource = await readSource("src/hooks/useDictionary.ts");
  const listenersSource = await readSource("src/hooks/useTauriEventListeners.ts");

  assert.match(utilsSource, /export function filterDictionaryEntriesByAutoLearning\(/);
  assert.match(utilsSource, /entry\.source !== "auto"/);

  assert.match(controllerSource, /filterDictionaryEntriesByAutoLearning\(/);
  assert.match(dictionaryHookSource, /filterDictionaryEntriesByAutoLearning\(/);
  assert.match(listenersSource, /filterDictionaryEntriesByAutoLearning\(/);
});

test("P3: macOS 听写提示组合键文案应为 Option\+Cmd", async () => {
  const platformHook = await readSource("src/hooks/usePlatform.ts");

  assert.match(platformHook, /export const modifierKey = isMacos \? "Option" : "Ctrl";/);
});
