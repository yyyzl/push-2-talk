# ASR Eval Baseline Report

## Suite

- Suite: `tests/asr_eval`
- Cases: 22
- Seed correction pairs: `tests/asr_eval/correction_pairs.json`
- Scope: Week 1 ASR personalization mini eval vertical slice

## Metrics

| Metric | Value |
|---|---:|
| final_accuracy | 100.00% |
| correction_pair_hit_rate | 81.82% |
| false_replacement_rate | 0.00% |
| false_replacement_count | 0 |
| avg_latency_ms | 0.364 |
| p95_latency_ms | 0.599 |
| quality_gate_passed | true |
| candidates_total | 19 |
| applied_candidates | 18 |
| below_threshold_candidates | 0 |
| skipped_overlap_candidates | 1 |
| pending_candidates | 0 |
| exact_text_candidates | 9 |
| en_phonetic_candidates | 6 |
| zh_pinyin_fuzzy_candidates | 2 |
| mixed_candidates | 1 |
| alias_candidates | 1 |
| exact_text_applied | 9 |
| en_phonetic_applied | 6 |
| zh_pinyin_fuzzy_applied | 1 |
| mixed_applied | 1 |
| alias_applied | 1 |
| exact_text_pass_enabled_cases | 22 |
| exact_text_pass_disabled_cases | 0 |
| exact_text_pass_candidates | 9 |
| exact_text_pass_applied | 9 |
| exact_text_pass_elapsed_us | 2344 |
| syllable_match_pass_enabled_cases | 22 |
| syllable_match_pass_disabled_cases | 0 |
| syllable_match_pass_candidates | 10 |
| syllable_match_pass_applied | 9 |
| syllable_match_pass_elapsed_us | 275 |

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
| `mvp-openai-zh-001` | `tech_mix` | PASS | 1 | 1 | `我调用 欧盆艾 接口` | `我调用 OpenAI 接口` |
| `mvp-openai-mixed-001` | `tech_mix` | PASS | 2 | 1 | `我调用 欧盆 ai 接口` | `我调用 OpenAI 接口` |

## Notes

- This is not a full ASR quality baseline yet. It verifies the local personalization decoder MVP over fixed text fixtures.
- The false-positive guards confirm that phrase-level pairs do not generalize to common words such as `cloud`, `open`, `type`, and `wind`.
- The TypeScript cases verify that conservative plural-suffix normalization lets `types script` and `types scripts` share the `type script` phonetic key.
- Match-kind metrics show the current mini eval is covered by exact text, English phonetic, Chinese fuzzy-pinyin, mixed-key, and alias hits.
- Pass summary metrics show `exact_text` and `syllable_match` contribution separately; elapsed values are local sample timings and should be compared directionally.
- Tuning command: `cargo run --bin eval_asr --no-default-features -- --apply-threshold 0.88 --max-window-tokens 5`.
- Threshold stress command: `cargo run --bin eval_asr --no-default-features -- --apply-threshold 0.99 --allow-quality-gate-failure`.
- Sweep command: `cargo run --bin eval_asr --no-default-features -- --sweep-thresholds 0.88,0.99 --sweep-window-tokens 3,5 --allow-quality-gate-failure`.
- Sample sweep: threshold `0.88` passes for window `3` and `5`; threshold `0.99` drops to 13/22 with 10 below-threshold candidates, useful for inspecting conservative cutoff behavior.
- Ablation command: `cargo run --bin eval_asr --no-default-features -- --disable-syllable-match-pass --allow-quality-gate-failure`.
- With syllable matching disabled, the sample run passes 13/22 cases with 9 exact-text applications only; the remaining 9 fixes come from English phonetic, Chinese fuzzy-pinyin, mixed-key, and alias paths.
- Diagnostics export schema v2 includes per-case pass summaries for `exact_text` and `syllable_match`.
- Mixed-language correction pairs do not participate in pure-ASCII English phonetic lookup; this prevents an `欧喷 ai -> OpenAI` pair from rewriting an unrelated ASCII `ai` span.
- Latency metrics are from a local sample run and cover only `PersonalizationEngine::convert`, not ASR provider time or LLM processing.
