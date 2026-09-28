import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock,
  CircleDashed,
  Globe2,
  Loader2,
  Plus,
  Settings2,
  Trash2,
  X,
} from "lucide-react";
import type {
  AssistantConfig,
  LlmFeatureConfig,
  ReasoningEffort,
  SearchConfig,
  SearchProviderConfig,
  SearchProviderType,
  SharedLlmConfig,
} from "../types";
import { ApiKeyInput, LlmConnectionConfig } from "../components/common";
import { ReasoningEffortSelect } from "../components/llm/ReasoningEffortSelect";
import { useConfigSave, type ConfigSyncStatus } from "../contexts/ConfigSaveContext";
import {
  isSearchProviderApiConfigured,
  isSearchProviderRuntimeUsable,
  resolveSearchDefaultProviderId,
  selectSearchDefaultProvider,
} from "../utils/searchRuntime";

export type AssistantPageProps = {
  assistantConfig: AssistantConfig;
  setAssistantConfig: Dispatch<SetStateAction<AssistantConfig>>;
  searchConfig: SearchConfig;
  setSearchConfig: Dispatch<SetStateAction<SearchConfig>>;
  sharedConfig: SharedLlmConfig;
  onNavigateToModels?: () => void;
  isRunning: boolean;
};

type SearchProviderTestState = {
  status: "testing" | "success" | "error";
  message?: string;
  resultCount?: number;
  latencyMs?: number;
};

const withReasoningEffort = (
  config: LlmFeatureConfig | undefined,
  effort: ReasoningEffort | undefined,
): LlmFeatureConfig | undefined => {
  if (!config && !effort) return undefined;
  return {
    ...(config ?? { use_shared: true }),
    reasoning: effort ? { effort } : undefined,
  };
};

