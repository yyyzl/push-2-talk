// 差异对比显示组件
import { ArrowRight } from 'lucide-react';

interface DiffDisplayProps {
  original: string;
  corrected: string;
}

export function DiffDisplay({ original, corrected }: DiffDisplayProps) {
  return (
    <div className="flex items-center gap-2 text-sm font-mono flex-wrap">
      <span className="line-through text-red-400 opacity-80 decoration-2 decoration-red-200">
        {original}
      </span>
      <ArrowRight size={14} className="text-stone-300" />
      <span className="font-bold text-[var(--sage)] bg-[rgba(120,140,93,0.12)] px-1.5 py-0.5 rounded">
        {corrected}
      </span>
    </div>
  );
}
