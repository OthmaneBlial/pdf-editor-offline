import { useMemo, useState } from 'react';
import axios from 'axios';
import {
  AlertTriangle,
  ArrowRight,
  Check,
  CheckCircle2,
  Download,
  Eye,
  FileJson,
  FileText,
  Fingerprint,
  LoaderCircle,
  LockKeyhole,
  Plus,
  RotateCcw,
  ScanSearch,
  ShieldCheck,
  Trash2,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';

interface SearchMatch {
  index: number;
  rect: number[];
}

interface RedactionMark {
  id: string;
  page_num: number;
  x: number;
  y: number;
  width: number;
  height: number;
  fill_color: [number, number, number];
  targetNumber: number;
}

interface ReviewSummary {
  mark_count: number;
  target_count: number;
  pages_affected: number[];
  actions: string[];
  source_will_be_preserved: boolean;
  review_token: string;
}

interface VerificationCheck {
  id: string;
  label: string;
  status: 'passed' | 'failed' | 'incomplete';
  items_checked: number;
  matches: number;
}

interface VerificationReport {
  status: 'verified' | 'failed' | 'incomplete';
  app_version: string;
  output_sha256: string;
  output_bytes: number;
  page_count: number;
  target_count: number;
  checks: VerificationCheck[];
  warnings: string[];
}

interface VerifiedResult {
  copy: { id: string; filename: string; download_url: string };
  verification: VerificationReport;
  reports: { json: string; markdown: string };
}

type WorkflowStage = 'mark' | 'review' | 'applying' | 'verified' | 'blocked';

const stageLabels = ['Mark', 'Review', 'Apply', 'Sanitize', 'Verify', 'Save copy'];

const actionLabels: Record<string, string> = {
  permanently_remove_marked_content: 'Permanently remove every marked area',
  remove_hidden_data_and_previous_revisions: 'Strip hidden data and previous revisions',
  reopen_with_independent_engines: 'Reopen with independent extraction and rendering engines',
  save_as_a_new_verified_copy: 'Save a new copy; preserve the original',
};

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const RedactProveWorkflow = () => {
  const { sessionId, currentPage } = useEditor();
  const [stage, setStage] = useState<WorkflowStage>('mark');
  const [targetInput, setTargetInput] = useState('');
  const [searchMatches, setSearchMatches] = useState<SearchMatch[]>([]);
  const [targets, setTargets] = useState<string[]>([]);
  const [marks, setMarks] = useState<RedactionMark[]>([]);
  const [searching, setSearching] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [review, setReview] = useState<ReviewSummary | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [result, setResult] = useState<VerifiedResult | null>(null);
  const [blockedReport, setBlockedReport] = useState<VerificationReport | null>(null);
  const [message, setMessage] = useState<string>('');

  const uniquePages = useMemo(
    () => Array.from(new Set(marks.map(mark => mark.page_num))).sort((a, b) => a - b),
    [marks],
  );

  const resetReview = () => {
    setReview(null);
    setAcknowledged(false);
    setStage('mark');
  };

  const searchTarget = async (event: React.FormEvent) => {
    event.preventDefault();
    const candidate = targetInput.trim();
    if (!sessionId || !candidate) return;
    setSearching(true);
    setMessage('');
    setSearchMatches([]);
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/text/search`,
        { text: candidate },
      );
      const matches = (response.data?.data?.matches ?? []) as SearchMatch[];
      setSearchMatches(matches);
      setMessage(
        matches.length
          ? `${matches.length} exact occurrence${matches.length === 1 ? '' : 's'} found on page ${currentPage + 1}.`
          : `No exact occurrence found on page ${currentPage + 1}.`,
      );
    } catch {
      setMessage('This page could not be searched. The source has not been changed.');
    } finally {
      setSearching(false);
    }
  };

  const addMatchesToPlan = () => {
    const candidate = targetInput.trim();
    if (!candidate || searchMatches.length === 0) return;
    const existingTargetIndex = targets.findIndex(
      target => target.localeCompare(candidate, undefined, { sensitivity: 'accent' }) === 0,
    );
    const targetNumber = existingTargetIndex >= 0 ? existingTargetIndex + 1 : targets.length + 1;
    const additions = searchMatches
      .filter(match => match.rect.length === 4)
      .map(match => {
        const [x0, y0, x1, y1] = match.rect;
        return {
          id: `${currentPage}-${x0}-${y0}-${x1}-${y1}-${targetNumber}`,
          page_num: currentPage,
          x: Math.max(0, x0 - 1),
          y: Math.max(0, y0 - 1),
          width: Math.max(1, x1 - x0 + 2),
          height: Math.max(1, y1 - y0 + 2),
          fill_color: [0, 0, 0] as [number, number, number],
          targetNumber,
        };
      });
    setMarks(previous => {
      const ids = new Set(previous.map(mark => mark.id));
      return [...previous, ...additions.filter(mark => !ids.has(mark.id))];
    });
    if (existingTargetIndex < 0) {
      setTargets(previous => [...previous, candidate]);
    }
    setSearchMatches([]);
    setTargetInput('');
    setMessage('Occurrences added to the guarded plan. Target text is hidden from the review summary.');
    resetReview();
  };

  const removeMark = (id: string) => {
    const removed = marks.find(mark => mark.id === id);
    if (!removed) return;
    const remaining = marks.filter(mark => mark.id !== id);
    const remainingTargetNumbers = new Set(remaining.map(mark => mark.targetNumber));
    const nextTargets = targets.filter((_, index) => remainingTargetNumbers.has(index + 1));
    const targetNumberMap = new Map<number, number>();
    let nextNumber = 1;
    targets.forEach((_, index) => {
      if (remainingTargetNumbers.has(index + 1)) {
        targetNumberMap.set(index + 1, nextNumber);
        nextNumber += 1;
      }
    });
    setMarks(remaining.map(mark => ({
      ...mark,
      targetNumber: targetNumberMap.get(mark.targetNumber) ?? mark.targetNumber,
    })));
    setTargets(nextTargets);
    resetReview();
  };

  const requestPayload = (reviewToken: string | null, reviewAcknowledged: boolean) => ({
    marks: marks.map(mark => ({
      page_num: mark.page_num,
      x: mark.x,
      y: mark.y,
      width: mark.width,
      height: mark.height,
      fill_color: mark.fill_color,
    })),
    targets,
    review_acknowledged: reviewAcknowledged,
    review_token: reviewToken,
  });

  const beginReview = async () => {
    if (!sessionId || marks.length === 0 || targets.length === 0) return;
    setReviewing(true);
    setMessage('');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/redaction/review`,
        requestPayload(null, false),
      );
      setReview(response.data.data as ReviewSummary);
      setStage('review');
    } catch {
      setMessage('The plan could not be prepared for review. Nothing was changed.');
    } finally {
      setReviewing(false);
    }
  };

  const applyAndVerify = async () => {
    if (!sessionId || !review || !acknowledged) return;
    setStage('applying');
    setMessage('Working locally. Keep this window open while every proof check runs.');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/redaction/apply`,
        requestPayload(review.review_token, true),
      );
      setResult(response.data.data as VerifiedResult);
      setStage('verified');
      setMessage('A verified copy is ready. Your original document was preserved.');
    } catch (error) {
      if (axios.isAxiosError(error) && error.response?.status === 422) {
        setBlockedReport(error.response.data?.data?.verification ?? null);
        setMessage('Verification could not establish removal. No copy was saved.');
      } else {
        setMessage('The guarded workflow stopped safely. No verified copy was saved.');
      }
      setStage('blocked');
    }
  };

  const downloadAsset = async (url: string, filename: string) => {
    try {
      const response = await axios.get(`${API_BASE_URL}${url}`, { responseType: 'blob' });
      await saveBlob(response.data, filename);
    } catch {
      setMessage('The local file could not be saved. The verified copy remains available in this session.');
    }
  };

  const startOver = () => {
    setStage('mark');
    setTargetInput('');
    setSearchMatches([]);
    setTargets([]);
    setMarks([]);
    setReview(null);
    setAcknowledged(false);
    setResult(null);
    setBlockedReport(null);
    setMessage('');
  };

  if (!sessionId) {
    return (
      <section className="min-h-full p-4 sm:p-8 flex items-center justify-center" aria-labelledby="redact-prove-title">
        <div className="max-w-xl w-full rounded-3xl border border-slate-700 bg-slate-900/90 p-8 text-center shadow-2xl">
          <div className="mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-amber-300 text-slate-950">
            <ShieldCheck className="h-7 w-7" />
          </div>
          <h2 id="redact-prove-title" className="font-display text-3xl font-bold text-white">Redact &amp; Prove</h2>
          <p className="mt-3 text-sm leading-6 text-slate-400">Upload a PDF first. The original will remain untouched while a separate copy is redacted, sanitized, reopened, and verified on this device.</p>
        </div>
      </section>
    );
  }

  return (
    <section className="min-h-full bg-[#07101d] text-slate-100" aria-labelledby="redact-prove-title">
      <div className="mx-auto max-w-7xl px-4 py-6 sm:px-8 sm:py-9">
        <header className="relative overflow-hidden rounded-3xl border border-slate-700/80 bg-slate-900 px-5 py-7 shadow-2xl sm:px-8">
          <div className="absolute -right-16 -top-20 h-64 w-64 rounded-full bg-amber-300/10 blur-3xl" aria-hidden="true" />
          <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
            <div className="max-w-2xl">
              <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-3 py-1 text-xs font-semibold uppercase tracking-[0.16em] text-emerald-300">
                <LockKeyhole className="h-3.5 w-3.5" /> On-device trust workflow
              </div>
              <h2 id="redact-prove-title" className="font-display text-3xl font-bold tracking-tight text-white sm:text-5xl">Redact &amp; <span className="font-serif font-normal italic text-amber-300">prove it.</span></h2>
              <p className="mt-3 max-w-xl text-sm leading-6 text-slate-400 sm:text-base">Mark exact text, review every destructive action, then create a sanitized copy that earns its verified status through independent checks.</p>
            </div>
            <div className="grid grid-cols-2 gap-3 text-xs sm:flex">
              <div className="rounded-2xl border border-slate-700 bg-slate-950/60 px-4 py-3">
                <span className="block text-slate-500">Source</span>
                <span className="mt-1 flex items-center gap-1.5 font-semibold text-emerald-300"><Check className="h-3.5 w-3.5" /> Preserved</span>
              </div>
              <div className="rounded-2xl border border-slate-700 bg-slate-950/60 px-4 py-3">
                <span className="block text-slate-500">Network</span>
                <span className="mt-1 font-semibold text-white">Local only</span>
              </div>
            </div>
          </div>
        </header>

        <ol className="my-6 grid grid-cols-3 gap-2 sm:grid-cols-6" aria-label="Redaction workflow stages">
          {stageLabels.map((label, index) => {
            const activeIndex = stage === 'mark' ? 0 : stage === 'review' ? 1 : stage === 'applying' ? 3 : 5;
            const complete = stage === 'verified' || index < activeIndex;
            const active = stage !== 'verified' && stage !== 'blocked' && index === activeIndex;
            return (
              <li key={label} className={`rounded-xl border px-2 py-3 text-center text-[10px] font-bold uppercase tracking-wider sm:text-xs ${complete ? 'border-emerald-400/30 bg-emerald-400/10 text-emerald-300' : active ? 'border-amber-300/50 bg-amber-300/10 text-amber-200' : 'border-slate-800 bg-slate-900/70 text-slate-500'}`}>
                <span className="mb-1 block font-mono text-[9px] opacity-60">0{index + 1}</span>{label}
              </li>
            );
          })}
        </ol>

        {message && (
          <div className={`mb-6 rounded-2xl border px-4 py-3 text-sm ${stage === 'blocked' ? 'border-rose-400/30 bg-rose-400/10 text-rose-200' : stage === 'verified' ? 'border-emerald-400/30 bg-emerald-400/10 text-emerald-200' : 'border-sky-400/20 bg-sky-400/10 text-sky-200'}`} role={stage === 'blocked' ? 'alert' : 'status'} aria-live="polite">
            {message}
          </div>
        )}

        {(stage === 'mark' || stage === 'review') && (
          <div className="grid gap-6 lg:grid-cols-[minmax(0,1.35fr)_minmax(320px,.65fr)]">
            <div className="rounded-3xl border border-slate-800 bg-slate-900/80 p-5 sm:p-7">
              <div className="mb-6 flex items-start gap-4">
                <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-amber-300 text-slate-950"><ScanSearch className="h-5 w-5" /></div>
                <div><h3 className="font-display text-xl font-bold text-white">Find exact text</h3><p className="mt-1 text-sm text-slate-400">Search page {currentPage + 1}. The target stays in this local session and never appears in the exported report.</p></div>
              </div>
              <form onSubmit={searchTarget} className="flex flex-col gap-3 sm:flex-row">
                <label className="sr-only" htmlFor="redaction-target">Exact text to remove</label>
                <input id="redaction-target" value={targetInput} onChange={event => setTargetInput(event.target.value)} maxLength={512} autoComplete="off" placeholder="Exact text to remove…" className="min-w-0 flex-1 rounded-xl border border-slate-700 bg-slate-950 px-4 py-3 text-sm text-white outline-none transition focus:border-amber-300 focus:ring-2 focus:ring-amber-300/20" />
                <button type="submit" disabled={searching || !targetInput.trim()} className="inline-flex items-center justify-center gap-2 rounded-xl bg-amber-300 px-5 py-3 text-sm font-bold text-slate-950 transition hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-40">
                  {searching ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <ScanSearch className="h-4 w-4" />} Search page
                </button>
              </form>

              {searchMatches.length > 0 && (
                <div className="mt-5 rounded-2xl border border-amber-300/30 bg-amber-300/5 p-4">
                  <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                    <div><p className="font-semibold text-amber-200">{searchMatches.length} exact match{searchMatches.length === 1 ? '' : 'es'} ready</p><p className="mt-1 text-xs text-slate-400">One permanent black redaction mark will cover each match.</p></div>
                    <button type="button" onClick={addMatchesToPlan} className="inline-flex items-center justify-center gap-2 rounded-xl border border-amber-300/40 bg-amber-300/10 px-4 py-2.5 text-sm font-semibold text-amber-200 hover:bg-amber-300/20"><Plus className="h-4 w-4" /> Add all to plan</button>
                  </div>
                </div>
              )}

              <div className="mt-8 border-t border-slate-800 pt-6">
                <div className="mb-4 flex items-center justify-between"><div><h3 className="font-display text-lg font-bold text-white">Guarded plan</h3><p className="mt-1 text-xs text-slate-500">Content-free summary · targets are not repeated here</p></div><span className="rounded-full bg-slate-800 px-3 py-1 font-mono text-xs text-slate-300">{marks.length} mark{marks.length === 1 ? '' : 's'}</span></div>
                {marks.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-slate-700 px-5 py-10 text-center"><Eye className="mx-auto h-6 w-6 text-slate-600" /><p className="mt-3 text-sm text-slate-500">Search and add at least one exact match.</p></div>
                ) : (
                  <ul className="space-y-2" aria-label="Planned redaction marks">
                    {marks.map((mark, index) => (
                      <li key={mark.id} className="flex items-center gap-3 rounded-xl border border-slate-800 bg-slate-950/60 px-4 py-3">
                        <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-slate-800 font-mono text-xs text-slate-300">{index + 1}</span>
                        <div className="min-w-0 flex-1"><p className="text-sm font-semibold text-slate-200">Target {mark.targetNumber} · Page {mark.page_num + 1}</p><p className="truncate font-mono text-[10px] text-slate-500">area {mark.width.toFixed(1)} × {mark.height.toFixed(1)} pt</p></div>
                        <button type="button" onClick={() => removeMark(mark.id)} className="rounded-lg p-2 text-slate-500 hover:bg-rose-400/10 hover:text-rose-300" aria-label={`Remove mark ${index + 1}`}><Trash2 className="h-4 w-4" /></button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>

            <aside className="rounded-3xl border border-slate-800 bg-slate-900/80 p-5 sm:p-7" aria-label="Review and apply">
              {stage === 'mark' ? (
                <>
                  <Fingerprint className="h-8 w-8 text-amber-300" />
                  <h3 className="mt-4 font-display text-xl font-bold text-white">Ready for human review?</h3>
                  <p className="mt-2 text-sm leading-6 text-slate-400">The next step binds your exact marks, targets, and source file to a one-time local review token.</p>
                  <dl className="my-6 grid grid-cols-2 gap-3">
                    <div className="rounded-xl bg-slate-950/60 p-3"><dt className="text-[10px] uppercase tracking-wider text-slate-500">Targets</dt><dd className="mt-1 text-2xl font-bold text-white">{targets.length}</dd></div>
                    <div className="rounded-xl bg-slate-950/60 p-3"><dt className="text-[10px] uppercase tracking-wider text-slate-500">Pages</dt><dd className="mt-1 text-2xl font-bold text-white">{uniquePages.length}</dd></div>
                  </dl>
                  <button type="button" onClick={beginReview} disabled={reviewing || marks.length === 0} className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-white px-5 py-3 text-sm font-bold text-slate-950 transition hover:bg-slate-200 disabled:cursor-not-allowed disabled:opacity-40">{reviewing ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <ArrowRight className="h-4 w-4" />} Review destructive actions</button>
                </>
              ) : review && (
                <>
                  <div className="flex items-center justify-between"><ShieldCheck className="h-8 w-8 text-amber-300" /><button type="button" onClick={resetReview} className="text-xs font-semibold text-slate-400 hover:text-white">Edit plan</button></div>
                  <h3 className="mt-4 font-display text-xl font-bold text-white">Final review</h3>
                  <p className="mt-2 text-sm text-slate-400">{review.mark_count} mark{review.mark_count === 1 ? '' : 's'} across {review.pages_affected.length} page{review.pages_affected.length === 1 ? '' : 's'}.</p>
                  <ul className="my-5 space-y-3">
                    {review.actions.map(action => <li key={action} className="flex gap-3 text-sm leading-5 text-slate-300"><Check className="mt-0.5 h-4 w-4 shrink-0 text-emerald-300" />{actionLabels[action] ?? action}</li>)}
                  </ul>
                  <label className="flex cursor-pointer items-start gap-3 rounded-xl border border-amber-300/30 bg-amber-300/5 p-4 text-sm leading-5 text-amber-100">
                    <input type="checkbox" checked={acknowledged} onChange={event => setAcknowledged(event.target.checked)} className="mt-0.5 h-4 w-4 accent-amber-300" />
                    <span>I reviewed every mark and understand that the new copy will permanently remove content and hidden data.</span>
                  </label>
                  <button type="button" onClick={applyAndVerify} disabled={!acknowledged} className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-amber-300 px-5 py-3 text-sm font-bold text-slate-950 transition hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-40"><LockKeyhole className="h-4 w-4" /> Apply &amp; verify copy</button>
                </>
              )}
            </aside>
          </div>
        )}

        {stage === 'applying' && (
          <div className="rounded-3xl border border-sky-400/20 bg-slate-900 p-8 text-center sm:p-12" role="status" aria-live="polite">
            <LoaderCircle className="mx-auto h-12 w-12 animate-spin text-amber-300" />
            <h3 className="mt-5 font-display text-2xl font-bold text-white">Building evidence locally</h3>
            <p className="mx-auto mt-3 max-w-xl text-sm leading-6 text-slate-400">Applying marks to a detached copy, sanitizing hidden structures, reopening with independent engines, rendering every page, and running OCR proof. The source remains available.</p>
          </div>
        )}

        {stage === 'verified' && result && (
          <div className="grid gap-6 lg:grid-cols-[.8fr_1.2fr]">
            <div className="rounded-3xl border border-emerald-400/30 bg-emerald-400/10 p-6 sm:p-8">
              <CheckCircle2 className="h-12 w-12 text-emerald-300" />
              <h3 className="mt-5 font-display text-3xl font-bold text-white">Removal verified</h3>
              <p className="mt-3 text-sm leading-6 text-emerald-100/70">All {result.verification.checks.length} required checks completed with zero target matches. The source document was not replaced.</p>
              <button type="button" onClick={() => downloadAsset(result.copy.download_url, result.copy.filename)} className="mt-6 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-emerald-300 px-5 py-3 text-sm font-bold text-slate-950 hover:bg-emerald-200"><Download className="h-4 w-4" /> Download verified PDF</button>
            </div>
            <div className="rounded-3xl border border-slate-800 bg-slate-900 p-6 sm:p-8">
              <div className="grid gap-3 sm:grid-cols-3">
                <div className="rounded-xl bg-slate-950/60 p-4"><p className="text-[10px] uppercase tracking-wider text-slate-500">Pages</p><p className="mt-1 text-xl font-bold text-white">{result.verification.page_count}</p></div>
                <div className="rounded-xl bg-slate-950/60 p-4"><p className="text-[10px] uppercase tracking-wider text-slate-500">Output</p><p className="mt-1 text-xl font-bold text-white">{formatBytes(result.verification.output_bytes)}</p></div>
                <div className="rounded-xl bg-slate-950/60 p-4"><p className="text-[10px] uppercase tracking-wider text-slate-500">App</p><p className="mt-1 text-xl font-bold text-white">v{result.verification.app_version}</p></div>
              </div>
              <div className="mt-5 rounded-xl border border-slate-800 bg-slate-950/60 p-4"><p className="text-[10px] uppercase tracking-wider text-slate-500">Output SHA-256</p><p className="mt-2 break-all font-mono text-xs leading-5 text-slate-300">{result.verification.output_sha256}</p></div>
              <div className="mt-5 grid gap-3 sm:grid-cols-2">
                <button type="button" onClick={() => downloadAsset(result.reports.json, 'redaction-report.json')} className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-700 px-4 py-3 text-sm font-semibold text-slate-200 hover:bg-slate-800"><FileJson className="h-4 w-4" /> Machine report</button>
                <button type="button" onClick={() => downloadAsset(result.reports.markdown, 'redaction-report.md')} className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-700 px-4 py-3 text-sm font-semibold text-slate-200 hover:bg-slate-800"><FileText className="h-4 w-4" /> Human report</button>
              </div>
              <button type="button" onClick={startOver} className="mt-5 inline-flex items-center gap-2 text-sm font-semibold text-slate-400 hover:text-white"><RotateCcw className="h-4 w-4" /> Start another verified copy</button>
            </div>
          </div>
        )}

        {stage === 'blocked' && (
          <div className="rounded-3xl border border-rose-400/30 bg-slate-900 p-6 sm:p-8" role="alert">
            <AlertTriangle className="h-10 w-10 text-rose-300" />
            <h3 className="mt-4 font-display text-2xl font-bold text-white">Verification stopped safely</h3>
            <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-400">Status: <strong className="text-rose-200">{blockedReport?.status ?? 'incomplete'}</strong>. A required check either found a match or could not run. No green status was issued and no candidate copy was kept.</p>
            {blockedReport?.warnings?.length ? <p className="mt-3 font-mono text-xs text-slate-500">{blockedReport.warnings.length} verifier warning{blockedReport.warnings.length === 1 ? '' : 's'} recorded in the content-free result.</p> : null}
            <button type="button" onClick={() => { setStage('review'); setAcknowledged(false); }} className="mt-6 inline-flex items-center gap-2 rounded-xl border border-slate-700 px-5 py-3 text-sm font-bold text-white hover:bg-slate-800"><RotateCcw className="h-4 w-4" /> Return to review</button>
          </div>
        )}
      </div>
    </section>
  );
};

export default RedactProveWorkflow;