export function AssistantPage({
  assistantConfig,
  setAssistantConfig,
  searchConfig,
  setSearchConfig,
  sharedConfig,
  onNavigateToModels,
  isRunning,
}: AssistantPageProps) {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [searchDetailsExpanded, setSearchDetailsExpanded] = useState(false);
  const [selectedSearchProviderId, setSelectedSearchProviderId] = useState<string | null>(null);
  const [testingProviderId, setTestingProviderId] = useState<string | null>(null);
  const [providerTestStates, setProviderTestStates] = useState<Record<string, SearchProviderTestState>>({});
  const [showSearchApiKey, setShowSearchApiKey] = useState(false);
  const { syncStatus } = useConfigSave();

  const usableProviderCount = searchConfig.providers.filter(isSearchProviderRuntimeUsable).length;
  const configuredProviders = useMemo(
    () => searchConfig.providers.filter(isSearchProviderApiConfigured),
    [searchConfig.providers],
  );
  const configuredProviderCount = configuredProviders.length;
  const hasConfiguredSearchApi = configuredProviderCount > 0;
  const shouldShowSearchDetails = hasConfiguredSearchApi && searchDetailsExpanded;
  const resolvedDefaultProviderId = useMemo(
    () =>
      resolveSearchDefaultProviderId(
        searchConfig.providers,
        searchConfig.default_provider_id ?? null,
      ),
    [searchConfig.default_provider_id, searchConfig.providers],
  );
  const defaultProvider = useMemo(() => {
    return searchConfig.providers.find((provider) => provider.id === resolvedDefaultProviderId) ?? null;
  }, [resolvedDefaultProviderId, searchConfig.providers]);
  const defaultProviderLabel = defaultProvider
    ? defaultProvider.display_name || SEARCH_PROVIDER_TEMPLATES[defaultProvider.provider_type].name
    : "未配置";
  const defaultProviderEndpoint = defaultProvider
    ? defaultProvider.endpoint?.trim() || SEARCH_PROVIDER_TEMPLATES[defaultProvider.provider_type].endpoint
    : "等待配置 API";
  const selectedSearchProvider = useMemo(() => {
    const explicitProvider = selectedSearchProviderId
      ? searchConfig.providers.find((provider) => provider.id === selectedSearchProviderId)
      : null;
    return explicitProvider ?? defaultProvider ?? searchConfig.providers[0] ?? null;
  }, [defaultProvider, searchConfig.providers, selectedSearchProviderId]);

  useEffect(() => {
    const currentDefaultProviderId = searchConfig.default_provider_id ?? null;
    if (currentDefaultProviderId === resolvedDefaultProviderId) return;

    setSearchConfig((prev) => {
      const nextDefaultProviderId = resolveSearchDefaultProviderId(
        prev.providers,
        prev.default_provider_id ?? null,
      );
      if ((prev.default_provider_id ?? null) === nextDefaultProviderId) return prev;
      return {
        ...prev,
        default_provider_id: nextDefaultProviderId,
      };
    });
  }, [
    resolvedDefaultProviderId,
    searchConfig.default_provider_id,
    setSearchConfig,
  ]);

  useEffect(() => {
    if (!drawerOpen) return;

    if (searchConfig.providers.length === 0) {
      if (selectedSearchProviderId !== null) setSelectedSearchProviderId(null);
      return;
    }

    const selectedExists =
      selectedSearchProviderId !== null &&
      searchConfig.providers.some((provider) => provider.id === selectedSearchProviderId);
    if (selectedExists) return;

    const defaultExists = searchConfig.default_provider_id
      ? searchConfig.providers.some((provider) => provider.id === searchConfig.default_provider_id)
      : false;
    setSelectedSearchProviderId(
      defaultExists ? searchConfig.default_provider_id ?? null : searchConfig.providers[0].id,
    );
  }, [
    drawerOpen,
    searchConfig.default_provider_id,
    searchConfig.providers,
    selectedSearchProviderId,
  ]);

  const addProvider = (providerType: SearchProviderType) => {
    const template = SEARCH_PROVIDER_TEMPLATES[providerType];
    const id = `${providerType}-${Date.now().toString(36)}`;
    const provider: SearchProviderConfig = {
      id,
      provider_type: providerType,
      display_name: template.name,
      enabled: true,
      endpoint: template.endpoint,
      api_key: "",
    };
    setSearchConfig((prev) => ({
      ...prev,
      providers: [...prev.providers, provider],
      default_provider_id:
        prev.default_provider_id ?? (isSearchProviderApiConfigured(provider) ? id : null),
    }));
    setSelectedSearchProviderId(id);
  };

  const updateProvider = (id: string, patch: Partial<SearchProviderConfig>) => {
    setSearchConfig((prev) => ({
      ...prev,
      providers: prev.providers.map((provider) =>
        provider.id === id ? { ...provider, ...patch } : provider,
      ),
    }));
  };

  const removeProvider = (id: string) => {
    setSearchConfig((prev) => {
      const providers = prev.providers.filter((provider) => provider.id !== id);
      return {
        ...prev,
        providers,
        default_provider_id: resolveSearchDefaultProviderId(
          providers,
          prev.default_provider_id === id ? null : prev.default_provider_id ?? null,
        ),
      };
    });
    setSelectedSearchProviderId((current) => (current === id ? null : current));
  };

  const testProvider = async (provider: SearchProviderConfig) => {
    const startedAt = performance.now();
    setTestingProviderId(provider.id);
    setProviderTestStates((prev) => ({
      ...prev,
      [provider.id]: { status: "testing" },
    }));
    try {
      const count = await invoke<number>("test_search_provider", { provider });
      setProviderTestStates((prev) => ({
        ...prev,
        [provider.id]: {
          status: "success",
          message: "搜索 API 已返回可解析结果。",
          resultCount: count,
          latencyMs: performance.now() - startedAt,
        },
      }));
    } catch (error) {
      setProviderTestStates((prev) => ({
        ...prev,
        [provider.id]: {
          status: "error",
          message: String(error),
          latencyMs: performance.now() - startedAt,
        },
      }));
    } finally {
      setTestingProviderId(null);
    }
  };

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
          <h4 className="text-sm font-bold text-stone-700">LLM 连接配置</h4>
          <div className="p-4 bg-[var(--paper)] rounded-2xl border border-[var(--stone)]">
            <LlmConnectionConfig
              sharedConfig={sharedConfig}
              featureName="assistant"
              onNavigateToModels={onNavigateToModels}
            />
          </div>
        </div>

        <div className="space-y-4">
          <div>
            <h4 className="text-sm font-bold text-stone-700 flex items-center gap-2">
              <Globe2 size={15} className="text-[var(--steel)]" />
              联网搜索
            </h4>
            <p className="text-xs text-stone-500 mt-1">
              开启后，AI 助手在需要实时信息时会把搜索词发送给第三方搜索 API。
            </p>
          </div>

          <div className="rounded-2xl border border-[var(--stone)] bg-[var(--paper)] p-4 space-y-4">
            {hasConfiguredSearchApi ? (
              <>
                <div className="flex items-start justify-between gap-4 rounded-xl border border-[rgba(176,174,165,0.55)] bg-white px-4 py-3">
                  <div className="min-w-0">
                    <div className="text-sm font-bold text-[var(--ink)]">启用联网搜索</div>
                    <div className="mt-1 text-xs leading-relaxed text-stone-500">
                      已配置 {configuredProviderCount} 个搜索 API，需要实时信息时允许 LLM 调用搜索工具。
                    </div>
                  </div>
                  <ToggleSwitch
                    checked={assistantConfig.enable_web_search}
                    disabled={isRunning}
                    label="启用联网搜索"
                    onChange={(checked) =>
                      setAssistantConfig((prev) => ({
                        ...prev,
                        enable_web_search: checked,
                      }))
                    }
                  />
                </div>

                <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <SearchStatusCard
                    label="默认搜索引擎"
                    value={defaultProviderLabel}
                    detail={defaultProviderEndpoint}
                    active={Boolean(defaultProvider)}
                  />
                  <SearchStatusCard
                    label="API 配置"
                    value="已配置"
                    detail={`${configuredProviderCount} 个引擎可用`}
                    active
                  />
                  <SearchStatusCard
                    label="可用引擎"
                    value={`${usableProviderCount}/${searchConfig.providers.length}`}
                    detail={searchConfig.enable_fallback ? "失败时自动降级" : "仅使用默认引擎"}
                    active={usableProviderCount > 0}
                  />
                </div>

                <div className="flex flex-col gap-3 rounded-xl border border-[rgba(106,155,204,0.22)] bg-[rgba(106,155,204,0.08)] px-4 py-3 md:flex-row md:items-center md:justify-between">
                  <div>
                    <div className="text-sm font-bold text-[var(--ink)]">联网搜索 API 配置</div>
                    <div className="mt-1 text-xs leading-relaxed text-stone-600">
                      管理搜索服务商、API Key、默认引擎和连接测试。高级参数默认折叠。
                    </div>
                    <AutoSaveStatus syncStatus={syncStatus} />
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      disabled={isRunning}
                      onClick={() => setSearchDetailsExpanded((prev) => !prev)}
                      aria-label={searchDetailsExpanded ? "收起高级参数" : "展开高级参数"}
                      title={searchDetailsExpanded ? "收起高级参数" : "展开高级参数"}
                      className="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-[var(--stone)] bg-white text-stone-700 transition-colors hover:border-[var(--steel)] hover:text-[var(--steel)] disabled:opacity-60"
                    >
                      {searchDetailsExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                    </button>
                    <button
                      type="button"
                      disabled={isRunning}
                      onClick={() => setDrawerOpen(true)}
                      className="inline-flex h-10 items-center justify-center gap-1.5 rounded-xl bg-[var(--ink)] px-4 text-xs font-bold text-white transition-colors hover:bg-[var(--steel)] disabled:opacity-60"
                    >
                      <Settings2 size={14} />
                      配置 API
                    </button>
                  </div>
                </div>

                {shouldShowSearchDetails && (
                  <>
                    <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                      <NumberField
                        label="最大工具循环"
                        value={assistantConfig.web_search_max_loops}
                        min={1}
                        max={3}
                        disabled={isRunning}
                        onChange={(value) =>
                          setAssistantConfig((prev) => ({
                            ...prev,
                            web_search_max_loops: value,
                          }))
                        }
                      />
                      <NumberField
                        label="单次结果数"
                        value={searchConfig.max_results}
                        min={1}
                        max={10}
                        disabled={isRunning}
                        onChange={(value) =>
                          setSearchConfig((prev) => ({
                            ...prev,
                            max_results: value,
                          }))
                        }
                      />
                      <NumberField
                        label="请求超时"
                        value={searchConfig.timeout_secs}
                        min={1}
                        max={30}
                        disabled={isRunning}
                        suffix="秒"
                        onChange={(value) =>
                          setSearchConfig((prev) => ({
                            ...prev,
                            timeout_secs: value,
                          }))
                        }
                      />
                    </div>

                    <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                      <div className="flex items-start justify-between gap-3 rounded-xl border border-[rgba(176,174,165,0.55)] bg-white px-4 py-3">
                        <span>
                          <span className="block text-sm font-semibold text-stone-700">文本处理模式也启用搜索</span>
                          <span className="mt-1 block text-xs leading-relaxed text-stone-500">
                            润色、翻译通常不需要联网，只有处理最新资料时建议开启。
                          </span>
                        </span>
                        <ToggleSwitch
                          checked={assistantConfig.web_search_in_text_mode}
                          disabled={isRunning}
                          label="文本处理模式也启用搜索"
                          onChange={(checked) =>
                            setAssistantConfig((prev) => ({
                              ...prev,
                              web_search_in_text_mode: checked,
                            }))
                          }
                        />
                      </div>
                      <div className="flex items-start justify-between gap-3 rounded-xl border border-[rgba(176,174,165,0.55)] bg-white px-4 py-3">
                        <span>
                          <span className="block text-sm font-semibold text-stone-700">默认引擎失败时自动降级</span>
                          <span className="mt-1 block text-xs leading-relaxed text-stone-500">
                            建议至少配置两个引擎，避免第三方 API 临时失败。
                          </span>
                        </span>
                        <ToggleSwitch
                          checked={searchConfig.enable_fallback}
                          disabled={isRunning}
                          label="默认引擎失败时自动降级"
                          onChange={(checked) =>
                            setSearchConfig((prev) => ({
                              ...prev,
                              enable_fallback: checked,
                            }))
                          }
                        />
                      </div>
                    </div>
                  </>
                )}
              </>
            ) : (
              <div className="flex flex-col gap-4 rounded-xl border border-[rgba(106,155,204,0.22)] bg-white px-4 py-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-sm font-bold text-[var(--ink)]">未配置联网搜索 API</div>
                  <div className="mt-1 text-xs leading-relaxed text-stone-500">
                    配置搜索服务商后再显示启用开关和高级参数，避免还没填 Key 就铺开无效选项。
                  </div>
                  <AutoSaveStatus syncStatus={syncStatus} />
                </div>
                <button
                  type="button"
                  disabled={isRunning}
                  onClick={() => setDrawerOpen(true)}
                  className="inline-flex h-10 items-center justify-center gap-1.5 rounded-xl bg-[var(--ink)] px-4 text-xs font-bold text-white transition-colors hover:bg-[var(--steel)] disabled:opacity-60"
                >
                  <Settings2 size={14} />
                  配置 API
                </button>
              </div>
            )}
          </div>
        </div>

        <div className="space-y-4">
          <h4 className="text-sm font-bold text-stone-700">问答模式提示词</h4>
          <p className="text-xs text-stone-500">无选中文本时，用于回答问题。</p>
          <ReasoningEffortSelect
            context={{ kind: "assistant", config: assistantConfig, shared: sharedConfig, text_processing: false }}
            value={assistantConfig.qa_llm?.reasoning?.effort}
            disabled={isRunning}
            label="问答思考模式"
            description="默认沿用已有行为；需要调整时选择当前模型提供的选项。"
            onChange={(effort) =>
              setAssistantConfig((prev) => ({
                ...prev,
                qa_llm: withReasoningEffort(prev.qa_llm, effort),
              }))
            }
          />
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
          <ReasoningEffortSelect
            context={{ kind: "assistant", config: assistantConfig, shared: sharedConfig, text_processing: true }}
            value={assistantConfig.text_processing_llm?.reasoning?.effort}
            disabled={isRunning}
            label="文本处理思考模式"
            description="默认沿用已有行为；支持关闭的模型可用于减少简单文本处理的等待。"
            onChange={(effort) =>
              setAssistantConfig((prev) => ({
                ...prev,
                text_processing_llm: withReasoningEffort(prev.text_processing_llm, effort),
              }))
            }
          />
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
      {drawerOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <button
            type="button"
            className="absolute inset-0 bg-stone-900/30 backdrop-blur-sm"
            aria-label="关闭搜索引擎管理"
            onClick={() => setDrawerOpen(false)}
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="search-provider-modal-title"
            className="relative flex h-[82vh] max-h-[calc(100vh-2rem)] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-[var(--stone)] bg-white shadow-2xl"
          >
            <div className="flex items-center justify-between border-b border-[var(--stone)] bg-[var(--paper)] px-5 py-4">
              <div>
                <div id="search-provider-modal-title" className="text-base font-bold text-[var(--ink)]">
                  管理搜索引擎
                </div>
                <div className="text-xs text-stone-500">配置 API 凭证 · 调整通用参数 · 测试连接</div>
              </div>
              <button
                type="button"
                onClick={() => setDrawerOpen(false)}
                aria-label="关闭搜索引擎管理"
                className="rounded-xl p-2 text-stone-400 transition-colors hover:bg-white hover:text-stone-700"
              >
                <X size={18} />
              </button>
            </div>

            <div className="border-b border-[var(--stone)] px-5 py-4">
              <div className="mb-3 flex items-center justify-between gap-3">
                <div>
                  <div className="text-sm font-bold text-[var(--ink)]">通用参数</div>
                  <div className="mt-0.5 text-xs text-stone-500">这些参数会作用于所有搜索服务商。</div>
                </div>
                <AutoSaveStatus syncStatus={syncStatus} />
              </div>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                <NumberField
                  label="单次最大结果"
                  value={searchConfig.max_results}
                  min={1}
                  max={10}
                  disabled={isRunning}
                  onChange={(value) =>
                    setSearchConfig((prev) => ({
                      ...prev,
                      max_results: value,
                    }))
                  }
                />
                <NumberField
                  label="请求超时"
                  value={searchConfig.timeout_secs}
                  min={1}
                  max={30}
                  suffix="秒"
                  disabled={isRunning}
                  onChange={(value) =>
                    setSearchConfig((prev) => ({
                      ...prev,
                      timeout_secs: value,
                    }))
                  }
                />
                <div className="flex items-center justify-between rounded-xl border border-[var(--stone)] bg-white px-3 py-2">
                  <span>
                    <span className="block text-xs font-semibold text-stone-600">失败自动降级</span>
                    <span className="mt-0.5 block text-[11px] text-stone-400">默认引擎失败时尝试备用项</span>
                  </span>
                  <ToggleSwitch
                    checked={searchConfig.enable_fallback}
                    disabled={isRunning}
                    label="失败自动降级"
                    onChange={(checked) =>
                      setSearchConfig((prev) => ({
                        ...prev,
                        enable_fallback: checked,
                      }))
                    }
                  />
                </div>
              </div>
            </div>

            <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[18rem_minmax(0,1fr)]">
              <aside
                aria-label="搜索服务商列表"
                className="flex min-h-0 flex-col border-b border-[var(--stone)] bg-[var(--paper)] md:border-b-0 md:border-r"
              >
                <div className="border-b border-[var(--stone)] p-4">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-stone-400">
                    添加服务商
                  </div>
                  <div className="mt-2 grid grid-cols-2 gap-2">
                    {(Object.keys(SEARCH_PROVIDER_TEMPLATES) as SearchProviderType[]).map((type) => (
                      <button
                        key={type}
                        type="button"
                        disabled={isRunning}
                        onClick={() => addProvider(type)}
                        className="inline-flex h-9 cursor-pointer items-center justify-center gap-1.5 rounded-xl border border-[var(--stone)] bg-white px-2 text-xs font-semibold text-stone-700 transition-colors hover:border-[var(--steel)] hover:text-[var(--steel)] disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        <Plus size={13} />
                        {SEARCH_PROVIDER_TEMPLATES[type].name}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="min-h-0 flex-1 overflow-y-auto p-3">
                  {searchConfig.providers.length === 0 ? (
                    <div className="rounded-xl border border-dashed border-[var(--stone)] bg-white px-4 py-8 text-center">
                      <div className="text-sm font-bold text-stone-600">还没有搜索服务商</div>
                      <div className="mt-1 text-xs leading-relaxed text-stone-500">
                        从上方选择一个模板，新建后会自动进入右侧详情。
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      {searchConfig.providers.map((provider) => {
                        const providerLabel =
                          provider.display_name || SEARCH_PROVIDER_TEMPLATES[provider.provider_type].name;
                        const isSelected = selectedSearchProvider?.id === provider.id;
                        const isDefault = searchConfig.default_provider_id === provider.id;
                        const isConfigured = isSearchProviderApiConfigured(provider);

                        return (
                          <div
                            key={provider.id}
                            className={[
                              "group flex items-stretch overflow-hidden rounded-xl border bg-white transition-colors",
                              isSelected
                                ? "border-[var(--steel)] ring-1 ring-[rgba(106,155,204,0.25)]"
                                : "border-[var(--stone)] hover:border-[var(--steel)]",
                            ].join(" ")}
                          >
                            <button
                              type="button"
                              onClick={() => setSelectedSearchProviderId(provider.id)}
                              className="min-w-0 flex-1 cursor-pointer px-3 py-2.5 text-left"
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="truncate text-sm font-bold text-[var(--ink)]">
                                  {providerLabel}
                                </span>
                                {isDefault && (
                                  <span className="shrink-0 rounded-full bg-[rgba(120,140,93,0.14)] px-2 py-0.5 text-[10px] font-bold text-[var(--sage)]">
                                    默认
                                  </span>
                                )}
                              </div>
                              <div className="mt-1 flex items-center justify-between gap-2">
                                <span className="truncate text-[11px] text-stone-500">
                                  {provider.endpoint?.trim() || "等待配置 Endpoint"}
                                </span>
                              </div>
                              <div className="mt-2 flex items-center gap-2">
                                <SearchProviderStateBadge
                                  configured={isConfigured}
                                  enabled={provider.enabled}
                                />
                                <span className="text-[10px] uppercase tracking-wider text-stone-400">
                                  {provider.provider_type}
                                </span>
                              </div>
                            </button>
                            <button
                              type="button"
                              disabled={isRunning}
                              onClick={() => removeProvider(provider.id)}
                              aria-label={`删除 ${providerLabel}`}
                              title="删除"
                              className="flex w-10 cursor-pointer items-center justify-center border-l border-[var(--stone)] text-stone-400 transition-colors hover:bg-red-50 hover:text-red-600 disabled:cursor-not-allowed disabled:opacity-60"
                            >
                              <Trash2 size={14} />
                            </button>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              </aside>

              <section aria-label="搜索服务商详情" className="min-h-0 overflow-y-auto p-5">
                {selectedSearchProvider ? (
                  <div className="space-y-5">
                    <div className="flex flex-col gap-3 border-b border-[var(--stone)] pb-4 sm:flex-row sm:items-start sm:justify-between">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <h3 className="truncate text-lg font-bold text-[var(--ink)]">
                            {selectedSearchProvider.display_name ||
                              SEARCH_PROVIDER_TEMPLATES[selectedSearchProvider.provider_type].name}
                          </h3>
                          <SearchProviderStateBadge
                            configured={isSearchProviderApiConfigured(selectedSearchProvider)}
                            enabled={selectedSearchProvider.enabled}
                          />
                        </div>
                        <p className="mt-1 text-xs leading-relaxed text-stone-500">
                          选择左侧服务商后编辑连接信息，未配置完整前不能设为默认。
                        </p>
                      </div>
                      <div className="flex flex-wrap items-center gap-2">
                        <div className="flex h-10 items-center gap-2 rounded-xl border border-[var(--stone)] bg-white px-3 text-xs font-semibold text-stone-700">
                          <span>启用</span>
                          <ToggleSwitch
                            checked={selectedSearchProvider.enabled}
                            disabled={isRunning}
                            label="启用搜索服务商"
                            onChange={(checked) =>
                              updateProvider(selectedSearchProvider.id, { enabled: checked })
                            }
                          />
                        </div>
                        <button
                          type="button"
                          disabled={
                            isRunning || !isSearchProviderApiConfigured(selectedSearchProvider)
                          }
                          onClick={() =>
                            setSearchConfig((prev) =>
                              selectSearchDefaultProvider(prev, selectedSearchProvider.id),
                            )
                          }
                          className="inline-flex h-10 cursor-pointer items-center justify-center gap-1.5 rounded-xl border border-[var(--stone)] bg-white px-3 text-xs font-semibold text-stone-700 transition-colors hover:border-[var(--steel)] hover:text-[var(--steel)] disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          <CheckCircle2 size={14} />
                          {searchConfig.default_provider_id === selectedSearchProvider.id
                            ? "当前默认"
                            : "设为默认"}
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      <Field
                        label="名称"
                        value={selectedSearchProvider.display_name}
                        disabled={isRunning}
                        onChange={(value) =>
                          updateProvider(selectedSearchProvider.id, { display_name: value })
                        }
                      />
                      <Field
                        label="Endpoint"
                        value={selectedSearchProvider.endpoint ?? ""}
                        disabled={isRunning}
                        onChange={(value) =>
                          updateProvider(selectedSearchProvider.id, { endpoint: value })
                        }
                      />
                      {selectedSearchProvider.provider_type !== "searxng" && (
                        <label className="space-y-1 text-xs font-semibold text-stone-500">
                          <span>API Key</span>
                          <ApiKeyInput
                            value={selectedSearchProvider.api_key ?? ""}
                            disabled={isRunning}
                            show={showSearchApiKey}
                            onToggleShow={() => setShowSearchApiKey((prev) => !prev)}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, { api_key: value })
                            }
                          />
                        </label>
                      )}
                      {selectedSearchProvider.provider_type === "serper" && (
                        <>
                          <Field
                            label="gl"
                            value={selectedSearchProvider.serper_gl ?? ""}
                            disabled={isRunning}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, { serper_gl: value })
                            }
                          />
                          <Field
                            label="hl"
                            value={selectedSearchProvider.serper_hl ?? ""}
                            disabled={isRunning}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, { serper_hl: value })
                            }
                          />
                          <Field
                            label="tbs"
                            value={selectedSearchProvider.serper_tbs ?? ""}
                            disabled={isRunning}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, { serper_tbs: value })
                            }
                          />
                        </>
                      )}
                      {selectedSearchProvider.provider_type === "searxng" && (
                        <>
                          <Field
                            label="language"
                            value={selectedSearchProvider.searxng_language ?? ""}
                            disabled={isRunning}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, {
                                searxng_language: value,
                              })
                            }
                          />
                          <Field
                            label="time_range"
                            value={selectedSearchProvider.searxng_time_range ?? ""}
                            disabled={isRunning}
                            onChange={(value) =>
                              updateProvider(selectedSearchProvider.id, {
                                searxng_time_range: value,
                              })
                            }
                          />
                        </>
                      )}
                    </div>

                    <div className="flex flex-col gap-3 rounded-xl border border-[var(--stone)] bg-[var(--paper)] px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                      <div>
                        <div className="text-sm font-bold text-[var(--ink)]">连接测试</div>
                        <p className="mt-1 text-xs leading-relaxed text-stone-500">
                          保存配置后可测试第三方搜索 API 是否可用。
                        </p>
                        <SearchProviderTestFeedback
                          state={providerTestStates[selectedSearchProvider.id]}
                        />
                      </div>
                      <button
                        type="button"
                        disabled={isRunning || testingProviderId === selectedSearchProvider.id}
                        onClick={() => void testProvider(selectedSearchProvider)}
                        className="inline-flex h-10 cursor-pointer items-center justify-center gap-1.5 rounded-xl border border-[var(--stone)] bg-white px-3 text-xs font-semibold text-stone-700 transition-colors hover:border-[var(--steel)] hover:text-[var(--steel)] disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        {testingProviderId === selectedSearchProvider.id ? (
                          <Loader2 size={14} className="animate-spin" />
                        ) : (
                          <CheckCircle2 size={14} />
                        )}
                        测试连接
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="flex min-h-[360px] flex-col items-center justify-center rounded-xl border border-dashed border-[var(--stone)] bg-[var(--paper)] px-6 text-center">
                    <CircleDashed size={28} className="text-stone-400" />
                    <div className="mt-3 text-sm font-bold text-stone-600">
                      选择左侧服务商后编辑连接信息
                    </div>
                    <p className="mt-1 max-w-sm text-xs leading-relaxed text-stone-500">
                      当前还没有选中的搜索服务商。可以先从左侧模板添加 Tavily、博查、Serper 或 SearXNG。
                    </p>
                  </div>
                )}
              </section>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function SearchProviderTestFeedback({ state }: { state?: SearchProviderTestState }) {
  if (!state) return null;

  if (state.status === "testing") {
    return (
      <div
        role="status"
        aria-live="polite"
        className="mt-3 inline-flex items-center gap-2 rounded-xl border border-amber-100 bg-amber-50 px-3 py-2 text-xs font-semibold text-amber-700"
      >
        <Loader2 size={14} className="animate-spin" />
        <span>测试中</span>
      </div>
    );
  }

  const isSuccess = state.status === "success";

  return (
    <div
      role="status"
      aria-live="polite"
      className={[
        "mt-3 rounded-xl px-4 py-3 text-sm border",
        isSuccess
          ? "bg-emerald-50 border-emerald-100 text-emerald-800"
          : "bg-red-50 border-red-100 text-red-700",
      ].join(" ")}
    >
      <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 items-start gap-2">
          <div className="mt-0.5">
            {isSuccess ? (
              <CheckCircle2 size={16} className="text-emerald-600" />
            ) : (
              <AlertCircle size={16} className="text-red-600" />
            )}
          </div>
          <div className="min-w-0">
            <div className="font-bold">{isSuccess ? "连接正常" : "连接失败"}</div>
            {isSuccess && typeof state.resultCount === "number" && (
              <div className="mt-1 text-xs font-semibold">返回 {state.resultCount} 条结果</div>
            )}
            {state.message && (
              <div className="mt-1 whitespace-pre-wrap break-words text-xs font-mono">
                {state.message}
              </div>
            )}
          </div>
        </div>
        <SearchTestLatencyBadge latencyMs={state.latencyMs} status={state.status} />
      </div>
    </div>
  );
}

