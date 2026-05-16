# 继续 ASR 当前 App 上下文热词来源

## 背景

第 0~3 阶段已经让 ASR 文本进入本地个性化二次解码闭环，Phase 7 的热词编译器也已经支持 `recent` 与 `app_context` 的来源排序。当前缺口是：录音开始时，ASR 还没有利用用户正在操作的前台应用文本作为短期热词来源。

## 目标

在每次录音开始前，从触发热键时的目标窗口读取一小段 UIA 文本，保守提取技术名词、产品名、代码符号等候选词，以 `word|app_context|category` 的 runtime-only 形式追加到本次 ASR dictionary snapshot 中，让云端 ASR 请求能更容易命中当前上下文中的专有词。

## 需求

- 使用触发热键时已经捕获的目标窗口句柄，不重新猜测目标窗口。
- 使用现有 `uia_text_reader::get_focused_window_text`，复用其 COM、超时、黑名单和并发保护。
- 提取逻辑必须保守，优先英文/代码/产品类词，不把普通长文本大量塞进热词。
- 上下文热词只参与本次运行时 ASR dictionary，不写入 `config.json`，不改变用户词库。
- UIA 读取失败、窗口无效、文本为空或候选词为空时，录音必须照常开始。
- 不改变 provider payload shape，仍由既有 `HotwordCompiler` 统一去 metadata、排序、去重和截断。

## 验收标准

- 录音开始路径会在 `handle_recording_start` 前追加当前 App 上下文热词。
- `Claude Code`、`GPT-5.3-Codex`、`ASR_PERSONALIZATION_QUALITY_LEAP.md` 这类上下文词可被提取为 `app_context` entries。
- 普通无技术含义文本不会产生大量候选词。
- 单次追加数量有上限，输入文本长度有上限。
- 相关 Rust 单测覆盖提取、格式、去重/上限、runtime 字典追加行为。
- 运行目标后端测试和 `cargo check`。

## 非目标

- 不做 OCR、浏览器 DOM 读取或编辑器插件集成。
- 不把上下文热词展示到前端词库。
- 不把上下文热词持久化或自动学习成用户词条。
- 不改变 UIA 文本读取模块的 timeout 策略。
