import {
  Children,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import {
  oneLight,
  oneDark,
} from "react-syntax-highlighter/dist/esm/styles/prism";
import type { SearchCitation } from "../types/assistant-result";

interface MarkdownRendererProps {
  content: string;
  className?: string;
  darkMode?: boolean;
  citations?: SearchCitation[];
}

const CITATION_HREF_PATTERN = /^(\d+):([a-z0-9]+)$/i;
const RAW_CITATION_PATTERN = /\[citation\]\((\d+):([a-z0-9]+)\)/gi;
const CITATION_TOKEN_PATTERN = /@@P2T_CITATION:(\d+):([a-z0-9]+)@@/gi;
const CITATION_TOOLTIP_MARGIN = 12;
const CITATION_TOOLTIP_GAP = 8;

function prepareCitationMarkdown(content: string): string {
  RAW_CITATION_PATTERN.lastIndex = 0;
  return content.replace(
    RAW_CITATION_PATTERN,
    (_, index: string, id: string) => `@@P2T_CITATION:${index}:${id}@@`,
  );
}

function findCitation(
  citations: SearchCitation[] | undefined,
  index: number,
  id: string,
) {
  return citations?.find((item) => item.index === index && item.id === id);
}

function CitationChip({
  index,
  id,
  citations,
  darkMode,
}: {
  index: number;
  id: string;
  citations?: SearchCitation[];
  darkMode: boolean;
}) {
  const citation = findCitation(citations, index, id);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const tooltipRef = useRef<HTMLSpanElement>(null);
  const [tooltipVisible, setTooltipVisible] = useState(false);
  const [anchorRect, setAnchorRect] = useState<DOMRectReadOnly | null>(null);
  const [tooltipStyle, setTooltipStyle] = useState<CSSProperties>({
    left: 0,
    top: 0,
    visibility: "hidden",
  });
  const tooltipId = citation ? `citation-tooltip-${index}-${id}` : undefined;

  useLayoutEffect(() => {
    if (!tooltipVisible || !anchorRect || !tooltipRef.current) return;

    const tooltipRect = tooltipRef.current.getBoundingClientRect();
    const maxLeft = window.innerWidth - tooltipRect.width - CITATION_TOOLTIP_MARGIN;
    const preferredLeft =
      anchorRect.left + anchorRect.width / 2 - tooltipRect.width / 2;
    const left = Math.min(
      Math.max(preferredLeft, CITATION_TOOLTIP_MARGIN),
      Math.max(CITATION_TOOLTIP_MARGIN, maxLeft),
    );

    let top = anchorRect.top - tooltipRect.height - CITATION_TOOLTIP_GAP;
    if (top < CITATION_TOOLTIP_MARGIN) {
      top = anchorRect.bottom + CITATION_TOOLTIP_GAP;
    }
    if (top + tooltipRect.height > window.innerHeight - CITATION_TOOLTIP_MARGIN) {
      top = Math.min(
        Math.max(
          window.innerHeight - tooltipRect.height - CITATION_TOOLTIP_MARGIN,
          CITATION_TOOLTIP_MARGIN,
        ),
        window.innerHeight - CITATION_TOOLTIP_MARGIN,
      );
    }

    setTooltipStyle({
      left,
      top,
      visibility: "visible",
    });
  }, [anchorRect, tooltipVisible]);

  const showTooltip = () => {
    if (!citation || !buttonRef.current) return;
    setAnchorRect(buttonRef.current.getBoundingClientRect());
    setTooltipVisible(true);
  };

  const hideTooltip = () => {
    setTooltipVisible(false);
    setTooltipStyle((current) => ({
      ...current,
      visibility: "hidden",
    }));
  };

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        aria-describedby={tooltipVisible ? tooltipId : undefined}
        aria-label={citation ? `打开引用来源 ${index}: ${citation.title}` : `引用 ${index}`}
        className="mx-0.5 inline-flex align-super text-[0.68em] font-semibold"
        style={{
          appearance: "none",
          background: "transparent",
          border: "none",
          padding: "0 1px",
          color: darkMode ? "#9CA3AF" : "var(--steel)",
          cursor: citation ? "pointer" : "default",
          fontFamily: "var(--font-sans)",
          lineHeight: 1,
          textDecoration: citation ? "underline" : "none",
          textUnderlineOffset: "2px",
        }}
        data-citation-title={citation?.title ?? ""}
        data-citation-url={citation?.url ?? ""}
        data-citation-snippet={citation?.snippet ?? ""}
        onMouseEnter={showTooltip}
        onMouseLeave={hideTooltip}
        onFocus={showTooltip}
        onBlur={hideTooltip}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          if (citation?.url) {
            void openUrl(citation.url);
          }
        }}
      >
        [{index}]
      </button>
      {citation && tooltipVisible && (
        <span
          id={tooltipId}
          ref={tooltipRef}
          role="tooltip"
          style={{
            position: "fixed",
            zIndex: 9999,
            width: "min(320px, calc(100vw - 24px))",
            pointerEvents: "none",
            ...tooltipStyle,
            borderRadius: "8px",
            padding: "10px 12px",
            background: darkMode ? "#20201F" : "white",
            color: darkMode ? "#E8E6DC" : "var(--ink)",
            border: darkMode
              ? "1px solid rgba(255,255,255,0.14)"
              : "1px solid rgba(0,0,0,0.12)",
            boxShadow: darkMode
              ? "0 12px 30px rgba(0,0,0,0.35)"
              : "0 12px 28px rgba(32,32,31,0.16)",
            fontFamily: "var(--font-sans)",
            fontSize: "12px",
            fontWeight: 400,
            lineHeight: 1.45,
          }}
        >
          <span className="block font-semibold">{citation.title}</span>
          <span className="mt-1 block truncate text-[11px] opacity-70">
            {citation.url}
          </span>
          <span className="mt-1 block text-[11px] opacity-80">
            {citation.snippet}
          </span>
        </span>
      )}
    </>
  );
}