function SearchTestLatencyBadge({
  latencyMs,
  status,
}: {
  latencyMs?: number;
  status: SearchProviderTestState["status"];
}) {
  if (latencyMs === undefined) return null;

  const label = latencyMs < 1000
    ? `${Math.round(latencyMs)}ms`
    : `${(latencyMs / 1000).toFixed(2)}s`;
  const isSuccess = status === "success";

  return (
    <span
      className={[
        "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-bold",
        isSuccess
          ? "border-emerald-100 bg-white/70 text-emerald-700"
          : "border-red-100 bg-white/70 text-red-700",
      ].join(" ")}
    >
      <Clock size={12} />
      {label}
    </span>
  );
}

function AutoSaveStatus({ syncStatus }: { syncStatus: ConfigSyncStatus }) {
  if (syncStatus === "idle") return null;

  const statusMap: Record<
    Exclude<ConfigSyncStatus, "idle">,
    { label: string; className: string; loading?: boolean }
  > = {
    syncing: {
      label: "保存中",
      className: "text-[var(--steel)]",
      loading: true,
    },
    success: {
      label: "已自动保存",
      className: "text-[var(--sage)]",
    },
    error: {
      label: "保存失败",
      className: "text-red-600",
    },
  };
  const status = statusMap[syncStatus];

  return (
    <div
      role="status"
      aria-live="polite"
      className={[
        "mt-2 inline-flex items-center gap-1.5 text-[11px] font-semibold",
        status.className,
      ].join(" ")}
      title="提示词与联网搜索配置会自动保存"
    >
      {status.loading ? (
        <Loader2 size={12} className="animate-spin" />
      ) : (
        <span className="h-1.5 w-1.5 rounded-full bg-current opacity-75" />
      )}
      <span>{status.label}</span>
    </div>
  );
}

