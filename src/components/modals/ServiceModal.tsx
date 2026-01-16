import type { Dispatch, SetStateAction } from "react";
import {
  AlertCircle,
  BookText,
  CheckCircle2,
  MessageSquareQuote,
  Mic,
  Plus,
  RotateCcw,
  Settings,
  Trash2,
  Wand2,
  X,
} from "lucide-react";
import type {
  AsrCache,
  AsrConfig,
  AsrProvider,
  DualHotkeyConfig,
  AssistantConfig,
  LlmConfig,
  LlmPreset,
  ServiceModalTab,
} from "../../types";
import { ASR_PROVIDERS } from "../../constants";
import { formatHotkeyDisplay } from "../../utils";
import { ApiKeyInput, Toggle } from "../common";

export type ServiceModalProps = {
  open: boolean;
  tab: ServiceModalTab;
  setTab: (tab: ServiceModalTab) => void;
  onClose: () => void;

  asrCache: AsrCache;
  setAsrCache: Dispatch<SetStateAction<AsrCache>>;
  asrConfig: AsrConfig;
  setAsrConfig: Dispatch<SetStateAction<AsrConfig>>;

  llmConfig: LlmConfig;
  setLlmConfig: Dispatch<SetStateAction<LlmConfig>>;
  activePreset: LlmPreset;
  defaultPresets: LlmPreset[];
  handleAddPreset: () => void;
  handleDeletePreset: (id: string) => void;
  handleUpdateActivePreset: (key: keyof LlmPreset, value: string) => void;

  assistantConfig: AssistantConfig;
  setAssistantConfig: Dispatch<SetStateAction<AssistantConfig>>;
  dualHotkeyConfig: DualHotkeyConfig;

  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;

  dictionary: string[];
  newWord: string;
  setNewWord: (next: string) => void;
  duplicateHint: boolean;
  setDuplicateHint: (next: boolean) => void;
  editingIndex: number | null;
  editingValue: string;
  setEditingValue: (next: string) => void;
  handleAddWord: () => void;
  handleDeleteWord: (index: number) => void;
  handleStartEdit: (index: number) => void;
  handleSaveEdit: () => void;
  handleCancelEdit: () => void;

  handleSaveConfig: () => void;
};

