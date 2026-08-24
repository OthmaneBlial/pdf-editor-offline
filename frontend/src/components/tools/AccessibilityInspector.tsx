import { useEffect, useMemo, useState } from 'react';
import axios from 'axios';
import {
  AlertTriangle,
  ArrowRight,
  Check,
  CircleHelp,
  Eye,
  FileWarning,
  Languages,
  ListTree,
  Loader2,
  RefreshCw,
  ScanSearch,
  ShieldAlert,
} from 'lucide-react';

import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';

type CheckStatus = 'pass' | 'needs_attention' | 'manual_review' | 'not_applicable';

type AccessibilityCheck = {
  id: string;
  title: string;
  status: CheckStatus;
  severity: 'high' | 'medium' | 'low' | 'info';
  summary: string;
  count: number;
  page_hints: number[];
  guidance: string[];
};

type AccessibilityReport = {
  audit_sha256: string;
  automated_remediation: false;
  pdf_ua_conformance_claim: false;
  summary: {
    status: 'needs_attention' | 'manual_review';
    total_pages: number;
    pages_scanned: number;
    partial: boolean;
    checks_passed: number;
    checks_needing_attention: number;
    checks_requiring_manual_review: number;
    high_priority_issues: number;
  };
  inventory: {
    language: { present: boolean; valid_format: boolean; value: string | null };
    tags: { present: boolean; elements: number };
    forms: { fields: number; labeled_fields: number; unlabeled_fields: number };
    images: { page_images: number; tagged_figures: number; figures_with_alt_text: number };
  };
  checks: AccessibilityCheck[];
};

const statusStyles: Record<CheckStatus, string> = {
  pass: 'border-emerald-300/30 bg-emerald-300/10 text-emerald-200',
  needs_attention: 'border-rose-300/30 bg-rose-300/10 text-rose-100',
  manual_review: 'border-amber-300/30 bg-amber-300/10 text-amber-100',
  not_applicable: 'border-slate-700 bg-slate-900 text-slate-400',
};

const statusLabels: Record<CheckStatus, string> = {
  pass: 'Evidence found',
  needs_attention: 'Needs attention',
  manual_review: 'Manual review',
  not_applicable: 'Not applicable',
};

const checkIcons: Record<string, typeof Eye> = {
  'document-language': Languages,
  'tag-tree': ListTree,
  'reading-order': ArrowRight,
  headings: ListTree,
  'image-alternatives': Eye,
  bookmarks: ListTree,
  tables: ScanSearch,
  'form-labels': FileWarning,
};