function SearchStatusCard({
  label,
  value,
  detail,
  active,
}: {
  label: string;
  value: string;
  detail: string;
  active: boolean;
}) {
  return (
    <div className="rounded-xl border border-[var(--stone)] bg-white px-3 py-2.5">
      <div className="mb-1 flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-stone-400">
        {active ? (
          <CheckCircle2 size={11} className="text-[var(--sage)]" />
        ) : (
          <CircleDashed size={11} className="text-stone-400" />
        )}
        {label}
      </div>
      <div className="truncate text-sm font-bold text-[var(--ink)]" title={value}>
        {value}
      </div>
      <div className="mt-0.5 truncate text-[11px] text-stone-500" title={detail}>
        {detail}
      </div>
    </div>
  );
}

function SearchProviderStateBadge({
  configured,
  enabled,
}: {
  configured: boolean;
  enabled: boolean;
}) {
  const state = !configured
    ? {
        label: "未配置",
        className: "bg-stone-100 text-stone-500",
      }
    : enabled
      ? {
          label: "可用",
          className: "bg-[rgba(120,140,93,0.14)] text-[var(--sage)]",
        }
      : {
          label: "已配置 · 未启用",
          className: "bg-[rgba(106,155,204,0.12)] text-[var(--steel)]",
        };

  return (
    <span
      className={[
        "inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[10px] font-bold",
        state.className,
      ].join(" ")}
    >
      {state.label}
    </span>
  );
}

