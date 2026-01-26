// 词典来源徽章组件
import { User, Sparkles } from 'lucide-react';
import type { DictionaryEntry } from '../../types';

interface SourceBadgeProps {
  source: DictionaryEntry['source'];
}

export function SourceBadge({ source }: SourceBadgeProps) {
  const isAuto = source === 'auto';

  return (
    <span
      className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider ${
        isAuto
          ? 'bg-[rgba(120,140,93,0.12)] text-[var(--sage)]'
          : 'bg-[rgba(106,155,204,0.12)] text-[var(--steel)]'
      }`}
    >
      {isAuto ? <Sparkles size={10} /> : <User size={10} />}
      {isAuto ? '自动' : '手动'}
    </span>
  );
}
