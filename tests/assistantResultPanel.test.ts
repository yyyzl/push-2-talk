import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// 测试 1: AssistantResultPayload 类型字段完整性
test("AssistantResultPayload 类型导出且包含必要字段", async () => {
  const mod = await import("../src/types/assistant-result");

  // 验证模块导出了类型相关的工具函数（类型本身在编译时验证）
  assert.equal(typeof mod.truncateText, "function");
  assert.equal(typeof mod.formatDuration, "function");
});

// 测试 2: truncateText 工具函数
test("truncateText: 短文本不截断", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  assert.equal(truncateText("hello", 10), "hello");
});

test("truncateText: 超长文本截断并添加省略号", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  const long = "a".repeat(120);
  const result = truncateText(long, 100);
  assert.equal(result.length, 101); // 100 chars + "…"
  assert.ok(result.endsWith("…"));
});

test("truncateText: 恰好等于限制长度不截断", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  const exact = "a".repeat(100);
  assert.equal(truncateText(exact, 100), exact);
});

test("truncateText: 空字符串返回空", async () => {
  const { truncateText } = await import("../src/types/assistant-result");
  assert.equal(truncateText("", 100), "");
});

// 测试 3: formatDuration 工具函数
test("formatDuration: 毫秒转秒（带一位小数）", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(1200), "1.2s");
});

test("formatDuration: 整秒显示", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(3000), "3.0s");
});

test("formatDuration: 超过 60 秒显示分+秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(65000), "1m 5s");
});

test("formatDuration: 不足 1 秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(500), "0.5s");
});

test("formatDuration: 0 毫秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(0), "0.0s");
});

test("formatDuration: 恰好 60 秒", async () => {
  const { formatDuration } = await import("../src/types/assistant-result");
  assert.equal(formatDuration(60000), "1m 0s");
});

// ==========================================
// Slice 3: ResultPanelWindow 键盘快捷键逻辑
// ==========================================

// getKeyboardAction: 根据按键事件返回应执行的动作
test("getKeyboardAction: Escape → dismiss", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("Escape", false, false), "dismiss");
});

test("getKeyboardAction: Ctrl+C → null (不拦截，保留原生复制)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("c", true, false), null);
});

test("getKeyboardAction: Meta+C → null (不拦截)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("c", false, true), null);
});

test("getKeyboardAction: Enter → null (不再触发粘贴)", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("Enter", false, false), null);
});

test("getKeyboardAction: 其他按键 → null", async () => {
  const { getKeyboardAction } = await import(
    "../src/windows/result-panel-actions"
  );
  assert.equal(getKeyboardAction("a", false, false), null);
  assert.equal(getKeyboardAction("Tab", false, false), null);
  assert.equal(getKeyboardAction("Shift", false, false), null);
});

// ==========================================
// Slice 3 新增: formatConversationForCopy 对话格式化
// ==========================================

test("formatConversationForCopy: 2 轮对话（第 1 轮有选中文本，第 2 轮无）", async () => {
  const { formatConversationForCopy } = await import(
    "../src/types/assistant-result"
  );
  const turns = [
    {
      user_instruction: "翻译这段话",
      selected_text: "Hello world",
      has_selection: true,
      assistant_response: "你好世界",
      asr_time_ms: 500,
      llm_time_ms: 1000,
    },
    {
      user_instruction: "换一种说法",
      selected_text: undefined,
      has_selection: false,
      assistant_response: "世界你好",
      asr_time_ms: 400,
      llm_time_ms: 800,
    },
  ];
  const result = formatConversationForCopy(turns);

  // 包含问答标记
  assert.ok(result.includes("**问**: 翻译这段话"));
  assert.ok(result.includes("**答**: 你好世界"));
  assert.ok(result.includes("**问**: 换一种说法"));
  assert.ok(result.includes("**答**: 世界你好"));

  // 第 1 轮包含选中文本引用
  assert.ok(result.includes("> 选中文本: Hello world"));

  // 第 2 轮不包含选中文本引用
  const secondQuestionIdx = result.indexOf("**问**: 换一种说法");
  const afterSecondQuestion = result.slice(secondQuestionIdx);
  assert.ok(!afterSecondQuestion.includes("> 选中文本:"));

  // 轮次间有分隔线
  assert.ok(result.includes("---"));
});

