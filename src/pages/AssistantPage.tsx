import type { Dispatch, SetStateAction } from "react";
import { AlertCircle } from "lucide-react";
import type { AssistantConfig } from "../types";
import { ApiKeyInput } from "../components/common";

export type AssistantPageProps = {
  assistantConfig: AssistantConfig;
  setAssistantConfig: Dispatch<SetStateAction<AssistantConfig>>;
  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;
  isRunning: boolean;
};

export function AssistantPage({
  assistantConfig,
  setAssistantConfig,
  showApiKey,
  setShowApiKey,
  isRunning,
}: AssistantPageProps) {
  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-6">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <span>AI 助手</span>
        </div>

        <div className="flex items-center gap-2 p-3 bg-[rgba(120,140,93,0.12)] border border-[rgba(120,140,93,0.22)] rounded-xl text-xs text-[var(--ink)]">
          <AlertCircle size={14} className="flex-shrink-0 text-[var(--sage)]" />
          <span>AI 助手无需开关：按下热键即可处理选中文本或回答问题。</span>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">连接信息</h4>
          <div className="space-y-3 p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
            <div className="space-y-2">
              <label className="text-xs font-bold text-stone-500">API Key</label>
              <ApiKeyInput
                value={assistantConfig.api_key}
                onChange={(value) => setAssistantConfig((prev) => ({ ...prev, api_key: value }))}
                show={showApiKey}
                onToggleShow={() => setShowApiKey(!showApiKey)}
                placeholder="sk-..."
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">模型名称</label>
                <input
                  type="text"
                  value={assistantConfig.model}
                  disabled={isRunning}
                  onChange={(e) => setAssistantConfig((prev) => ({ ...prev, model: e.target.value }))}
                  className="w-full px-3 py-2.5 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                  placeholder="glm-4-flash"
                />
              </div>
              <div className="space-y-2">
                <label className="text-xs font-bold text-stone-500">API 地址</label>
                <input
                  type="text"
                  value={assistantConfig.endpoint}
                  disabled={isRunning}
                  onChange={(e) => setAssistantConfig((prev) => ({ ...prev, endpoint: e.target.value }))}
                  className="w-full px-3 py-2.5 bg-white border border-[var(--stone)] rounded-xl text-sm focus:outline-none focus:border-[var(--steel)] disabled:opacity-60"
                  placeholder="https://api..."
                />
              </div>
            </div>
          </div>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">问答模式提示词</h4>
          <p className="text-xs text-stone-500">无选中文本时，用于回答问题。</p>
          <textarea
            value={assistantConfig.qa_system_prompt}
            disabled={isRunning}
            onChange={(e) => setAssistantConfig((prev) => ({ ...prev, qa_system_prompt: e.target.value }))}
            className="w-full min-h-[140px] p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl text-sm focus:outline-none focus:border-[var(--steel)] resize-none mono text-stone-700 leading-relaxed disabled:opacity-60"
            placeholder="定义 AI 助手如何回答问题..."
          />
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">文本处理提示词</h4>
          <p className="text-xs text-stone-500">有选中文本时，用于翻译、润色、总结等。</p>
          <textarea
            value={assistantConfig.text_processing_system_prompt}
            disabled={isRunning}
            onChange={(e) =>
              setAssistantConfig((prev) => ({ ...prev, text_processing_system_prompt: e.target.value }))
            }
            className="w-full min-h-[140px] p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl text-sm focus:outline-none focus:border-[var(--steel)] resize-none mono text-stone-700 leading-relaxed disabled:opacity-60"
            placeholder="定义 AI 助手如何处理选中的文本..."
          />
        </div>
      </div>
    </div>
  );
}
