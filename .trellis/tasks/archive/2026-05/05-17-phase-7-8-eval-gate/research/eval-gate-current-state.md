# Phase 7 to Phase 8 eval gate current state

## 现有评测能力

`src-tauri/src/bin/eval_asr.rs` 已支持：

* `--suite <dir>`：指定评测 suite。
* `--diagnostics-out <dir>`：输出 `asr_eval_diagnostics.json`。
* `--sweep-thresholds` + `--sweep-window-tokens`：对比阈值和窗口参数。
* `--disable-exact-text-pass` / `--disable-syllable-match-pass`：做 pass ablation。
* `--allow-quality-gate-failure`：允许失败场景仍完整输出报告。

## 当前样本范围

`tests/asr_eval/cases/` 目前有 2 个 fixture 文件，共 26 条 case：

* `claude_code_mvp.json`：8 条。
* `tech_terms_mini.json`：18 条。

这套数据适合作为可复现 mini gate，但不能代表 80-120 条真实 daily ASR 样本的正式 Phase 0B 评测。

## 已有 baseline

`tests/asr_eval/baseline_report.md` 记录 Week 1 mini eval：

* final_accuracy: 100.00%
* false_replacement_rate: 0.00%
* avg_latency_ms: 0.538
* p95_latency_ms: 0.943
* syllable match disabled 时通过 16/26，说明 second-decoding 路径贡献 10 条修正。

## 本轮建议

先跑 current head 的默认 suite、diagnostics、sweep 和 syllable ablation，并新增独立 Phase 7 gate 报告。若 current head 在 mini suite 仍为 26/26 且无 pending candidate，则 Phase 8 暂不启动；后续真正的门槛是补充真实样本集后重跑。