test("formatConversationForCopy: 单轮对话无分隔线", async () => {
  const { formatConversationForCopy } = await import(
    "../src/types/assistant-result"
  );
  const turns = [
    {
      user_instruction: "今天天气怎样",
      selected_text: undefined,
      has_selection: false,
      assistant_response: "今天晴天",
      asr_time_ms: 300,
      llm_time_ms: 600,
    },
  ];
  const result = formatConversationForCopy(turns);

  assert.ok(result.includes("**问**: 今天天气怎样"));
  assert.ok(result.includes("**答**: 今天晴天"));
  // 单轮无分隔线
  assert.ok(!result.includes("---"));
});

// ==========================================
// formatTimingDisplay: 耗时信息自适应显示
// ==========================================

test("formatTimingDisplay: 语音轮次（asr > 0）显示完整耗时", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(
    formatTimingDisplay(1100, 1200),
    "ASR 1.1s · LLM 1.2s · 总计 2.3s",
  );
});

test("formatTimingDisplay: 文本轮次（asr = 0）只显示 LLM 耗时", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(formatTimingDisplay(0, 1200), "LLM 1.2s");
});

test("formatTimingDisplay: 边界情况（asr = 0, llm = 0）", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(formatTimingDisplay(0, 0), "LLM 0.0s");
});

test("formatTimingDisplay: 有联网搜索耗时时展示搜索段", async () => {
  const { formatTimingDisplay } = await import(
    "../src/types/assistant-result"
  );
  assert.equal(
    formatTimingDisplay(1100, 1200, 2300),
    "ASR 1.1s · 搜索 2.3s · LLM 1.2s · 总计 4.6s",
  );
});

test("findCitationByMarker: 按 index + id 精确匹配引用", async () => {
  const { findCitationByMarker } = await import(
    "../src/types/assistant-result"
  );
  const citations = [
    {
      index: 1,
      id: "abc123def456",
      title: "OpenAI News",
      url: "https://example.com/openai",
      snippet: "snippet",
      source: "example.com",
    },
  ];

  assert.equal(
    findCitationByMarker(citations, 1, "abc123def456")?.url,
    "https://example.com/openai",
  );
  assert.equal(findCitationByMarker(citations, 2, "abc123def456"), null);
  assert.equal(findCitationByMarker(citations, 1, "missing"), null);
});