export function ServiceModal({
  open,
  tab,
  setTab,
  onClose,
  asrCache,
  setAsrCache,
  asrConfig,
  setAsrConfig,
  llmConfig,
  setLlmConfig,
  activePreset,
  defaultPresets,
  handleAddPreset,
  handleDeletePreset,
  handleUpdateActivePreset,
  assistantConfig,
  setAssistantConfig,
  dualHotkeyConfig,
  showApiKey,
  setShowApiKey,
  dictionary,
  newWord,
  setNewWord,
  duplicateHint,
  setDuplicateHint,
  editingIndex,
  editingValue,
  setEditingValue,
  handleAddWord,
  handleDeleteWord,
  handleStartEdit,
  handleSaveEdit,
  handleCancelEdit,
  handleSaveConfig,
}: ServiceModalProps) {
  if (!open) return null;

  const serviceModalTab = tab;
  const setServiceModalTab = setTab;

  return (

            <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
              <div className="bg-white rounded-3xl border border-[var(--stone)] shadow-2xl w-full max-w-4xl mx-4 h-[85vh] flex flex-col overflow-hidden animate-in zoom-in-95 duration-200">
                {/* Modal Header with Tabs */}
                <div className="px-6 py-4 border-b border-[var(--stone)] bg-[var(--paper)]">
                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center gap-3">
                      <div className="p-2 bg-[rgba(106,155,204,0.12)] rounded-xl text-[var(--steel)]">
                        <Settings size={20} />
                      </div>
                      <div>
                        <h3 className="text-lg font-bold text-slate-900">服务配置</h3>
                        <p className="text-xs text-slate-500">统一管理 ASR、LLM 润色和 AI 助手配置</p>
                      </div>
                    </div>
                    <button onClick={() => onClose()} className="p-2 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-600 transition-colors">
                      <X size={20} />
                    </button>
                  </div>
                  {/* Tab Bar */}
                  <div className="flex gap-1 p-1 bg-[var(--panel)] border border-[var(--stone)] rounded-2xl">
                    <button
                      onClick={() => setServiceModalTab('asr')}
                      className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                        serviceModalTab === 'asr'
                          ? 'bg-white text-[var(--steel)] shadow-sm'
                          : 'text-slate-500 hover:text-slate-700'
                      }`}
                    >
                      <Mic size={16} />
                      ASR 语音识别
                    </button>
                    <button
                      onClick={() => setServiceModalTab('llm')}
                      className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                        serviceModalTab === 'llm'
                          ? 'bg-white text-violet-600 shadow-sm'
                          : 'text-slate-500 hover:text-slate-700'
                      }`}
                    >
                      <Wand2 size={16} />
                      LLM 润色
                    </button>
                    <button
                      onClick={() => setServiceModalTab('assistant')}
                      className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                        serviceModalTab === 'assistant'
                          ? 'bg-white text-emerald-700 shadow-sm'
                          : 'text-slate-500 hover:text-slate-700'
                      }`}
                    >
                      <MessageSquareQuote size={16} />
                      AI 助手
                    </button>
                    <button
                      onClick={() => setServiceModalTab('dictionary')}
                      className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all ${
                        serviceModalTab === 'dictionary'
                          ? 'bg-white text-orange-600 shadow-sm'
                          : 'text-slate-500 hover:text-slate-700'
                      }`}
                    >
                      <BookText size={16} />
                      个人词典
                    </button>
                  </div>
                </div>
                {/* Tab Content */}
                <div className="flex-1 overflow-hidden">
                  {/* ASR Tab */}
                  {serviceModalTab === 'asr' && (
                    <div className="h-full overflow-y-auto p-6 space-y-6">
                      <div className="flex items-center gap-2 p-3 bg-blue-50 border border-blue-100 rounded-xl text-xs text-blue-700">
                        <AlertCircle size={14} className="flex-shrink-0" />
                        <span>ASR 用于语音转文字，支持阿里千问和豆包两种主模型，以及硅基移动作为备用模型</span>
                      </div>
                      {/* 主模型配置 */}
                      <div className="space-y-4">
                        <h4 className="text-sm font-bold text-slate-700">主模型</h4>
                        <div className="space-y-3 p-4 bg-slate-50 rounded-xl border border-slate-200">
                          <div className="space-y-2">
                            <label className="text-xs font-medium text-slate-600">服务商</label>
                            <select
                              value={asrConfig.primary.provider}
                              onChange={(e) => {
                                const newProvider = e.target.value as AsrProvider;
                                setAsrConfig(prev => ({
                                  ...prev,
                                  primary: newProvider === 'qwen'
                                    ? { provider: 'qwen', api_key: asrCache.qwen.api_key }
                                    : { provider: 'doubao', api_key: '', app_id: asrCache.doubao.app_id, access_token: asrCache.doubao.access_token }
                                }));
                                setAsrCache(prev => ({ ...prev, active_provider: newProvider }));
                              }}
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                            >
                              <option value="qwen">{ASR_PROVIDERS.qwen.name}</option>
                              <option value="doubao">{ASR_PROVIDERS.doubao.name}</option>
                            </select>
                          </div>
                          {asrConfig.primary.provider === 'qwen' ? (
                            <div className="space-y-2">
                              <label className="text-xs font-medium text-slate-600">API Key</label>
                              <ApiKeyInput
                                value={asrConfig.primary.api_key}
                                onChange={(value) => {
                                  setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, api_key: value } }));
                                  setAsrCache(prev => ({ ...prev, qwen: { api_key: value } }));
                                }}
                                show={showApiKey}
                                onToggleShow={() => setShowApiKey(!showApiKey)}
                                placeholder="sk-..."
                              />
                            </div>
                          ) : (
                            <>
                              <div className="space-y-2">
                                <label className="text-xs font-medium text-slate-600">APP ID</label>
                                <input
                                  type="text"
                                  value={asrConfig.primary.app_id || ''}
                                  onChange={(e) => {
                                    const value = e.target.value;
                                    setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, app_id: value } }));
                                    setAsrCache(prev => ({ ...prev, doubao: { ...prev.doubao, app_id: value } }));
                                  }}
                                  className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                                  placeholder="输入豆包 APP ID"
                                />
                              </div>
                              <div className="space-y-2">
                                <label className="text-xs font-medium text-slate-600">Access Token</label>
                                <ApiKeyInput
                                  value={asrConfig.primary.access_token || ''}
                                  onChange={(value) => {
                                    setAsrConfig(prev => ({ ...prev, primary: { ...prev.primary, access_token: value } }));
                                    setAsrCache(prev => ({ ...prev, doubao: { ...prev.doubao, access_token: value } }));
                                  }}
                                  show={showApiKey}
                                  onToggleShow={() => setShowApiKey(!showApiKey)}
                                  placeholder="输入 Access Token"
                                />
                              </div>
                            </>
                          )}
                          <div className="text-xs text-slate-500">模型：{ASR_PROVIDERS[asrConfig.primary.provider].model}</div>
                        </div>
                      </div>
                      {/* 备用模型配置 */}
                      <div className="space-y-4">
                        <div className="flex items-center justify-between">
                          <h4 className="text-sm font-bold text-slate-700">备用模型（可选）</h4>
                          <Toggle
                            checked={asrConfig.enable_fallback}
                            onCheckedChange={(next) =>
                              setAsrConfig(prev => ({
                                ...prev,
                                enable_fallback: next,
                                fallback: next && (!prev.fallback?.api_key)
                                  ? { provider: 'siliconflow', api_key: asrCache.siliconflow.api_key }
                                  : prev.fallback
                              }))
                            }
                            size="xs"
                            variant="blue"
                          />
                        </div>
                        {asrConfig.enable_fallback && (
                          <div className="space-y-3 p-4 bg-slate-50 rounded-xl border border-slate-200 animate-in slide-in-from-top-2 fade-in duration-300">
                            <div className="space-y-2">
                              <label className="text-xs font-medium text-slate-600">服务商</label>
                              <select
                                value={asrConfig.fallback?.provider || 'siliconflow'}
                                onChange={(e) => setAsrConfig(prev => ({ ...prev, fallback: { provider: e.target.value as AsrProvider, api_key: prev.fallback?.api_key || '' } }))}
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition-all"
                              >
                                <option value="siliconflow">{ASR_PROVIDERS.siliconflow.name}</option>
                              </select>
                            </div>
                            <div className="space-y-2">
                              <label className="text-xs font-medium text-slate-600">API Key</label>
                              <ApiKeyInput
                                value={asrConfig.fallback?.api_key || ''}
                                onChange={(val) => {
                                  setAsrConfig(prev => ({ ...prev, fallback: { provider: prev.fallback?.provider || 'siliconflow', api_key: val } }));
                                  setAsrCache(prev => ({ ...prev, siliconflow: { api_key: val } }));
                                }}
                                show={showApiKey}
                                onToggleShow={() => setShowApiKey(!showApiKey)}
                                placeholder="sk-..."
                              />
                            </div>
                            <div className="text-xs text-slate-500">模型：{ASR_PROVIDERS[asrConfig.fallback?.provider || 'siliconflow'].model}</div>
                            <div className="flex items-start gap-2 p-3 bg-blue-50 border border-blue-100 rounded-lg text-xs text-blue-700">
                              <AlertCircle size={14} className="mt-0.5 flex-shrink-0" />
                              <span>备用模型在主模型响应较慢时并行请求，先返回结果的模型优先使用</span>
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                  {/* LLM Tab */}
                  {serviceModalTab === 'llm' && (
                    <div className="h-full flex overflow-hidden">
                      {/* Left Sidebar: Presets List */}
                      <div className="w-1/3 bg-slate-50 border-r border-slate-200 flex flex-col">
                        <div className="p-4 border-b border-slate-200 bg-slate-50/50">
                          <div className="flex items-center gap-2 p-2 mb-3 bg-violet-50 border border-violet-100 rounded-lg text-xs text-violet-700">
                            <AlertCircle size={12} className="flex-shrink-0" />
                            <span>Ctrl+Win 听写时使用</span>
                          </div>
                          <button
                            onClick={handleAddPreset}
                            className="w-full py-2 bg-white border border-slate-200 rounded-lg text-sm text-slate-600 font-medium hover:border-violet-300 hover:text-violet-600 transition-all flex items-center justify-center gap-2 shadow-sm"
                          >
                            <Plus size={14} /> 新增预设
                          </button>
                        </div>
                        <div className="flex-1 overflow-y-auto p-2 space-y-1">
                          {llmConfig.presets.map(preset => (
                            <div
                              key={preset.id}
                              onClick={() => setLlmConfig(prev => ({ ...prev, active_preset_id: preset.id }))}
                              className={`group flex items-center justify-between p-3 rounded-xl cursor-pointer transition-all ${
                                llmConfig.active_preset_id === preset.id
                                  ? 'bg-white shadow-md border border-violet-100 ring-1 ring-violet-500/20'
                                  : 'hover:bg-slate-100 border border-transparent'
                              }`}
                            >
                              <div className="flex items-center gap-3">
                                <div className={`p-1.5 rounded-lg ${llmConfig.active_preset_id === preset.id ? 'bg-violet-100 text-violet-600' : 'bg-slate-200 text-slate-500'}`}>
                                  <MessageSquareQuote size={14} />
                                </div>
                                <span className={`text-sm font-medium ${llmConfig.active_preset_id === preset.id ? 'text-slate-900' : 'text-slate-600'}`}>
                                  {preset.name}
                                </span>
                              </div>
                              {llmConfig.presets.length > 1 && (
                                <button
                                  onClick={(e) => { e.stopPropagation(); handleDeletePreset(preset.id); }}
                                  className={`p-1.5 rounded-md text-slate-400 hover:bg-red-50 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100 ${llmConfig.active_preset_id === preset.id ? 'opacity-100' : ''}`}
                                  title="删除预设"
                                >
                                  <Trash2 size={14} />
                                </button>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                      {/* Right Content: Preset Details & Global Config */}
                      <div className="flex-1 flex flex-col bg-white overflow-hidden">
                        <div className="flex-1 overflow-y-auto p-6 space-y-6">
                          <div className="space-y-2">
                            <label className="text-sm font-medium text-slate-700">预设名称</label>
                            <input
                              type="text"
                              value={activePreset?.name || ""}
                              onChange={(e) => handleUpdateActivePreset('name', e.target.value)}
                              className="w-full px-4 py-2.5 bg-white border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all font-medium text-slate-900"
                              placeholder="例如：邮件整理"
                            />
                          </div>
                          <div className="space-y-2 flex-1 flex flex-col">
                            <div className="flex justify-between items-center">
                              <label className="text-sm font-medium text-slate-700">系统提示词 (System Prompt)</label>
                              <button
                                onClick={() => {
                                  const original = defaultPresets.find(p => p.id === activePreset.id);
                                  if(original) handleUpdateActivePreset('system_prompt', original.system_prompt);
                                }}
                                className="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 transition-colors"
                              >
                                <RotateCcw size={12} /> 恢复默认
                              </button>
                            </div>
                            <textarea
                              value={activePreset?.system_prompt || ""}
                              onChange={(e) => handleUpdateActivePreset('system_prompt', e.target.value)}
                              className="w-full flex-1 min-h-[150px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/20 focus:border-violet-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                              placeholder="在这里定义 AI 的行为..."
                            />
                          </div>
                          <div className="h-px bg-slate-100 my-4"></div>
                          <div className="space-y-4">
                            <h4 className="text-xs font-bold text-slate-400 uppercase tracking-wider">模型设置</h4>
                            <div className="grid grid-cols-2 gap-4">
                              <div className="col-span-2 space-y-1.5">
                                <label className="text-xs font-medium text-slate-500">API Key</label>
                                <ApiKeyInput
                                  value={llmConfig.api_key}
                                  onChange={(value) => setLlmConfig({ ...llmConfig, api_key: value })}
                                  show={showApiKey}
                                  onToggleShow={() => setShowApiKey(!showApiKey)}
                                  placeholder="sk-..."
                                  inputClassName="bg-slate-50 text-xs focus:ring-0 focus:border-violet-500"
                                />
                              </div>
                              <div className="space-y-1.5">
                                <label className="text-xs font-medium text-slate-500">模型名称</label>
                                <input
                                  type="text"
                                  value={llmConfig.model}
                                  onChange={(e) => setLlmConfig({ ...llmConfig, model: e.target.value })}
                                  className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                                  placeholder="glm-4-flash"
                                />
                              </div>
                              <div className="space-y-1.5">
                                <label className="text-xs font-medium text-slate-500">API 地址</label>
                                <input
                                  type="text"
                                  value={llmConfig.endpoint}
                                  onChange={(e) => setLlmConfig({ ...llmConfig, endpoint: e.target.value })}
                                  className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs focus:outline-none focus:border-violet-500 transition-all"
                                  placeholder="https://api..."
                                />
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  )}
                  {/* AI Assistant Tab */}
                  {serviceModalTab === 'assistant' && (
                    <div className="h-full overflow-y-auto p-6 space-y-6">
                      <div className="flex items-center gap-2 p-3 bg-emerald-50 border border-emerald-100 rounded-xl text-xs text-emerald-700">
                        <AlertCircle size={14} className="flex-shrink-0" />
                        <span>AI 助手 ({formatHotkeyDisplay(dualHotkeyConfig.assistant)}) 可智能处理选中文本或回答问题，无需开关，配置 API 即可使用</span>
                      </div>
                      {/* 模型配置 */}
                      <div className="space-y-4">
                        <h4 className="text-sm font-bold text-slate-700">模型配置</h4>
                        <div className="space-y-4 p-4 bg-slate-50 rounded-xl border border-slate-200">
                          <div className="space-y-2">
                            <label className="text-xs font-medium text-slate-600">API Key</label>
                            <ApiKeyInput
                              value={assistantConfig.api_key}
                              onChange={(value) => setAssistantConfig(prev => ({ ...prev, api_key: value }))}
                              show={showApiKey}
                              onToggleShow={() => setShowApiKey(!showApiKey)}
                              placeholder="sk-..."
                              inputClassName="focus:ring-emerald-500/20 focus:border-emerald-500"
                            />
                          </div>
                          <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                              <label className="text-xs font-medium text-slate-600">模型名称</label>
                              <input
                                type="text"
                                value={assistantConfig.model}
                                onChange={(e) => setAssistantConfig(prev => ({ ...prev, model: e.target.value }))}
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all"
                                placeholder="glm-4-flash"
                              />
                            </div>
                            <div className="space-y-2">
                              <label className="text-xs font-medium text-slate-600">API 地址</label>
                              <input
                                type="text"
                                value={assistantConfig.endpoint}
                                onChange={(e) => setAssistantConfig(prev => ({ ...prev, endpoint: e.target.value }))}
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all"
                                placeholder="https://api..."
                              />
                            </div>
                          </div>
                        </div>
                      </div>
                      {/* 问答模式提示词 */}
                      <div className="space-y-4">
                        <h4 className="text-sm font-bold text-slate-700">问答模式提示词</h4>
                        <p className="text-xs text-slate-500">无选中文本时，AI 助手将使用此提示词回答问题</p>
                        <textarea
                          value={assistantConfig.qa_system_prompt}
                          onChange={(e) => setAssistantConfig(prev => ({ ...prev, qa_system_prompt: e.target.value }))}
                          className="w-full min-h-[120px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                          placeholder="定义 AI 助手如何回答问题..."
                        />
                      </div>
                      {/* 文本处理提示词 */}
                      <div className="space-y-4">
                        <h4 className="text-sm font-bold text-slate-700">文本处理提示词</h4>
                        <p className="text-xs text-slate-500">有选中文本时，AI 助手将使用此提示词处理文本（翻译、润色、总结等）</p>
                        <textarea
                          value={assistantConfig.text_processing_system_prompt}
                          onChange={(e) => setAssistantConfig(prev => ({ ...prev, text_processing_system_prompt: e.target.value }))}
                          className="w-full min-h-[120px] p-4 bg-slate-50 border border-slate-200 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 transition-all resize-none font-mono text-slate-600 leading-relaxed"
                          placeholder="定义 AI 助手如何处理选中的文本..."
                        />
                      </div>
                    </div>
                  )}
                  {/* Dictionary Tab */}
                  {serviceModalTab === 'dictionary' && (
                    <div className="h-full overflow-y-auto p-6 space-y-6">
                      {/* 提示信息 */}
                      <div className="flex items-center gap-2 p-3 bg-orange-50 border border-orange-100 rounded-xl text-xs text-orange-700">
                        <AlertCircle size={14} className="flex-shrink-0" />
                        <span>添加常用词汇（专业术语、人名、产品名等），提升语音识别准确率</span>
                      </div>
                      {/* 添加词条 */}
                      <div className="space-y-2">
                        <div className="flex gap-2">
                          <input
                            type="text"
                            value={newWord}
                            onChange={(e) => { setNewWord(e.target.value); setDuplicateHint(false); }}
                            onKeyDown={(e) => e.key === 'Enter' && handleAddWord()}
                            className={`flex-1 px-4 py-2.5 bg-white border rounded-xl text-sm focus:outline-none focus:ring-2 transition-all ${
                              duplicateHint
                                ? 'border-red-300 focus:ring-red-500/20 focus:border-red-500'
                                : 'border-slate-200 focus:ring-orange-500/20 focus:border-orange-500'
                            }`}
                            placeholder="输入词汇，按回车添加"
                          />
                          <button
                            onClick={handleAddWord}
                            disabled={!newWord.trim()}
                            className="px-4 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-xl hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                          >
                            <Plus size={16} />
                            添加
                          </button>
                        </div>
                        {duplicateHint && (
                          <p className="text-xs text-red-500 pl-1">该词汇已存在</p>
                        )}
                      </div>
                      {/* 词条列表 - 气囊/标签云布局 */}
                      {dictionary.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-slate-300 space-y-3">
                          <BookText size={48} strokeWidth={1} />
                          <p className="text-sm text-slate-400">暂无词条</p>
                          <p className="text-xs text-slate-300">添加常用词汇，让语音识别更准确</p>
                        </div>
                      ) : (
                        <div>
                          <div className="text-xs text-slate-500 mb-3">共 {dictionary.length} 个词条</div>
                          <div className="flex flex-wrap gap-2">
                            {dictionary.map((word, index) => (
                              editingIndex === index ? (
                                /* 编辑模式 */
                                <div key={index} className="flex items-center gap-1 px-2 py-1 bg-white border-2 border-orange-400 rounded-full shadow-sm">
                                  <input
                                    type="text"
                                    value={editingValue}
                                    onChange={(e) => setEditingValue(e.target.value)}
                                    onKeyDown={(e) => {
                                      if (e.key === 'Enter') handleSaveEdit();
                                      if (e.key === 'Escape') handleCancelEdit();
                                    }}
                                    className="w-24 px-2 py-0.5 bg-transparent text-sm focus:outline-none text-slate-700"
                                    autoFocus
                                  />
                                  <button onClick={handleSaveEdit} className="p-0.5 text-emerald-600 hover:text-emerald-700" title="保存">
                                    <CheckCircle2 size={14} />
                                  </button>
                                  <button onClick={handleCancelEdit} className="p-0.5 text-slate-400 hover:text-slate-600" title="取消">
                                    <X size={14} />
                                  </button>
                                </div>
                              ) : (
                                /* 显示模式 - 气囊样式 */
                                <div
                                  key={index}
                                  className="group flex items-center gap-1.5 px-3 py-1.5 bg-white border border-slate-200 rounded-full text-sm text-slate-700 hover:border-orange-300 hover:shadow-sm transition-all cursor-default"
                                >
                                  <span className="font-medium" onDoubleClick={() => handleStartEdit(index)}>{word}</span>
                                  <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                                    <button
                                      onClick={() => handleStartEdit(index)}
                                      className="p-0.5 text-slate-400 hover:text-orange-500 transition-colors"
                                      title="编辑"
                                    >
                                      <Wand2 size={12} />
                                    </button>
                                    <button
                                      onClick={() => handleDeleteWord(index)}
                                      className="p-0.5 text-slate-400 hover:text-red-500 transition-colors"
                                      title="删除"
                                    >
                                      <X size={12} />
                                    </button>
                                  </div>
                                </div>
                              )
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
                {/* Modal Footer */}
                <div className="px-6 py-4 border-t border-[var(--stone)] bg-[var(--paper)] flex items-center justify-end gap-3">
                  <button
                    onClick={() => onClose()}
                    className="px-5 py-2.5 text-sm font-medium text-slate-600 hover:text-slate-800 hover:bg-slate-100 rounded-xl transition-colors"
                  >
                    关闭
                  </button>
                  <button
                    onClick={() => {
                      handleSaveConfig();
                      onClose();
                    }}
                    className={`px-5 py-2.5 text-sm font-medium text-white rounded-xl shadow-lg transition-all ${
                      serviceModalTab === 'asr' ? 'bg-blue-500 hover:bg-blue-600 shadow-blue-500/20' :
                      serviceModalTab === 'llm' ? 'bg-violet-500 hover:bg-violet-600 shadow-violet-500/20' :
                      serviceModalTab === 'assistant' ? 'bg-emerald-500 hover:bg-emerald-600 shadow-emerald-500/20' :
                      'bg-orange-500 hover:bg-orange-600 shadow-orange-500/20'
                    }`}
                  >
                    保存并应用
                  </button>
                </div>
              </div>
            </div>

  );
}