function transformCitationText(
  text: string,
  citations: SearchCitation[] | undefined,
  darkMode: boolean,
  keyPrefix: string,
): ReactNode {
  CITATION_TOKEN_PATTERN.lastIndex = 0;
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let partIndex = 0;

  while ((match = CITATION_TOKEN_PATTERN.exec(text)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }
    nodes.push(
      <CitationChip
        key={`${keyPrefix}-${partIndex}`}
        index={Number(match[1])}
        id={match[2]}
        citations={citations}
        darkMode={darkMode}
      />,
    );
    lastIndex = match.index + match[0].length;
    partIndex += 1;
  }

  if (lastIndex === 0) return text;
  if (lastIndex < text.length) nodes.push(text.slice(lastIndex));
  return nodes;
}

function transformChildren(
  children: ReactNode,
  citations: SearchCitation[] | undefined,
  darkMode: boolean,
  keyPrefix: string,
): ReactNode {
  return Children.map(children, (child, index) => {
    if (typeof child === "string") {
      return transformCitationText(
        child,
        citations,
        darkMode,
        `${keyPrefix}-${index}`,
      );
    }
    return child;
  });
}

/**
 * 可复用的 Markdown 渲染组件
 *
 * 支持 GFM（表格、任务列表、删除线）+ 代码块语法高亮
 */
