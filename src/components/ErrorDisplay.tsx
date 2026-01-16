// src/components/ErrorDisplay.tsx
// 错误展示组件 - 显示用户友好的错误信息

import { useState, useMemo, useEffect } from 'react';
import { ScrollText, X } from 'lucide-react';
import { parseError, ERROR_ICONS } from '../utils/errorParser';

interface ErrorDisplayProps {
  error: string | null;
  onClose: () => void;
}

export const ErrorDisplay: React.FC<ErrorDisplayProps> = ({ error, onClose }) => {
  const [showDetails, setShowDetails] = useState(false);

  // 使用 useMemo 避免每次渲染重新解析
  const friendlyError = useMemo(() => {
    return error ? parseError(error) : null;
  }, [error]);

  // 错误变化时自动收起详情
  useEffect(() => {
    setShowDetails(false);
  }, [error]);

  if (!friendlyError) {
    return null;
  }

  return (
    <div className="bg-red-50/80 border border-red-100 rounded-2xl text-red-600 text-sm animate-in slide-in-from-top-2 fade-in duration-300 overflow-hidden">
      <div className="flex items-start gap-3 p-4">
        <span className="text-lg flex-shrink-0">
          {ERROR_ICONS[friendlyError.category]}
        </span>
        <div className="flex-1 min-w-0">
          <div className="font-medium text-red-700">{friendlyError.title}</div>
          <div className="text-red-500 text-xs mt-0.5">{friendlyError.suggestion}</div>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={() => setShowDetails(!showDetails)}
            className="p-1.5 hover:bg-red-100 rounded-lg transition-colors text-red-400 hover:text-red-600"
            title={showDetails ? '收起详情' : '查看详情'}
          >
            <ScrollText size={14} />
          </button>
          <button
            onClick={onClose}
            className="p-1.5 hover:bg-red-100 rounded-lg transition-colors text-red-400 hover:text-red-600"
            title="关闭"
          >
            <X size={14} />
          </button>
        </div>
      </div>
      {showDetails && (
        <div className="px-4 pb-4 pt-0">
          <div className="bg-red-100/50 rounded-lg p-3 text-xs text-red-600/80 font-mono break-all">
            {friendlyError.details}
          </div>
        </div>
      )}
    </div>
  );
};