function ToggleSwitch({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={() => onChange(!checked)}
      role="switch"
      aria-label={label}
      aria-checked={checked}
      className={[
        "relative h-6 w-11 shrink-0 rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgba(106,155,204,0.35)] disabled:cursor-not-allowed disabled:opacity-60",
        checked ? "bg-[var(--steel)]" : "bg-stone-300",
      ].join(" ")}
    >
      <span
        className={[
          "absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow-sm transition-transform",
          checked ? "translate-x-5" : "translate-x-0",
        ].join(" ")}
      />
    </button>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  disabled,
  suffix,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled: boolean;
  suffix?: string;
  onChange: (value: number) => void;
}) {
  const clamp = (next: number) => Math.min(max, Math.max(min, next));

  return (
    <label className="space-y-1">
      <span className="text-xs font-semibold text-stone-500">{label}</span>
      <div className="flex overflow-hidden rounded-xl border border-[var(--stone)] bg-white focus-within:border-[var(--steel)]">
        <button
          type="button"
          disabled={disabled || value <= min}
          onClick={() => onChange(clamp(value - 1))}
          className="flex h-10 w-9 items-center justify-center border-r border-[var(--stone)] text-sm font-bold text-stone-500 transition-colors hover:bg-[var(--paper)] disabled:opacity-40"
          aria-label={`${label} 减少`}
        >
          -
        </button>
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          disabled={disabled}
          onChange={(event) => onChange(clamp(Number(event.target.value) || min))}
          className="h-10 min-w-0 flex-1 bg-white px-2 text-center text-sm font-semibold text-[var(--ink)] outline-none disabled:opacity-60"
        />
        <button
          type="button"
          disabled={disabled || value >= max}
          onClick={() => onChange(clamp(value + 1))}
          className="flex h-10 w-9 items-center justify-center border-l border-[var(--stone)] text-sm font-bold text-stone-500 transition-colors hover:bg-[var(--paper)] disabled:opacity-40"
          aria-label={`${label} 增加`}
        >
          +
        </button>
        {suffix && (
          <span className="flex h-10 items-center border-l border-[var(--stone)] px-2 text-xs text-stone-400">
            {suffix}
          </span>
        )}
      </div>
    </label>
  );
}

const SEARCH_PROVIDER_TEMPLATES: Record<
  SearchProviderType,
  { name: string; endpoint: string }
> = {
  tavily: { name: "Tavily", endpoint: "https://api.tavily.com/search" },
  bocha: { name: "博查", endpoint: "https://api.bochaai.com/v1/web-search" },
  serper: { name: "Serper", endpoint: "https://google.serper.dev/search" },
  searxng: { name: "SearXNG", endpoint: "http://127.0.0.1:8080" },
};

function Field({
  label,
  value,
  disabled,
  type = "text",
  onChange,
}: {
  label: string;
  value: string;
  disabled: boolean;
  type?: "text" | "password";
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1 text-xs font-semibold text-stone-500">
      <span>{label}</span>
      <input
        type={type}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        className="w-full rounded-xl border border-[var(--stone)] bg-white px-3 py-2 text-sm font-normal text-stone-700 outline-none focus:border-[var(--steel)] disabled:opacity-60"
      />
    </label>
  );
}