export default function MarkdownRenderer({
  content,
  className = "",
  darkMode = false,
  citations,
}: MarkdownRendererProps) {
  return (
    <div className={`markdown-body ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
        // 代码块 / 行内代码
        code({ className: codeClassName, children, ...props }) {
          const match = /language-(\w+)/.exec(codeClassName || "");
          const codeString = String(children).replace(/\n$/, "");

          // 判断是否为代码块（通过 className 中的 language- 前缀）
          if (match) {
            return (
              <SyntaxHighlighter
                style={darkMode ? oneDark : oneLight}
                language={match[1]}
                PreTag="div"
                customStyle={{
                  margin: 0,
                  borderRadius: "6px",
                  fontSize: "13px",
                  background: darkMode ? "#2A2A28" : "#F5F4F0",
                }}
              >
                {codeString}
              </SyntaxHighlighter>
            );
          }

          // 行内代码
          return (
            <code
              className="inline-code"
              style={{
                background: darkMode ? "#2A2A28" : "#F5F4F0",
                padding: "2px 6px",
                borderRadius: "4px",
                fontSize: "0.9em",
                fontFamily: "'JetBrains Mono', monospace",
              }}
              {...props}
            >
              {children}
            </code>
          );
        },

        // 代码块容器
        pre({ children }) {
          return (
            <pre
              style={{
                margin: "12px 0",
                borderRadius: "6px",
                overflow: "auto",
              }}
            >
              {children}
            </pre>
          );
        },

        // 链接
        a({ children, href, ...props }) {
          const citationMatch = href?.match(CITATION_HREF_PATTERN);
          if (citationMatch) {
            return (
              <CitationChip
                index={Number(citationMatch[1])}
                id={citationMatch[2]}
                citations={citations}
                darkMode={darkMode}
              />
            );
          }

          return (
            <a
              href={href}
              style={{ color: "var(--steel)", textDecoration: "underline" }}
              target="_blank"
              rel="noopener noreferrer"
              {...props}
            >
              {transformChildren(children, citations, darkMode, "a")}
            </a>
          );
        },

        // 表格
        table({ children }) {
          return (
            <div style={{ overflowX: "auto", margin: "12px 0" }}>
              <table
                style={{
                  width: "100%",
                  borderCollapse: "collapse",
                  fontSize: "14px",
                }}
              >
                {children}
              </table>
            </div>
          );
        },

        th({ children }) {
          return (
            <th
              style={{
                border: `1px solid ${darkMode ? "#444" : "var(--sand)"}`,
                padding: "8px 12px",
                textAlign: "left",
                fontWeight: 600,
                background: darkMode ? "#2A2A28" : "#F5F4F0",
              }}
            >
              {transformChildren(children, citations, darkMode, "th")}
            </th>
          );
        },

        td({ children }) {
          return (
            <td
              style={{
                border: `1px solid ${darkMode ? "#444" : "var(--sand)"}`,
                padding: "8px 12px",
              }}
            >
              {transformChildren(children, citations, darkMode, "td")}
            </td>
          );
        },

        // 标题层级
        h1({ children }) {
          return (
            <h1
              style={{
                fontSize: "1.5em",
                fontWeight: 700,
                margin: "16px 0 8px",
                lineHeight: 1.3,
              }}
            >
              {transformChildren(children, citations, darkMode, "h1")}
            </h1>
          );
        },
        h2({ children }) {
          return (
            <h2
              style={{
                fontSize: "1.3em",
                fontWeight: 600,
                margin: "14px 0 6px",
                lineHeight: 1.3,
              }}
            >
              {transformChildren(children, citations, darkMode, "h2")}
            </h2>
          );
        },
        h3({ children }) {
          return (
            <h3
              style={{
                fontSize: "1.15em",
                fontWeight: 600,
                margin: "12px 0 4px",
                lineHeight: 1.3,
              }}
            >
              {transformChildren(children, citations, darkMode, "h3")}
            </h3>
          );
        },

        // 段落
        p({ children }) {
          return (
            <p style={{ margin: "8px 0", lineHeight: 1.7 }}>
              {transformChildren(children, citations, darkMode, "p")}
            </p>
          );
        },

        // 列表
        ul({ children }) {
          return (
            <ul style={{ margin: "8px 0", paddingLeft: "24px" }}>{children}</ul>
          );
        },
        ol({ children }) {
          return (
            <ol style={{ margin: "8px 0", paddingLeft: "24px" }}>{children}</ol>
          );
        },
        li({ children }) {
          return (
            <li style={{ margin: "4px 0", lineHeight: 1.6 }}>
              {transformChildren(children, citations, darkMode, "li")}
            </li>
          );
        },

        // 引用块
        blockquote({ children }) {
          return (
            <blockquote
              style={{
                margin: "12px 0",
                padding: "8px 16px",
                borderLeft: `3px solid ${darkMode ? "#555" : "var(--sand)"}`,
                color: darkMode ? "#999" : "var(--stone-dark)",
                background: darkMode ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
              }}
            >
              {transformChildren(children, citations, darkMode, "blockquote")}
            </blockquote>
          );
        },

        // 分割线
        hr() {
          return (
            <hr
              style={{
                margin: "16px 0",
                border: "none",
                borderTop: `1px solid ${darkMode ? "#333" : "var(--sand)"}`,
              }}
            />
          );
        },
        }}
      >
        {prepareCitationMarkdown(content)}
      </ReactMarkdown>
    </div>
  );
}
