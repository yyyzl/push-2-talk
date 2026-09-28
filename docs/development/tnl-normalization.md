# TNL 与个性化纠错

本文记录当前运行契约。历史阶段计划见 [归档](../archive/asr-personalization-plan-2026-05.md)，原详细阶段记录见 [历史快照](../archive/tnl-normalization-2026-05.md)，二者不作为当前实现要求。

## 运行流程

ASR 输出先经过 TNL 文本归一化，再按配置运行个性化纠错；低置信候选可交给已有 LLM 仲裁。普通听写和助手共用确定性处理规则，选区读取、热键和输入通过平台接口处理。

- `tnl/engine.rs`：词典分类路由、短语预处理、技术片段保护及候选生成。
- `tnl/tech_span.rs`：文件、代码、链接等片段检测，以及 Jieba/用户词典专名检测。
- `tnl/syllable_lattice.rs`：生成有限窗口的读音键，供匹配复用。
- `personalization/correction_pair_store.rs`：纠错对与接受、拒绝、撤销反馈；保存须原子化并串行化修改。
- `personalization/engine.rs`、`convert_pipeline.rs`：精确匹配和发音/别名匹配两个固定 pass，按分数与重叠情况选取候选。
- `personalization/hotword_compiler.rs`：仅负责在用的 ASR 热词编译；不再提供无运行消费者的 TNL/LLM pack。

这些处理消费识别后的文本，不重新识别音频。ASR 热词协议另见 [热词编译](asr-hotword-compilation.md)。

## 口语清理

`DisfluencyMode` 的 `off`、`conservative`、`aggressive` 三种存储值继续可读、可往返保存。界面只提供关闭与保守；旧 `aggressive` 按保守规则运行，不自动改写旧字段。

- 关闭：完全保留输入。
- 保守：仅移除后面有软停顿（逗号或横向空格）且仍有正文的句首“嗯、呃”。
- 不清除“这个、那个、就是说”等有语义的词；不猜测错误起句；不压缩连续汉字。
- 不跨句号、问号、感叹号或换行删除独立回应；不能把非空回应清空。
- `这个，给小王；那个，给小李。`、`嗯。`、`哈哈哈`、`好好好` 必须保留。

文本规则无法理解所有语境，需要逐字保留的场景选择关闭。旧配置缺少该字段时保持关闭，新安装默认保守。

## 词典与候选

- `word|source|category` 仍兼容旧字符串。统一使用 `dictionary_utils` 解析，禁止把元数据当作待识别词发送。
- `email`、`url` 不走普通近音/连字符改写；`code_symbol` 不走模糊近音替换。分类确实影响运行规则，不能作为无用标签删除。
- `phrase` 使用短语规则，不能跨不允许的标点拼接。
- 精确词典保护优先，不能把用户已经写对的词再次模糊替换。
- 字节范围必须落在 UTF-8 边界；候选不能重叠应用，按倒序替换以避免偏移失效。
- 中英混合窗口不能只凭英文键忽略中文；普通英文词、重复反馈及手工词优先级保护继续保留。
- 当前默认阈值 0.88、窗口 5；不要仅凭固定小样本全过继续增加复杂模型或调整阈值。
- SQLite 用户词典已承担实际读写，JSON 是兼容快照。详见 [数据库约定](database-guidelines.md)。删除未调用查询不意味着删除数据库列或用户数据。

## 配置兼容与上下文热词

旧配置缺少个性化 exact/syllable/纠错热词开关时均为关闭；明确保存的值继续保留。新安装保留保守个性化能力。

`enable_context_hotwords` 对新安装和缺字段的旧配置均默认关闭；明确开启的旧值不重置。偏好设置提供实验性开关，停止服务后调整、下次启动生效。

- 近期历史仅取成功记录、最近 24 小时、最多 20 条，以英文技术词为主。
- 当前应用文本最多分析 4,000 字符、提取 20 条；通过平台读文本接口，不在共享业务层直接依赖 UIA。
- 临时热词不能写入持久化用户词典；关闭时不得生成历史热词或读取应用上下文作为 ASR 热词。
- 热词会随请求发给所选 ASR 服务，界面须明确说明。识别收益与起录延迟仍需真实音频对照；Jieba 和复杂候选增强本轮不再扩展。

## 诊断

正常使用不保存额外个性化诊断文件。仅在启动进程时显式设置 `PUSHTOTALK_PERSONALIZATION_DIAGNOSTICS=1` 才开启；没有该值时甚至不解析诊断目录。

```powershell
# Windows PowerShell，仅对本次启动设置
$env:PUSHTOTALK_PERSONALIZATION_DIAGNOSTICS = "1"
& "C:\path\to\PushToTalk.exe"
Remove-Item Env:PUSHTOTALK_PERSONALIZATION_DIAGNOSTICS
```

```sh
# macOS，直接启动应用内的可执行文件，使本次进程继承该环境变量
PUSHTOTALK_PERSONALIZATION_DIAGNOSTICS=1 /Applications/PushToTalk.app/Contents/MacOS/push-to-talk
```

诊断包含识别文本和候选；单字符串截断到 160 字符，候选最多 20 条。新文件保存在平台配置目录的 `diagnostics/personalization-session/`。写入时清除该目录超过 7 天的本功能文件，总计最多 200 份；停止记录后不运行后台清理任务。保留旧日期目录与其他文件，不主动清除用户历史。写入失败只记警告，不阻止听写。

## 验证

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml --bin eval_asr --no-default-features
cargo run --manifest-path src-tauri/Cargo.toml --bin eval_asr --no-default-features -- --disable-syllable-match-pass --allow-quality-gate-failure
npx tsx scripts/asr-eval-readiness.ts --allow-not-ready --json
```

`tests/asr_eval/cases/` 是本地文本回归，新增了正常指代、回应、笑声和强调表达保护。评测只跑文本清理与个性化引擎，不代表完整 ASR、Jieba、LLM 或桌面输入验收，更不能称为真实语音准确率。

`scripts/asr-eval-draft.ts` 从历史或诊断生成待复核草稿，只允许人工确认的 expected_text 进入正式集；`scripts/asr-eval-readiness.ts` 检查数量，不能替代代表性与质量验证。真实录音不足时继续收集已复核样本，不启动神经 reranker 等下一阶段扩展。

评测的各类命中率只作为样本构成统计，不设“至少修改 70% 文本”的门槛。质量门槛继续要求全量文本精确正确、误替换率不超过 1%、本地 P95 不超过 30ms、无未决候选；增加保留原文用例不会因无需替换而导致失败。
