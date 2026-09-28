# ASR Phase 7 Gate 评测报告

> 2026-05-18 更新：本文件主体保留 2026-05-17 的 Phase 7 历史快照。当 Phase 4 disfluency mini cases 接入后，当前 `tests/asr_eval` 已扩展到 32 条；默认 Conservative eval 运行 32/32 通过，readiness gate 为 32/80、缺 48 条。Phase 8 决策仍然不变：不要基于 mini suite 启动本地 reranker。

## 评测范围

- 日期：2026-05-17
- Suite：`tests/asr_eval`
- Case 数：26
- Seed correction pairs: `tests/asr_eval/correction_pairs.json`
- Scope: Phase 7 后的 mini gate，用于判断是否有足够证据启动 Phase 8 本地 reranker。

## 执行命令

```powershell
cd src-tauri
cargo run --bin eval_asr --no-default-features
cargo run --bin eval_asr --no-default-features -- --diagnostics-out target/asr_eval_phase7_gate
cargo run --bin eval_asr --no-default-features -- --sweep-thresholds 0.70,0.88,0.99 --sweep-window-tokens 3,5 --allow-quality-gate-failure
cargo run --bin eval_asr --no-default-features -- --disable-syllable-match-pass --allow-quality-gate-failure
```

## 默认门槛

| 指标 | 值 |
|---|---:|
| final_accuracy | 100.00% |
| passed | 26/26 |
| correction_pair_hit_rate | 73.08% |
| exact_text_hit_rate | 34.62% |
| syllable_match_hit_rate | 38.46% |
| false_replacement_rate | 0.00% |
| false_replacement_count | 0 |
| avg_latency_ms | 0.653 |
| p95_latency_ms | 1.110 |
| quality_gate_passed | true |
| candidates_total | 22 |
| applied_candidates | 19 |
| below_threshold_candidates | 2 |
| skipped_overlap_candidates | 1 |
| pending_candidates | 0 |
| exact_text_candidates | 10 |
| en_phonetic_candidates | 6 |
| zh_pinyin_fuzzy_candidates | 2 |
| mixed_candidates | 0 |
| alias_candidates | 4 |
| exact_text_applied | 9 |
| en_phonetic_applied | 6 |
| zh_pinyin_fuzzy_applied | 1 |
| mixed_applied | 0 |
| alias_applied | 3 |

Diagnostics 输出路径：

```text
src-tauri/target/asr_eval_phase7_gate/asr_eval_diagnostics.json
```

Diagnostics 文件已成功解析：

| 字段 | 值 |
|---|---:|
| schema_version | 4 |
| cases | 26 |
| final_accuracy | 1.0 |
| quality_gate_passed | true |
| apply_threshold | 0.88 |
| max_window_tokens | 5 |

## 参数 Sweep

| Threshold | WindowTokens | Passed | Accuracy | HitRate | ExactHit | SyllableHit | FalseReplacement | P95(ms) | Applied | BelowThreshold | QualityGate |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 0.70 | 3 | 24/26 | 92.31% | 65.38% | 34.62% | 30.77% | 0.00% | 0.337 | 17 | 2 | FAIL |
| 0.70 | 5 | 26/26 | 100.00% | 73.08% | 34.62% | 38.46% | 0.00% | 0.648 | 19 | 2 | PASS |
| 0.88 | 3 | 24/26 | 92.31% | 65.38% | 34.62% | 30.77% | 0.00% | 0.705 | 17 | 2 | FAIL |
| 0.88 | 5 | 26/26 | 100.00% | 73.08% | 34.62% | 38.46% | 0.00% | 0.828 | 19 | 2 | PASS |
| 0.99 | 3 | 16/26 | 61.54% | 34.62% | 34.62% | 0.00% | 0.00% | 0.759 | 9 | 11 | FAIL |
| 0.99 | 5 | 16/26 | 61.54% | 34.62% | 34.62% | 0.00% | 0.00% | 1.143 | 9 | 13 | FAIL |

## Syllable-Match 消融

使用 `--disable-syllable-match-pass --allow-quality-gate-failure`：

| 指标 | 值 |
|---|---:|
| final_accuracy | 61.54% |
| passed | 16/26 |
| correction_pair_hit_rate | 34.62% |
| exact_text_hit_rate | 34.62% |
| syllable_match_hit_rate | 0.00% |
| false_replacement_rate | 0.00% |
| p95_latency_ms | 0.626 |
| quality_gate_passed | false |
| candidates_total | 10 |
| applied_candidates | 9 |

这个消融保留 exact-text correction，禁用 English phonetic、Chinese fuzzy-pinyin、mixed 和 alias 候选生成。结果从 26/26 退到 16/26，说明当前 mini suite 中有 10 条通过来自本地 syllable/window second-decoding 路径。

## 决策

不要仅基于这次 mini gate 启动 Phase 8 本地 reranker。

理由：

- 当前可复现 mini suite 在 Phase 7 后没有残留失败 case。
- 默认运行没有误替换，也没有 pending candidate。
- 消融里的质量差距已经能由现有本地 second-decoding 路径解释，不是缺少神经 reranker 暴露出的缺口。
- 只有当更大的 80-120 条真实 ASR suite 显示当前 exact / phonetic / fuzzy / alias passes 覆盖不了的残留失败时，Phase 8 才有足够价值。

## 限制

- 本报告使用固定文本 fixture，不是新的 provider audio transcript。
- 历史快照的 suite 是 26 条 case；当前 suite 已扩展到 32 条，但仍是 mini gate，不是完整 Phase 0B 正式评测。
- 延迟值来自本地样本运行，只适合方向性比较。
