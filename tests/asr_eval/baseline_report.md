# ASR Eval Baseline Report

## Suite

- Suite: `tests/asr_eval`
- Cases: 20
- Seed correction pairs: `tests/asr_eval/correction_pairs.json`
- Scope: Week 1 ASR personalization mini eval vertical slice

## Metrics

| Metric | Value |
|---|---:|
| final_accuracy | 100.00% |
| correction_pair_hit_rate | 80.00% |
| false_replacement_rate | 0.00% |
| false_replacement_count | 0 |
| avg_latency_ms | 0.307 |
| p95_latency_ms | 0.453 |
| quality_gate_passed | true |
| candidates_total | 16 |
| applied_candidates | 16 |
| below_threshold_candidates | 0 |
| skipped_overlap_candidates | 0 |
| pending_candidates | 0 |

## Cases

| ID | Category | Result | Candidates | Applied | Raw | Actual |
|---|---|---|---:|---:|---|---|
| `mvp-claude-code-001` | `tech_mix` | PASS | 1 | 1 | `我打开 cloud code` | `我打开 Claude Code` |
| `mvp-claude-code-002` | `tech_mix` | PASS | 1 | 1 | `我打开 claud code` | `我打开 Claude Code` |
| `mvp-claude-code-003` | `tech_mix` | PASS | 1 | 1 | `我打开 cloud coat` | `我打开 Claude Code` |
| `mvp-claude-code-004` | `tech_mix` | PASS | 1 | 1 | `我打开 克劳德 code` | `我打开 Claude Code` |
| `mvp-claude-code-005` | `false_positive_guard` | PASS | 0 | 0 | `I use cloud storage` | `I use cloud storage` |
| `mvp-openai-001` | `tech_mix` | PASS | 1 | 1 | `我调用 open ai 接口` | `我调用 OpenAI 接口` |
| `mvp-openai-002` | `tech_mix` | PASS | 1 | 1 | `我调用 open eye 接口` | `我调用 OpenAI 接口` |
| `mvp-openai-003` | `tech_mix` | PASS | 1 | 1 | `配置 open ai key` | `配置 OpenAI key` |
| `mvp-openai-004` | `tech_mix` | PASS | 1 | 1 | `测试 open eye key` | `测试 OpenAI key` |
| `mvp-openai-005` | `false_positive_guard` | PASS | 0 | 0 | `Please open the settings` | `Please open the settings` |
| `mvp-typescript-001` | `tech_mix` | PASS | 1 | 1 | `安装 type script 依赖` | `安装 TypeScript 依赖` |
| `mvp-typescript-002` | `tech_mix` | PASS | 1 | 1 | `把文件改成 types script` | `把文件改成 TypeScript` |
| `mvp-typescript-003` | `tech_mix` | PASS | 1 | 1 | `type script 类型报错` | `TypeScript 类型报错` |
| `mvp-typescript-004` | `tech_mix` | PASS | 1 | 1 | `检查 types scripts 配置` | `检查 TypeScript 配置` |
| `mvp-typescript-005` | `false_positive_guard` | PASS | 0 | 0 | `Please type carefully` | `Please type carefully` |
| `mvp-windsurf-001` | `tech_mix` | PASS | 1 | 1 | `打开 wind surf` | `打开 Windsurf` |
| `mvp-windsurf-002` | `tech_mix` | PASS | 1 | 1 | `用 wind surf agent 改代码` | `用 Windsurf agent 改代码` |
| `mvp-windsurf-003` | `tech_mix` | PASS | 1 | 1 | `切到 wind surf workspace` | `切到 Windsurf workspace` |
| `mvp-windsurf-004` | `tech_mix` | PASS | 1 | 1 | `wind surf 插件启动了` | `Windsurf 插件启动了` |
| `mvp-windsurf-005` | `false_positive_guard` | PASS | 0 | 0 | `The wind speed changed` | `The wind speed changed` |

## Notes

- This is not a full ASR quality baseline yet. It verifies the local personalization decoder MVP over fixed text fixtures.
- The false-positive guards confirm that phrase-level pairs do not generalize to common words such as `cloud`, `open`, `type`, and `wind`.
- The TypeScript cases verify that conservative plural-suffix normalization lets `types script` and `types scripts` share the `type script` phonetic key.
- Latency metrics are from a local sample run and cover only `PersonalizationEngine::convert`, not ASR provider time or LLM processing.
