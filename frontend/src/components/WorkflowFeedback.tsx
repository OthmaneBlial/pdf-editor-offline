import type { ReactNode } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  CircleX,
  Info,
  LoaderCircle,
} from 'lucide-react';

export type WorkflowFeedbackTone = 'info' | 'progress' | 'success' | 'warning' | 'error';

interface WorkflowFeedbackProps {
  tone?: WorkflowFeedbackTone;
  title?: string;
  children: ReactNode;
  progress?: number;
  progressLabel?: string;
  actions?: ReactNode;
  className?: string;
}

const toneStyles: Record<WorkflowFeedbackTone, string> = {
  info: 'border-sky-300/50 bg-sky-50 text-sky-950',
  progress: 'border-violet-300/50 bg-violet-50 text-violet-950',
  success: 'border-emerald-300/60 bg-emerald-50 text-emerald-950',
  warning: 'border-amber-300/60 bg-amber-50 text-amber-950',
  error: 'border-rose-300/60 bg-rose-50 text-rose-950',
};

const toneIcons = {
  info: Info,
  progress: LoaderCircle,
  success: CheckCircle2,
  warning: AlertTriangle,
  error: CircleX,
};

export default function WorkflowFeedback({
  tone = 'info',
  title,
  children,
  progress,
  progressLabel = 'Operation progress',
  actions,
  className = '',
}: WorkflowFeedbackProps) {
  const Icon = toneIcons[tone];
  const boundedProgress = progress == null ? null : Math.min(100, Math.max(0, progress));
  const urgent = tone === 'error';

  return (
    <section
      role={urgent ? 'alert' : 'status'}
      aria-live={urgent ? 'assertive' : 'polite'}
      aria-atomic="true"
      className={`rounded-2xl border p-4 text-sm shadow-sm ${toneStyles[tone]} ${className}`}
    >
      <div className="flex items-start gap-3">
        <Icon className={`mt-0.5 h-5 w-5 shrink-0 ${tone === 'progress' ? 'animate-spin motion-reduce:animate-none' : ''}`} aria-hidden="true" />
        <div className="min-w-0 flex-1">
          {title && <h3 className="font-black">{title}</h3>}
          <div className={`${title ? 'mt-1' : ''} leading-5`}>{children}</div>
          {boundedProgress != null && (
            <div className="mt-3">
              <div className="mb-1 flex items-center justify-between gap-3 font-mono text-[10px] font-bold uppercase tracking-wider">
                <span>{progressLabel}</span>
                <span>{boundedProgress}%</span>
              </div>
              <div role="progressbar" aria-label={progressLabel} aria-valuemin={0} aria-valuemax={100} aria-valuenow={boundedProgress} className="h-2 overflow-hidden rounded-full bg-black/10">
                <div className="h-full rounded-full bg-current transition-[width] motion-reduce:transition-none" style={{ width: `${boundedProgress}%` }} />
              </div>
            </div>
          )}
          {actions && <div className="mt-3 flex flex-wrap gap-2">{actions}</div>}
        </div>
      </div>
    </section>
  );
}
