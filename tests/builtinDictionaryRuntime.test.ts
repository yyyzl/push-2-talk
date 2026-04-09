import assert from "node:assert/strict";
import test from "node:test";
import {
  setBuiltinDomainsSnapshot,
  getBuiltinWordsForDomains,
  normalizeBuiltinDictionaryDomains,
  BUILTIN_DICTIONARY_DOMAINS,
  BUILTIN_DICTIONARY_LIMIT,
} from "../src/utils/builtinDictionary";

test("runtime snapshot 更新后，取词结果应变化", () => {
  setBuiltinDomainsSnapshot([{ name: "AI", words: ["GPT"] }]);
  assert.deepEqual(getBuiltinWordsForDomains(["AI"]), ["GPT"]);

  setBuiltinDomainsSnapshot([{ name: "AI", words: ["Claude"] }]);
  assert.deepEqual(getBuiltinWordsForDomains(["AI"]), ["Claude"]);
});

test("normalize 应剔除 snapshot 不存在的领域", () => {
  setBuiltinDomainsSnapshot([{ name: "AI", words: ["GPT"] }]);
  assert.deepEqual(normalizeBuiltinDictionaryDomains(["AI", "不存在"]), ["AI"]);
});

test("snapshot 为空时 normalize 应保留已保存领域，等待远端词库加载", () => {
  setBuiltinDomainsSnapshot([]);
  assert.deepEqual(normalizeBuiltinDictionaryDomains(["AI", "不存在"]), ["AI", "不存在"]);
});

test("BUILTIN_DICTIONARY_DOMAINS 向后兼容导出应反映当前 snapshot", () => {
  setBuiltinDomainsSnapshot([{ name: "测试", words: ["A"] }]);
  assert.equal(BUILTIN_DICTIONARY_DOMAINS.length, 1);
  assert.equal(BUILTIN_DICTIONARY_DOMAINS[0].name, "测试");
});

test("BUILTIN_DICTIONARY_LIMIT 应保留导出", () => {
  assert.equal(typeof BUILTIN_DICTIONARY_LIMIT, "number");
  assert.equal(BUILTIN_DICTIONARY_LIMIT, 5);
});