test("MarkdownRenderer: 搜索引用应预处理为内部标记，避免渲染成 localhost 链接", async () => {
  const source = await readFile("src/components/MarkdownRenderer.tsx", "utf8");

  assert.match(source, /function prepareCitationMarkdown/);
  assert.match(source, /RAW_CITATION_PATTERN/);
  assert.match(source, /CITATION_TOKEN_PATTERN/);
  assert.match(source, /\{prepareCitationMarkdown\(content\)\}/);
  assert.doesNotMatch(source, /\{content\}\s*<\/ReactMarkdown>/);
  assert.match(source, /P2T_CITATION/);
  assert.match(source, /\[\{index\}\]/);
  assert.match(source, /align-super/);
  assert.match(source, /打开引用来源/);
  assert.match(source, /data-citation-title/);
  assert.match(source, /data-citation-url/);
  assert.match(source, /data-citation-snippet/);
  assert.match(source, /useLayoutEffect/);
  assert.match(source, /tooltipRef/);
  assert.match(source, /position: "fixed"/);
  assert.match(source, /pointerEvents: "none"/);
  assert.match(source, /window\.innerWidth/);
  assert.match(source, /window\.innerHeight/);
  assert.match(source, /Math\.min\(\s*Math\.max/);
  assert.doesNotMatch(source, /group-hover:block/);
  assert.doesNotMatch(source, /absolute bottom-full/);
  assert.doesNotMatch(source, />\s*\{index\}\s*\n\s*\{/);
});

test("ResultPanelWindow: 用户卡片应提供可展开的选中文本展示", async () => {
  const source = await readFile("src/windows/ResultPanelWindow.tsx", "utf8");

  assert.match(source, /function SelectedTextPreview/);
  assert.match(source, /选中文本/);
  assert.match(source, /aria-expanded=\{expanded\}/);
  assert.match(source, /setExpanded/);
  assert.match(source, /maxHeight:\s*expanded\s*\?/);
  assert.match(source, /whiteSpace:\s*"pre-wrap"/);
  assert.match(source, /wordBreak:\s*"break-word"/);
});

test("DEFAULT_ASSISTANT_CONFIG: 文本处理提示词应支持基于选区提问", async () => {
  const { DEFAULT_ASSISTANT_CONFIG } = await import("../src/constants");

  assert.match(
    DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt,
    /基于选中文本回答问题/,
  );
  assert.match(DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt, /编辑类任务/);
  assert.match(DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt, /解释/);
  assert.match(DEFAULT_ASSISTANT_CONFIG.text_processing_system_prompt, /分析/);
});

test("resolveInitialWebSearchEnabled: 配置启用且默认搜索引擎可用时默认开启", async () => {
  const { resolveInitialWebSearchEnabled } = await import(
    "../src/utils/searchRuntime"
  );

  assert.equal(
    resolveInitialWebSearchEnabled({
      assistant_config: {
        enabled: true,
        llm: { use_shared: true },
        qa_system_prompt: "qa",
        text_processing_system_prompt: "tp",
        enable_web_search: true,
        web_search_max_loops: 3,
        web_search_in_text_mode: false,
      },
      search_config: {
        providers: [
          {
            id: "default",
            provider_type: "tavily",
            display_name: "Tavily",
            enabled: true,
            endpoint: "https://api.tavily.com/search",
            api_key: "key",
          },
        ],
        default_provider_id: "default",
        max_results: 5,
        timeout_secs: 6,
        enable_fallback: true,
      },
    }),
    true,
  );
});

test("resolveInitialWebSearchEnabled: Tavily/Bocha/Serper 可使用默认 endpoint", async () => {
  const { resolveInitialWebSearchEnabled } = await import(
    "../src/utils/searchRuntime"
  );

  assert.equal(
    resolveInitialWebSearchEnabled({
      assistant_config: {
        enabled: true,
        llm: { use_shared: true },
        qa_system_prompt: "qa",
        text_processing_system_prompt: "tp",
        enable_web_search: true,
        web_search_max_loops: 3,
        web_search_in_text_mode: false,
      },
      search_config: {
        providers: [
          {
            id: "default",
            provider_type: "tavily",
            display_name: "Tavily",
            enabled: true,
            endpoint: "",
            api_key: "key",
          },
        ],
        default_provider_id: "default",
        max_results: 5,
        timeout_secs: 6,
        enable_fallback: true,
      },
    }),
    true,
  );
});

test("resolveInitialWebSearchEnabled: 搜索 API 可运行时不依赖旧 AI 助手开关", async () => {
  const { resolveInitialWebSearchEnabled } = await import(
    "../src/utils/searchRuntime"
  );

  const base = {
    assistant_config: {
      enabled: true,
      llm: { use_shared: true },
      qa_system_prompt: "qa",
      text_processing_system_prompt: "tp",
      enable_web_search: true,
      web_search_max_loops: 3,
      web_search_in_text_mode: false,
    },
    search_config: {
      providers: [
        {
          id: "default",
          provider_type: "tavily" as const,
          display_name: "Tavily",
          enabled: true,
          endpoint: "https://api.tavily.com/search",
          api_key: "",
        },
      ],
      default_provider_id: "default",
      max_results: 5,
      timeout_secs: 6,
      enable_fallback: true,
    },
  };

  assert.equal(resolveInitialWebSearchEnabled(base), false);
  assert.equal(
    resolveInitialWebSearchEnabled({
      ...base,
      assistant_config: {
        ...base.assistant_config,
        enable_web_search: false,
      },
      search_config: {
        ...base.search_config,
        providers: [
          {
            ...base.search_config.providers[0],
            api_key: "key",
          },
        ],
      },
    }),
    true,
  );
});

test("resolveInitialWebSearchEnabled: 默认引擎缺失但存在可用 provider 时默认开启", async () => {
  const { resolveInitialWebSearchEnabled } = await import(
    "../src/utils/searchRuntime"
  );

  assert.equal(
    resolveInitialWebSearchEnabled({
      assistant_config: {
        enabled: true,
        llm: { use_shared: true },
        qa_system_prompt: "qa",
        text_processing_system_prompt: "tp",
        enable_web_search: false,
        web_search_max_loops: 3,
        web_search_in_text_mode: false,
      },
      search_config: {
        providers: [
          {
            id: "usable",
            provider_type: "tavily",
            display_name: "Tavily",
            enabled: true,
            endpoint: "",
            api_key: "key",
          },
        ],
        default_provider_id: null,
        max_results: 5,
        timeout_secs: 6,
        enable_fallback: true,
      },
    }),
    true,
  );
});

test("AssistantBubble: 联网搜索结果展示应克制呈现数量和条目", async () => {
  const source = await readFile("src/windows/ResultPanelWindow.tsx", "utf8");
  const assistantBubbleStart = source.indexOf("function AssistantBubble");
  const loadingBubbleStart = source.indexOf("function LoadingBubble");
  assert.notEqual(assistantBubbleStart, -1);
  assert.notEqual(loadingBubbleStart, -1);
  const assistantBubble = source.slice(assistantBubbleStart, loadingBubbleStart);

  assert.match(source, /function SearchToolCallPanel/);
  assert.match(source, /搜索到 \$\{totalResultCount\} 个结果/);
  assert.match(source, /call\.results\.slice\(0, 3\)\.map/);
  assert.match(source, /className="truncate text-\[11px\] font-medium"/);
  assert.doesNotMatch(assistantBubble, /rgba\(59,130,246/);
});

test("TextInputBar: 底部追问栏控件保持同轴居中", async () => {
  const source = await readFile("src/windows/ResultPanelWindow.tsx", "utf8");
  const start = source.indexOf("function TextInputBar");
  assert.notEqual(start, -1);
  const textInputBar = source.slice(start);

  assert.match(textInputBar, /const INPUT_BAR_CONTROL_HEIGHT = 42/);
  assert.match(textInputBar, /className="flex items-center gap-3 px-6 py-3 shrink-0"/);
  assert.match(textInputBar, /className="flex min-h-\[42px\] min-w-0 flex-1 items-center"/);
  assert.match(textInputBar, /height: `\$\{INPUT_BAR_CONTROL_HEIGHT\}px`/);
  assert.match(textInputBar, /minHeight: `\$\{INPUT_BAR_CONTROL_HEIGHT\}px`/);
  assert.match(textInputBar, /Math\.max\(INPUT_BAR_CONTROL_HEIGHT, Math\.min\(el\.scrollHeight, 84\)\)/);
  assert.match(textInputBar, /paddingTop: "10px"/);
  assert.match(textInputBar, /paddingBottom: "10px"/);
  assert.match(textInputBar, /lineHeight: "20px"/);
});

test("TextInputBar: 文本追问应把联网搜索开关传给后端命令", async () => {
  const source = await readFile("src/windows/ResultPanelWindow.tsx", "utf8");
  const start = source.indexOf("const handleTextSend");
  assert.notEqual(start, -1);
  const handleTextSend = source.slice(start, source.indexOf("// ==========================================", start + 1));

  assert.match(handleTextSend, /invoke\("send_text_question", \{ text, webSearchEnabled \}\)/);
  assert.match(handleTextSend, /\[webSearchEnabled\]/);
});
