import type { AppPage } from "../../pages/types";
import type { UpdateStatus } from "../../types";
import {
  GraduationCap,
  History,
  Keyboard,
  LayoutDashboard,
  Mic,
  PanelLeftClose,
  PanelLeftOpen,
  SlidersHorizontal,
  Sparkles,
  MessageSquare,
  HelpCircle,
  Zap,
} from "lucide-react";
import { RedDot } from "../common/RedDot";

export type SidebarProps = {
  collapsed: boolean;
  onToggleCollapsed: () => void;

  activePage: AppPage;
  onNavigate: (page: AppPage) => void;

  updateStatus?: UpdateStatus;
};

export function Sidebar({
  collapsed,
  onToggleCollapsed,
  activePage,
  onNavigate,
  updateStatus,
}: SidebarProps) {
  const containerWidth = collapsed ? "w-[72px]" : "w-60";

  const navTextClass = collapsed ? "hidden" : "block";
  const sectionTitleClass = collapsed ? "hidden" : "block";
  const headerClass = collapsed
    ? "px-2 py-4 mb-4 flex items-center justify-center"
    : "px-3 py-5 mb-4 flex items-center gap-3";

  const navItemBase = [
    "w-full flex items-center gap-3 px-3 py-2 text-sm font-medium",
    "rounded-xl transition-colors",
    collapsed ? "justify-center px-0" : "",
  ].join(" ");

  const navItem = (page: AppPage) =>
    [
      navItemBase,
      activePage === page
        ? "bg-[var(--paper)] text-[var(--crail)] border border-[var(--stone)] shadow-sm"
        : "text-[var(--stone-dark)] hover:bg-[var(--paper)] hover:text-[var(--crail)]",
    ].join(" ");

  return (
    <aside
      className={[
        "shrink-0 h-screen bg-[var(--panel)] border-r border-[var(--stone)]",
        "flex flex-col p-4 z-30 transition-[width] duration-200 ease-in-out font-sans",
        containerWidth,
      ].join(" ")}
    >
      <div className={headerClass}>
        <div className={navTextClass}>
          <p className="text-[10px] text-stone-300 mono uppercase tracking-widest">
            PushToTalk
          </p>
          {/* <div className="text-sm font-bold tracking-tight lowercase">pushtotalk</div> */}
          <div className="text-[11px] text-[var(--stone-dark)] font-semibold">
            AI 语音助手
          </div>
        </div>
      </div>

      <nav className="flex-1 space-y-1">
        <button
          onClick={() => onNavigate("dashboard")}
          className={navItem("dashboard")}
        >
          <LayoutDashboard className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>主页看板</span>
        </button>

        <div
          className={[
            "pt-5 pb-2 px-3 text-[10px] font-bold uppercase tracking-[0.15em]",
            "text-stone-400",
            sectionTitleClass,
          ].join(" ")}
        >
          配置中心
        </div>

        <button
          onClick={() => onNavigate("asr")}
          className={navItem("asr")}
          title="语音识别引擎"
        >
          <Mic className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>语音识别引擎</span>
        </button>

        <button
          onClick={() => onNavigate("models")}
          className={navItem("models")}
          title="LLM 模型配置"
        >
          <Zap className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>LLM 模型配置</span>
        </button>

        <button
          onClick={() => onNavigate("llm")}
          className={navItem("llm")}
          title="语句润色预设"
        >
          <Sparkles className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>语句润色</span>
        </button>

        <button
          onClick={() => onNavigate("assistant")}
          className={navItem("assistant")}
          title="快捷助手"
        >
          <MessageSquare className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>AI 助手</span>
        </button>

        <button
          onClick={() => onNavigate("hotkeys")}
          className={navItem("hotkeys")}
          title="快捷键"
        >
          <Keyboard className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>快捷键</span>
        </button>

        <button
          onClick={() => onNavigate("dictionary")}
          className={navItem("dictionary")}
          title="词库"
        >
          <GraduationCap className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>词库</span>
        </button>

        <div
          className={[
            "pt-5 pb-2 px-3 text-[10px] font-bold uppercase tracking-[0.15em]",
            "text-stone-400",
            sectionTitleClass,
          ].join(" ")}
        >
          偏好
        </div>

        <button
          onClick={() => onNavigate("preferences")}
          className={navItem("preferences")}
          title="偏好设置"
        >
          <div className="relative">
            <SlidersHorizontal className="shrink-0 w-5 h-5" />
            {updateStatus === "available" && (
              <div className="absolute -top-0.5 -right-0.5">
                <RedDot size="sm" />
              </div>
            )}
          </div>
          <span className={navTextClass}>偏好设置</span>
        </button>

        <button
          onClick={() => onNavigate("help")}
          className={navItem("help")}
          title="帮助与支持"
        >
          <HelpCircle className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>帮助与支持</span>
        </button>

        <div
          className={[
            "pt-5 pb-2 px-3 text-[10px] font-bold uppercase tracking-[0.15em]",
            "text-stone-400",
            sectionTitleClass,
          ].join(" ")}
        >
          记录
        </div>

        <button
          onClick={() => onNavigate("history")}
          className={navItem("history")}
          title="历史记录"
        >
          <History className="shrink-0 w-5 h-5" />
          <span className={navTextClass}>历史记录</span>
        </button>
      </nav>

      <div className="pt-3 border-t border-[var(--stone)]">
        <button
          onClick={onToggleCollapsed}
          className={[
            "w-full flex items-center gap-3 px-3 py-2 text-sm font-bold rounded-xl transition-colors",
            collapsed ? "justify-center px-0 text-[var(--stone-dark)]" : "text-[var(--stone-dark)]",
            "hover:bg-white/70 hover:text-[var(--ink)]",
          ].join(" ")}
          title={collapsed ? "展开侧栏" : "收起侧栏"}
        >
          {collapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
          <span className={navTextClass}>{collapsed ? "" : "收起侧栏"}</span>
        </button>
      </div>
    </aside>
  );
}
