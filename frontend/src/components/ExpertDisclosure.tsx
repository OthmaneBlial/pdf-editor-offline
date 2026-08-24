import type { ReactNode } from 'react';
import { ChevronDown, SlidersHorizontal } from 'lucide-react';

interface ExpertDisclosureProps {
  title: string;
  summary: string;
  children: ReactNode;
  className?: string;
  defaultOpen?: boolean;
  tone?: 'light' | 'dark';
}

export default function ExpertDisclosure({
  title,
  summary,
  children,
  className = '',
  defaultOpen = false,
  tone = 'light',
}: ExpertDisclosureProps) {
  const dark = tone === 'dark';

  return (
    <details
      open={defaultOpen || undefined}
      className={`group overflow-hidden rounded-2xl border ${dark ? 'border-white/15 bg-black/20 text-white' : 'border-slate-200 bg-white text-slate-950'} ${className}`}
    >
      <summary className={`touch-target flex min-h-12 cursor-pointer list-none items-center gap-3 px-4 py-3 focus-visible:outline-none ${dark ? 'hover:bg-white/5' : 'hover:bg-slate-50'} [&::-webkit-details-marker]:hidden`}>
        <SlidersHorizontal className={`h-4 w-4 shrink-0 ${dark ? 'text-cyan-300' : 'text-sky-700'}`} aria-hidden="true" />
        <span className="min-w-0 flex-1">
          <strong className="block text-sm">{title}</strong>
          <span className={`mt-0.5 block text-xs ${dark ? 'text-slate-400' : 'text-slate-500'}`}>{summary}</span>
        </span>
        <span className={`rounded-full px-2 py-1 font-mono text-[9px] uppercase tracking-wider ${dark ? 'bg-white/5 text-slate-400' : 'bg-slate-100 text-slate-500'}`}>Optional</span>
        <ChevronDown className="h-4 w-4 shrink-0 transition-transform group-open:rotate-180" aria-hidden="true" />
      </summary>
      <div className={`border-t p-4 ${dark ? 'border-white/10' : 'border-slate-200'}`}>
        {children}
      </div>
    </details>
  );
}
