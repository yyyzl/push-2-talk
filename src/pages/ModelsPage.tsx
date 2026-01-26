import { useState } from "react";
import { Plus, Trash2, Edit2, Settings2, Globe, Check, Zap, Sparkles, MessageSquare, GraduationCap } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";
import type { SharedLlmConfig, LlmProvider } from "../types";
import { ApiKeyInput, Modal, ConfigSelect } from "../components/common";

export type ModelsPageProps = {
  sharedConfig: SharedLlmConfig;
  setSharedConfig: Dispatch<SetStateAction<SharedLlmConfig>>;
  showApiKey: boolean;
  setShowApiKey: (next: boolean) => void;
  isRunning: boolean;
};

// 使用 crypto.randomUUID() 生成安全的唯一 ID
const generateId = () => crypto.randomUUID().substring(0, 12);

export function ModelsPage({
  sharedConfig,
  setSharedConfig,
  showApiKey,
  setShowApiKey,
  isRunning,
}: ModelsPageProps) {
  const [editingProvider, setEditingProvider] = useState<LlmProvider | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  // 删除确认弹窗状态
  const [deleteConfirm, setDeleteConfirm] = useState<{ show: boolean; providerId: string | null }>({
    show: false,
    providerId: null,
  });

  // 注意：前端不再处理迁移逻辑，由后端 config.rs 统一处理
  // 后端 load() 会检测旧配置并自动迁移到 Provider Registry

  const handleSaveProvider = () => {
    if (!editingProvider) return;

    // 简单的校验
    if (!editingProvider.name) return;

    setSharedConfig(prev => {
      const exists = prev.providers.some(p => p.id === editingProvider.id);
      let newProviders;
      if (exists) {
        newProviders = prev.providers.map(p => p.id === editingProvider.id ? editingProvider : p);
      } else {
        newProviders = [...prev.providers, editingProvider];
      }

      // 如果是第一个添加的，设为默认
      const newDefaultId = prev.providers.length === 0 ? editingProvider.id : prev.default_provider_id;

      return {
        ...prev,
        providers: newProviders,
        default_provider_id: newDefaultId,
      };
    });

    setIsModalOpen(false);
    setEditingProvider(null);
  };

  const handleDeleteProvider = (id: string) => {
    if (sharedConfig.providers.length <= 1) {
      // 使用状态替代 alert（这里可以用 toast 或临时提示）
      setDeleteConfirm({ show: true, providerId: null }); // 显示错误提示
      setTimeout(() => setDeleteConfirm({ show: false, providerId: null }), 2000);
      return;
    }

    // 显示确认弹窗
    setDeleteConfirm({ show: true, providerId: id });
  };

  const confirmDelete = () => {
    const id = deleteConfirm.providerId;
    if (!id) {
      setDeleteConfirm({ show: false, providerId: null });
      return;
    }

    setSharedConfig(prev => {
      const newProviders = prev.providers.filter(p => p.id !== id);
      // 如果删除了默认的，重置为第一个
      let newDefaultId = prev.default_provider_id;
      if (id === prev.default_provider_id) {
        newDefaultId = newProviders[0]?.id || "";
      }
      return {
        ...prev,
        providers: newProviders,
        default_provider_id: newDefaultId,
        // 清理绑定
        polishing_provider_id: prev.polishing_provider_id === id ? undefined : prev.polishing_provider_id,
        assistant_provider_id: prev.assistant_provider_id === id ? undefined : prev.assistant_provider_id,
        learning_provider_id: prev.learning_provider_id === id ? undefined : prev.learning_provider_id,
      };
    });
    setDeleteConfirm({ show: false, providerId: null });
  };

  const openAddModal = () => {
    setEditingProvider({
      id: generateId(),
      name: "",
      endpoint: "",
      api_key: "",
      default_model: "",
    });
    setIsModalOpen(true);
  };

  const openEditModal = (provider: LlmProvider) => {
    setEditingProvider({ ...provider });
    setIsModalOpen(true);
  };

  const providerOptions = sharedConfig.providers.map(p => ({ value: p.id, label: p.name }));

  // 表单验证
  const isFormValid = editingProvider &&
    editingProvider.name.trim() !== '' &&
    editingProvider.endpoint.trim() !== '' &&
    editingProvider.api_key.trim() !== '';

  return (
    <div className="mx-auto max-w-5xl space-y-8 font-sans pb-20">

      {/* 顶部：功能绑定 */}
      <section className="bg-white/80 backdrop-blur-sm border border-[var(--stone)] rounded-3xl p-8 shadow-sm">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 bg-stone-100 rounded-xl text-[var(--ink)]">
            <Settings2 size={20} />
          </div>
          <div>
            <h2 className="text-base font-bold text-[var(--ink)]">功能默认绑定</h2>
            <p className="text-xs text-stone-500 mt-0.5">为不同的 AI 助手功能指定默认的 LLM 提供商</p>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {/* Default Provider Card */}
          <div className="relative group bg-[var(--paper)] rounded-2xl p-4 border border-[var(--stone)] hover:border-[var(--steel)] transition-all hover:shadow-md">
            <div className="flex items-center gap-2 mb-3">
              <Zap size={16} className="text-amber-500" />
              <label className="text-xs font-bold text-stone-600 uppercase tracking-widest">默认提供商</label>
            </div>
            <ConfigSelect
              value={sharedConfig.default_provider_id}
              onChange={(val) => setSharedConfig(prev => ({ ...prev, default_provider_id: val }))}
              options={providerOptions}
              disabled={isRunning || sharedConfig.providers.length === 0}
            />
            <p className="text-[10px] text-stone-400 mt-2">
              未指定功能时的兜底选择
            </p>
          </div>

          {/* Polishing Provider Card */}
          <div className="relative group bg-[var(--paper)] rounded-2xl p-4 border border-[var(--stone)] hover:border-[var(--steel)] transition-all hover:shadow-md">
            <div className="flex items-center gap-2 mb-3">
              <Sparkles size={16} className="text-purple-500" />
              <label className="text-xs font-bold text-stone-600 uppercase tracking-widest">语句润色</label>
            </div>
            <ConfigSelect
              value={sharedConfig.polishing_provider_id || ""}
              onChange={(val) => setSharedConfig(prev => ({ ...prev, polishing_provider_id: val || undefined }))}
              options={[{ value: "", label: "跟随默认" }, ...providerOptions]}
              disabled={isRunning}
            />
          </div>

          {/* Assistant Provider Card */}
          <div className="relative group bg-[var(--paper)] rounded-2xl p-4 border border-[var(--stone)] hover:border-[var(--steel)] transition-all hover:shadow-md">
            <div className="flex items-center gap-2 mb-3">
              <MessageSquare size={16} className="text-sky-500" />
              <label className="text-xs font-bold text-stone-600 uppercase tracking-widest">AI 助手</label>
            </div>
            <ConfigSelect
              value={sharedConfig.assistant_provider_id || ""}
              onChange={(val) => setSharedConfig(prev => ({ ...prev, assistant_provider_id: val || undefined }))}
              options={[{ value: "", label: "跟随默认" }, ...providerOptions]}
              disabled={isRunning}
            />
          </div>

          {/* Learning Provider Card (Newly Added) */}
          <div className="relative group bg-[var(--paper)] rounded-2xl p-4 border border-[var(--stone)] hover:border-[var(--steel)] transition-all hover:shadow-md">
            <div className="flex items-center gap-2 mb-3">
              <GraduationCap size={16} className="text-[var(--sage)]" />
              <label className="text-xs font-bold text-stone-600 uppercase tracking-widest">词库学习</label>
            </div>
            <ConfigSelect
              value={sharedConfig.learning_provider_id || ""}
              onChange={(val) => setSharedConfig(prev => ({ ...prev, learning_provider_id: val || undefined }))}
              options={[{ value: "", label: "跟随默认" }, ...providerOptions]}
              disabled={isRunning}
            />
          </div>
        </div>
      </section>

      {/* 列表区域 */}
      <section className="space-y-6">
        <div className="flex items-center justify-between px-2">
          <div>
            <h2 className="text-xl font-bold text-[var(--ink)]">提供商列表</h2>
            <p className="text-sm text-stone-500 mt-1">管理你的 LLM 模型 API 连接配置</p>
          </div>

          <button
            onClick={openAddModal}
            disabled={isRunning}
            className="flex items-center gap-2 px-5 py-2.5 bg-[var(--ink)] text-white/90 rounded-xl text-sm font-bold hover:bg-stone-800 hover:text-white hover:shadow-lg hover:-translate-y-0.5 transition-all disabled:opacity-50 disabled:hover:translate-y-0 disabled:hover:shadow-none"
          >
            <Plus size={18} />
            添加提供商
          </button>
        </div>

        {sharedConfig.providers.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 bg-white/50 border-2 border-dashed border-[var(--stone)] rounded-3xl text-stone-400 group cursor-pointer hover:border-[var(--steel)] hover:bg-white transition-all" onClick={openAddModal}>
            <div className="bg-stone-100 p-4 rounded-full mb-4 group-hover:scale-110 transition-transform">
              <Globe size={32} className="opacity-40 text-[var(--ink)]" />
            </div>
            <p className="text-base font-bold text-stone-500">暂无提供商</p>
            <p className="text-sm">支持 OpenAI、DeepSeek、智谱等所有兼容 OpenAI 格式的 API</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
            {sharedConfig.providers.map(provider => {
              const isDefault = provider.id === sharedConfig.default_provider_id;
              // 检查是否有任何功能绑定到此 Provider
              const isUsed = isDefault ||
                [sharedConfig.polishing_provider_id, sharedConfig.assistant_provider_id, sharedConfig.learning_provider_id].includes(provider.id);

              return (
                <div key={provider.id} className="group relative flex flex-col justify-between bg-white border border-[var(--stone)] rounded-2xl p-6 hover:border-[var(--steel)] hover:shadow-lg transition-all duration-300">
                  {/* Card Header */}
                  <div className="flex justify-between items-start mb-4">
                    <div className="flex items-center gap-4">

                      <div>
                        <h3 className="text-base font-bold text-[var(--ink)] leading-tight">{provider.name}</h3>
                        <div className="flex items-center gap-2 mt-1.5">
                          {isDefault && (
                            <span className="text-[10px] font-bold px-2 py-0.5 bg-amber-50 text-amber-600 rounded-full border border-amber-100 flex items-center gap-1">
                              <Check size={8} strokeWidth={4} /> 默认
                            </span>
                          )}
                          {!isDefault && isUsed && (
                            <span className="text-[10px] font-bold px-2 py-0.5 bg-sky-50 text-sky-600 rounded-full border border-sky-100">
                              使用中
                            </span>
                          )}
                        </div>
                      </div>
                    </div>

                    {/* Action Menu - Always visible on desktop for easier access, but designed subtly */}
                    <div className="flex gap-1">
                      <button
                        onClick={() => openEditModal(provider)}
                        disabled={isRunning}
                        className="p-2 text-stone-400 hover:text-[var(--ink)] hover:bg-stone-100 rounded-lg transition-all"
                        title="编辑"
                      >
                        <Edit2 size={16} />
                      </button>
                      {sharedConfig.providers.length > 1 && (
                        <button
                          onClick={() => handleDeleteProvider(provider.id)}
                          disabled={isRunning}
                          className="p-2 text-stone-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-all"
                          title="删除"
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Card Body - Tech Details */}
                  <div className="space-y-2 mt-2 pt-4 border-t border-[var(--sep)]">
                    <div className="grid grid-cols-[auto_1fr] gap-2 items-center text-xs">
                      <span className="font-bold text-stone-400 uppercase tracking-tight">Model</span>
                      <span className="font-mono text-[var(--ink)] bg-stone-100/50 px-2 py-1 rounded truncate">{provider.default_model}</span>
                    </div>
                    <div className="grid grid-cols-[auto_1fr] gap-2 items-center text-xs">
                      <span className="font-bold text-stone-400 uppercase tracking-tight">Endpoint</span>
                      <span className="font-mono text-stone-500 truncate" title={provider.endpoint}>{provider.endpoint}</span>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* 编辑/添加 Modal */}
      <Modal
        open={isModalOpen}
        onClose={() => setIsModalOpen(false)}
      >
        {editingProvider && (
          <div className="bg-white rounded-3xl overflow-hidden shadow-2xl max-w-2xl w-full">
            {/* Header */}
            <div className="px-8 py-6 border-b border-[var(--stone)] bg-stone-50/50">
              <h2 className="text-xl font-bold text-[var(--ink)] flex items-center gap-3">
                {editingProvider.id && sharedConfig.providers.some(p => p.id === editingProvider.id) ? (
                  <>
                    <div className="p-2 bg-white border border-[var(--stone)] rounded-lg shadow-sm text-[var(--ink)]"><Edit2 size={18} /></div>
                    编辑提供商
                  </>
                ) : (
                  <>
                    <div className="p-2 bg-[var(--ink)] text-white rounded-lg shadow-sm"><Plus size={18} /></div>
                    添加提供商
                  </>
                )}
              </h2>
              <p className="text-sm text-stone-500 mt-2 ml-11">配置标准的 OpenAI 兼容 API 接口</p>
            </div>

            {/* Body */}
            <div className="p-8 space-y-6 max-h-[60vh] overflow-y-auto">
              <div className="space-y-2">
                <label className="text-sm font-bold text-[var(--ink)] ml-1">提供商名称</label>
                <input
                  type="text"
                  value={editingProvider.name}
                  autoFocus
                  onChange={e => setEditingProvider({ ...editingProvider, name: e.target.value })}
                  className="w-full px-4 py-3 bg-[var(--paper)] border-2 border-transparent focus:bg-white focus:border-[var(--steel)] rounded-xl text-sm transition-all focus:outline-none placeholder:text-stone-300"
                  placeholder="例如：DeepSeek, OpenAI, 智谱 AI"
                />
                <p className="text-xs text-stone-400 ml-1">起个好记的名字，方便后续选择</p>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="space-y-2">
                  <label className="text-sm font-bold text-[var(--ink)] ml-1">模型代码 (Model)</label>
                  <input
                    type="text"
                    value={editingProvider.default_model}
                    onChange={e => setEditingProvider({ ...editingProvider, default_model: e.target.value })}
                    className="w-full px-4 py-3 bg-[var(--paper)] border-2 border-transparent focus:bg-white focus:border-[var(--steel)] rounded-xl text-sm transition-all focus:outline-none placeholder:text-stone-300 font-mono"
                    placeholder="如: gpt-4o, deepseek-chat"
                  />
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-bold text-[var(--ink)] ml-1">API 端点 (Endpoint)</label>
                  <input
                    type="text"
                    value={editingProvider.endpoint}
                    onChange={e => setEditingProvider({ ...editingProvider, endpoint: e.target.value })}
                    className="w-full px-4 py-3 bg-[var(--paper)] border-2 border-transparent focus:bg-white focus:border-[var(--steel)] rounded-xl text-sm transition-all focus:outline-none placeholder:text-stone-300 font-mono"
                    placeholder="https://api.example.com/v1"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-bold text-[var(--ink)] ml-1">API Key</label>
                <ApiKeyInput
                  value={editingProvider.api_key}
                  onChange={val => setEditingProvider({ ...editingProvider, api_key: val })}
                  show={showApiKey}
                  onToggleShow={() => setShowApiKey(!showApiKey)}
                />
                <p className="text-xs text-stone-400 ml-1">密钥将安全存储在本地</p>
              </div>
            </div>

            {/* Footer */}
            <div className="px-8 py-5 border-t border-[var(--stone)] bg-stone-50/50 flex justify-end gap-3">
              <button
                onClick={() => setIsModalOpen(false)}
                className="px-6 py-2.5 text-sm font-bold text-stone-600 hover:bg-stone-200/50 rounded-xl transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleSaveProvider}
                disabled={!isFormValid}
                className="px-8 py-2.5 text-sm font-bold text-white bg-[var(--ink)] hover:bg-stone-800 hover:shadow-lg hover:-translate-y-0.5 rounded-xl transition-all shadow-md disabled:opacity-50 disabled:shadow-none disabled:transform-none"
              >
                保存配置
              </button>
            </div>
          </div>
        )}
      </Modal>

      {/* 删除确认弹窗 */}
      <Modal
        open={deleteConfirm.show}
        onClose={() => setDeleteConfirm({ show: false, providerId: null })}
      >
        <div className="bg-white rounded-2xl overflow-hidden shadow-2xl max-w-sm w-full">
          <div className="p-6 text-center">
            {deleteConfirm.providerId ? (
              <>
                <div className="w-12 h-12 mx-auto mb-4 bg-red-100 rounded-full flex items-center justify-center">
                  <Trash2 size={24} className="text-red-500" />
                </div>
                <h3 className="text-lg font-bold text-[var(--ink)] mb-2">确认删除</h3>
                <p className="text-sm text-stone-500 mb-6">确定要删除这个提供商吗？此操作不可撤销。</p>
                <div className="flex gap-3 justify-center">
                  <button
                    onClick={() => setDeleteConfirm({ show: false, providerId: null })}
                    className="px-5 py-2 text-sm font-bold text-stone-600 hover:bg-stone-100 rounded-xl transition-colors"
                  >
                    取消
                  </button>
                  <button
                    onClick={confirmDelete}
                    className="px-5 py-2 text-sm font-bold text-white bg-red-500 hover:bg-red-600 rounded-xl transition-colors"
                  >
                    确认删除
                  </button>
                </div>
              </>
            ) : (
              <>
                <div className="w-12 h-12 mx-auto mb-4 bg-amber-100 rounded-full flex items-center justify-center">
                  <Settings2 size={24} className="text-amber-500" />
                </div>
                <h3 className="text-lg font-bold text-[var(--ink)] mb-2">无法删除</h3>
                <p className="text-sm text-stone-500">至少保留一个提供商</p>
              </>
            )}
          </div>
        </div>
      </Modal>
    </div>
  );
}