export default function AccessibilityInspector() {
  const { sessionId, documentMutationVersion } = useEditor();
  const [report, setReport] = useState<AccessibilityReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    setReport(null);
    setError('');
  }, [sessionId, documentMutationVersion]);

  const inspect = async () => {
    if (!sessionId) return;
    setLoading(true);
    setError('');
    try {
      const response = await axios.get<{ data: AccessibilityReport }>(
        `${API_BASE_URL}/api/documents/${sessionId}/accessibility`,
      );
      setReport(response.data.data);
    } catch (requestError) {
      console.error(requestError);
      setError('The local inspection failed safely. Your document was not changed.');
    } finally {
      setLoading(false);
    }
  };

  const priorityChecks = useMemo(
    () => report?.checks.filter(check => check.status === 'needs_attention') ?? [],
    [report],
  );

  return (
    <main className="min-h-full overflow-auto bg-[#f4f0e8] p-3 text-slate-950 sm:p-6 lg:p-8">
      <div className="mx-auto max-w-7xl">
        <section className="relative overflow-hidden rounded-[2rem] border-2 border-slate-950 bg-[#d9ff43] p-5 shadow-[7px_7px_0_#0f172a] sm:p-8">
          <div className="absolute -right-10 -top-14 h-48 w-48 rounded-full border-[28px] border-slate-950/10" aria-hidden="true" />
          <div className="relative grid gap-6 lg:grid-cols-[1fr_auto] lg:items-end">
            <div>
              <p className="font-mono text-[10px] font-black uppercase tracking-[.28em]">Document evidence · local only</p>
              <h1 className="mt-3 max-w-4xl font-display text-4xl font-black leading-[.92] tracking-tight sm:text-6xl">Accessibility is structure, not a badge.</h1>
              <p className="mt-4 max-w-3xl text-sm font-medium leading-6 sm:text-base">Inspect language, tags, reading order, headings, image alternatives, bookmarks, tables, and form labels. The report separates machine evidence from checks that still need a person and assistive technology.</p>
            </div>
            <div className="grid grid-cols-2 gap-2 text-center font-mono text-[9px] font-black uppercase tracking-wider sm:flex">
              <span className="rounded-xl border-2 border-slate-950 bg-white px-4 py-3">No upload</span>
              <span className="rounded-xl border-2 border-slate-950 bg-white px-4 py-3">No auto-fix</span>
              <span className="rounded-xl border-2 border-slate-950 bg-white px-4 py-3">No PDF/UA claim</span>
            </div>
          </div>
        </section>

        {!sessionId ? (
          <section className="mt-7 rounded-[2rem] border-2 border-dashed border-slate-400 bg-white p-8 text-center" aria-labelledby="accessibility-empty-title">
            <CircleHelp className="mx-auto h-11 w-11 text-slate-400" aria-hidden="true" />
            <h2 id="accessibility-empty-title" className="mt-4 text-2xl font-black">Open a PDF first</h2>
            <p className="mx-auto mt-2 max-w-xl text-sm text-slate-600">Use the local upload control in the sidebar. The inspector reads the active editing session and never sends the file to an external service.</p>
          </section>
        ) : (
          <>
            <section className="mt-7 flex flex-col gap-4 rounded-[2rem] border-2 border-slate-950 bg-slate-950 p-5 text-white shadow-[7px_7px_0_#f59e0b] sm:flex-row sm:items-center sm:justify-between">
              <div>
                <p className="font-mono text-[10px] font-black uppercase tracking-[.25em] text-amber-300">Current local session</p>
                <h2 className="mt-1 text-2xl font-black">Run a bounded, content-free inspection</h2>
                <p className="mt-1 text-sm text-slate-400">Any later edit invalidates this snapshot and asks you to inspect again.</p>
              </div>
              <button type="button" onClick={() => void inspect()} disabled={loading} className="touch-target inline-flex min-h-12 shrink-0 items-center justify-center gap-2 rounded-2xl border-2 border-white bg-cyan-300 px-5 text-sm font-black text-slate-950 transition hover:-translate-y-0.5 hover:bg-cyan-200 disabled:opacity-60">
                {loading ? <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" /> : report ? <RefreshCw className="h-5 w-5" aria-hidden="true" /> : <ScanSearch className="h-5 w-5" aria-hidden="true" />}
                {loading ? 'Inspecting locally…' : report ? 'Inspect again' : 'Inspect accessibility'}
              </button>
            </section>

            <div aria-live="polite" className="mt-5">
              {error && <div role="alert" className="rounded-2xl border-2 border-rose-700 bg-rose-50 p-4 text-sm font-bold text-rose-900">{error}</div>}
            </div>

            {report && (
              <div className="mt-7 space-y-7">
                <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4" aria-label="Accessibility inspection summary">
                  {[
                    ['Needs attention', report.summary.checks_needing_attention, 'bg-rose-300'],
                    ['Manual checks', report.summary.checks_requiring_manual_review, 'bg-amber-300'],
                    ['Evidence found', report.summary.checks_passed, 'bg-emerald-300'],
                    ['Pages scanned', `${report.summary.pages_scanned}/${report.summary.total_pages}`, 'bg-cyan-300'],
                  ].map(([label, value, color]) => (
                    <div key={label} className={`rounded-3xl border-2 border-slate-950 ${color} p-5 shadow-[4px_4px_0_#0f172a]`}>
                      <strong className="block text-4xl font-black">{value}</strong>
                      <span className="mt-1 block font-mono text-[10px] font-black uppercase tracking-wider">{label}</span>
                    </div>
                  ))}
                </section>

                {report.inventory.tags.present && (
                  <section className="rounded-3xl border-2 border-rose-700 bg-rose-50 p-5 text-rose-950" aria-labelledby="preserve-semantics-title">
                    <div className="flex gap-3">
                      <ShieldAlert className="mt-0.5 h-6 w-6 shrink-0" aria-hidden="true" />
                      <div>
                        <h2 id="preserve-semantics-title" className="font-black">This PDF already has tagged accessibility semantics</h2>
                        <p className="mt-1 text-sm leading-6">Page, content, form, or annotation edits may degrade the tag tree and reading order. Review the change warnings, compare the output, then rerun this inspector and an independent PDF/UA checker.</p>
                      </div>
                    </div>
                  </section>
                )}

                <section aria-labelledby="accessibility-checks-title">
                  <div className="mb-4 flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
                    <div>
                      <p className="font-mono text-[10px] font-black uppercase tracking-[.25em] text-slate-500">Eight evidence lanes</p>
                      <h2 id="accessibility-checks-title" className="text-3xl font-black">What the file can — and cannot — prove</h2>
                    </div>
                    <p className="max-w-xl text-xs leading-5 text-slate-600">Page hints locate likely review points; they never expose document text. A “manual review” result is not a pass.</p>
                  </div>
                  <div className="grid gap-4 lg:grid-cols-2">
                    {report.checks.map(check => {
                      const Icon = checkIcons[check.id] ?? CircleHelp;
                      return (
                        <article key={check.id} className="rounded-3xl border-2 border-slate-950 bg-white p-5 shadow-[4px_4px_0_#cbd5e1]">
                          <div className="flex items-start justify-between gap-3">
                            <div className="flex gap-3">
                              <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-slate-950 text-cyan-300"><Icon className="h-5 w-5" aria-hidden="true" /></span>
                              <div><h3 className="font-black">{check.title}</h3><p className="mt-1 text-xs leading-5 text-slate-600">{check.summary}</p></div>
                            </div>
                            <span className={`shrink-0 rounded-full border px-2.5 py-1 font-mono text-[8px] font-black uppercase tracking-wider ${statusStyles[check.status]}`}>{statusLabels[check.status]}</span>
                          </div>
                          {check.page_hints.length > 0 && <p className="mt-4 rounded-xl bg-slate-100 px-3 py-2 font-mono text-[10px] font-bold text-slate-600">Review page{check.page_hints.length > 1 ? 's' : ''}: {check.page_hints.join(', ')}{check.page_hints.length === 20 ? '…' : ''}</p>}
                          <div className="mt-4 border-t border-slate-200 pt-4">
                            <p className="font-mono text-[9px] font-black uppercase tracking-wider text-slate-500">Manual repair guidance</p>
                            <ul className="mt-2 space-y-2 text-xs leading-5 text-slate-700">{check.guidance.map(item => <li key={item} className="flex gap-2"><ArrowRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-slate-400" aria-hidden="true" />{item}</li>)}</ul>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                </section>

                <section className="grid gap-4 rounded-[2rem] border-2 border-slate-950 bg-[#fee8bd] p-5 lg:grid-cols-[1fr_auto] lg:items-center">
                  <div className="flex gap-3">
                    <AlertTriangle className="mt-0.5 h-6 w-6 shrink-0" aria-hidden="true" />
                    <div><h2 className="font-black">Reporting before remediation</h2><p className="mt-1 text-sm leading-6">The inspector does not rewrite tags, guess reading order, manufacture alt text, or claim PDF/UA compliance. Export a separate copy, validate it independently, and test it with real assistive technology.</p></div>
                  </div>
                  <div className="rounded-2xl border-2 border-slate-950 bg-white px-4 py-3 font-mono text-[9px] font-bold uppercase tracking-wide">
                    <span className="flex items-center gap-2"><Check className="h-4 w-4 text-emerald-700" /> Content-free audit</span>
                    <span className="mt-1 block max-w-48 truncate text-slate-500" title={report.audit_sha256}>SHA {report.audit_sha256.slice(0, 16)}…</span>
                  </div>
                </section>

                {priorityChecks.length === 0 && <p className="sr-only">No automated check needs immediate attention. Manual validation is still required.</p>}
              </div>
            )}
          </>
        )}
      </div>
    </main>
  );
}
