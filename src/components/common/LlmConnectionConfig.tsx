import type { SharedLlmConfig } from "../../types";
import { ArrowUpRight, Cpu, PlugZap } from "lucide-react";

export type LlmConnectionConfigProps = {
  sharedConfig: SharedLlmConfig;
  featureName: "polishing" | "assistant" | "learning";
  onNavigateToModels?: () => void;
};

export function LlmConnectionConfig({
  sharedConfig,
  featureName,
  onNavigateToModels,
}: LlmConnectionConfigProps) {
  const providerIdKey = `${featureName}_provider_id` as keyof SharedLlmConfig;
  const currentProviderId = (sharedConfig[providerIdKey] as string) || sharedConfig.default_provider_id;
  const currentProvider = sharedConfig.providers.find((p) => p.id === currentProviderId);

  // 如果没有找到明确指定的 provider（可能是 "跟随默认" 但默认也没设置），尝试取第一个
  const displayProvider = currentProvider || sharedConfig.providers[0];

  const isFollowDefault = !sharedConfig[providerIdKey];

  if (!displayProvider) {
    return (
      <div className="group relative overflow-hidden rounded-2xl border border-dashed border-[var(--stone)] bg-[var(--paper)]/50 p-6 text-center transition-all hover:border-[var(--steel)] hover:bg-[var(--paper)]">
        <div className="flex flex-col items-center justify-center gap-3">
          <div className="rounded-xl bg-stone-100 p-3 text-stone-400 group-hover:bg-white group-hover:text-[var(--steel)] transition-colors">
            <PlugZap size={24} />
          </div>
          <div className="space-y-1">
            <h3 className="text-sm font-bold text-[var(--ink)]">未配置 LLM 服务</h3>
            <p className="text-xs text-stone-500">
              请先添加一个模型提供商以启用此功能
            </p>
          </div>
          {onNavigateToModels && (
            <button
              onClick={onNavigateToModels}
              className="mt-2 inline-flex items-center gap-1.5 text-xs font-bold text-[var(--steel)] hover:text-[var(--ink)] hover:underline"
            >
              前往配置 <ArrowUpRight size={12} />
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="group relative overflow-hidden rounded-2xl border border-[var(--stone)] bg-white p-1 shadow-sm transition-all hover:border-[var(--steel)] hover:shadow-md">
      {/* 顶部状态栏 - 模拟卡片头部 */}
      <div className="flex items-center justify-between rounded-xl bg-[var(--paper)] px-4 py-3 border border-transparent group-hover:border-[rgba(0,0,0,0.03)] transition-colors">
        <div className="flex items-center gap-3">
          <div className="flex bg-white p-2 rounded-lg border border-[var(--stone)] text-[var(--ink)] shadow-sm">
            <Cpu size={18} />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-bold text-[var(--ink)]">{displayProvider.name}</h3>
              {isFollowDefault && (
                <span className="inline-flex items-center rounded-md bg-stone-100 px-2 py-0.5 text-[10px] font-medium text-stone-500 border border-stone-200">
                  跟随默认
                </span>
              )}
            </div>
          </div>
        </div>

        {onNavigateToModels && (
          <button
            onClick={onNavigateToModels}
            className="text-xs font-bold text-[var(--steel)] px-3 py-1.5 rounded-lg hover:bg-white hover:shadow-sm transition-all"
          >
            更换
          </button>
        )}
      </div>

      {/* 详细信息区域 */}
      <div className="grid grid-cols-2 gap-px bg-[var(--stone)]/30 mt-1 rounded-xl overflow-hidden border border-[var(--stone)]/30">
        <div className="bg-white p-3 hover:bg-[var(--paper)]/30 transition-colors">
          <span className="block text-[10px] font-bold text-stone-400 uppercase tracking-wider mb-1">模型架构</span>
          <div className="text-xs font-mono text-[var(--ink)] truncate" title={displayProvider.default_model}>
            {displayProvider.default_model}
          </div>
        </div>
        <div className="bg-white p-3 hover:bg-[var(--paper)]/30 transition-colors">
          <span className="block text-[10px] font-bold text-stone-400 uppercase tracking-wider mb-1">API 端点</span>
          <div className="text-xs font-mono text-stone-500 truncate" title={displayProvider.endpoint}>
            {new URL(displayProvider.endpoint).host}
          </div>
        </div>
      </div>
    </div>
  );
}