# ASR Eval Baseline Report

## Suite

- Suite: `tests/asr_eval`
- Cases: 5
- Seed correction pairs: `tests/asr_eval/correction_pairs.json`
- Scope: Week 1 Claude Code MVP vertical slice

## Metrics

| Metric | Value |
|---|---:|
| final_accuracy | 100.00% |
| correction_pair_hit_rate | 80.00% |
| false_replacement_count | 0 |
| avg_latency_ms | 0.114 |
| p95_latency_ms | 0.155 |

## Cases

| ID | Category | Result | Raw | Actual |
|---|---|---|---|---|
| `mvp-claude-code-001` | `tech_mix` | PASS | `我打开 cloud code` | `我打开 Claude Code` |
| `mvp-claude-code-002` | `tech_mix` | PASS | `我打开 claud code` | `我打开 Claude Code` |
| `mvp-claude-code-003` | `tech_mix` | PASS | `我打开 cloud coat` | `我打开 Claude Code` |
| `mvp-claude-code-004` | `tech_mix` | PASS | `我打开 克劳德 code` | `我打开 Claude Code` |
| `mvp-claude-code-005` | `false_positive_guard` | PASS | `I use cloud storage` | `I use cloud storage` |

## Notes

- This is not a full ASR quality baseline yet. It verifies the local personalization decoder MVP over fixed text fixtures.
- The false-positive guard confirms that a `cloud code -> Claude Code` pair does not generalize to `cloud storage`.
- Latency metrics are from a local sample run and cover only `PersonalizationEngine::convert`, not ASR provider time or LLM processing.
