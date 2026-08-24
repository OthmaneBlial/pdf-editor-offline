import { useCallback, useEffect, useMemo, useState } from 'react';
import axios from 'axios';
import {
  CheckCircle2,
  CircleX,
  FileSearch,
  Gauge,
  Languages,
  LoaderCircle,
  RefreshCcw,
  RotateCcw,
  ScanSearch,
  Search,
  ShieldCheck,
  Square,
  Trash2,
  WandSparkles,
} from 'lucide-react';

import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';

interface OCRCapabilities {
  available: boolean;
  engine: string;
  version: string | null;
  languages: string[];
  hidden_downloads: boolean;
  orientation_data_available?: boolean;
}

interface OCRJobResult {
  document_id: string;
  filename: string;
  page_count: number;
  download_url: string;
  source_preserved: boolean;
  pages_processed: number;
  word_count: number;
  average_confidence: number | null;
}

interface OCRJob {
  id: string;
  source_document_id: string;
  status: 'queued' | 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled';
  progress: number;
  pages_completed: number;
  pages_total: number;
  current_page: number | null;
  stage: string;
  can_cancel: boolean;
  can_retry: boolean;
  result: OCRJobResult | null;
  error: { code: string; message: string } | null;
}

interface OCRPageSummary {
  page: number;
  word_count: number;
  average_confidence: number | null;
  minimum_confidence: number | null;
  deskew_degrees: number;
  orientation_degrees: number;
  correction_count: number;
  layer_status: string;
}

interface OCRLayer {
  layer_status: string;
  source_preserved: boolean;
  visual_source_preserved: boolean;
  word_count: number;
  average_confidence: number | null;
  pages_processed: number;
  pages: OCRPageSummary[];
}

interface OCRWord {
  id: string;
  text: string;
  confidence: number;
  bbox: number[];
  corrected?: boolean;
}

interface OCRPage extends OCRPageSummary {
  words: OCRWord[];
}

interface SearchMatch {
  page: number;
  word_id: string;
  text: string;
  confidence: number;
  context: string;
}

const LANGUAGE_NAMES: Record<string, string> = {
  ara: 'Arabic',
  deu: 'German',
  eng: 'English',
  fra: 'French',
  hin: 'Hindi',
  ita: 'Italian',
  jpn: 'Japanese',
  kor: 'Korean',
  nld: 'Dutch',
  osd: 'Orientation detection',
  por: 'Portuguese',
  rus: 'Russian',
  spa: 'Spanish',
  tur: 'Turkish',
  chi_sim: 'Chinese (simplified)',
  chi_tra: 'Chinese (traditional)',
};

const TERMINAL_STATES = new Set(['succeeded', 'failed', 'cancelled']);

function responseData<T>(response: { data?: { data?: T } }): T {
  return response.data?.data as T;
}

function errorMessage(error: unknown): string {
  if (axios.isAxiosError(error)) {
    const detail = error.response?.data?.detail;
    if (typeof detail === 'string') return detail;
    const message = error.response?.data?.message;
    if (typeof message === 'string') return message;
  }
  return error instanceof Error ? error.message : 'The local OCR operation failed safely.';
}

function confidenceTone(confidence: number | null) {
  if (confidence === null) return 'text-slate-400';
  if (confidence >= 85) return 'text-emerald-300';
  if (confidence >= 65) return 'text-amber-300';
  return 'text-rose-300';
}

export default function OCRSearchWorkflow() {
  const {
    sessionId,
    pageCount,
    restoreRecoveredDocument,
    reportToolResult,
    setCurrentPage,
  } = useEditor();
  const [capabilities, setCapabilities] = useState<OCRCapabilities | null>(null);
  const [pageRange, setPageRange] = useState('all');
  const [languages, setLanguages] = useState<string[]>([]);
  const [dpi, setDpi] = useState(180);
  const [minimumConfidence, setMinimumConfidence] = useState(0);
  const [autoRotate, setAutoRotate] = useState(true);
  const [deskew, setDeskew] = useState(true);
  const [job, setJob] = useState<OCRJob | null>(null);
  const [layer, setLayer] = useState<OCRLayer | null>(null);
  const [selectedPage, setSelectedPage] = useState<number | null>(null);
  const [pageDetail, setPageDetail] = useState<OCRPage | null>(null);
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [search, setSearch] = useState('');
  const [matches, setMatches] = useState<SearchMatch[]>([]);
  const [removeConfirmed, setRemoveConfirmed] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadLayer = useCallback(async (documentId: string) => {
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${documentId}/ocr/layer`);
      const next = responseData<OCRLayer>(response);
      setLayer(next);
      const firstActive = next.pages.find(page => page.layer_status === 'active');
      setSelectedPage(firstActive?.page ?? null);
    } catch (loadError) {
      if (!axios.isAxiosError(loadError) || loadError.response?.status !== 404) {
        setError(errorMessage(loadError));
      }
      setLayer(null);
      setSelectedPage(null);
      setPageDetail(null);
    }
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const response = await axios.get(`${API_BASE_URL}/api/ocr/capabilities`);
        const next = responseData<OCRCapabilities>(response);
        setCapabilities(next);
        setAutoRotate(Boolean(next.orientation_data_available));
        setLanguages(current => {
          if (current.length) return current.filter(language => next.languages.includes(language));
          const preferred = next.languages.includes('eng') ? 'eng' : next.languages[0];
          return preferred ? [preferred] : [];
        });
      } catch (loadError) {
        setError(errorMessage(loadError));
      }
    })();
  }, []);

  useEffect(() => {
    setNotice(null);
    setError(null);
    setMatches([]);
    if (!sessionId) {
      setLayer(null);
      setJob(null);
      return;
    }
    setJob(null);
    void loadLayer(sessionId);
    void axios
      .get(`${API_BASE_URL}/api/documents/${sessionId}/ocr/jobs`)
      .then(response => {
        const jobs = responseData<{ jobs: OCRJob[] }>(response).jobs;
        if (jobs[0]) setJob(jobs[0]);
      })
      .catch(() => undefined);
  }, [loadLayer, sessionId]);

  useEffect(() => {
    if (!job || TERMINAL_STATES.has(job.status)) return;
    const timer = window.setInterval(() => {
      void axios
        .get(`${API_BASE_URL}/api/documents/${job.source_document_id}/ocr/jobs/${job.id}`)
        .then(response => {
          const next = responseData<OCRJob>(response);
          setJob(next);
          if (next.status === 'succeeded') {
            setNotice('Searchable copy ready. Your source session is unchanged.');
            reportToolResult('success', 'OCR copy ready — source preserved.');
          } else if (next.status === 'failed' || next.status === 'cancelled') {
            setError(next.error?.message ?? `OCR ${next.status}.`);
          }
        })
        .catch(pollError => setError(errorMessage(pollError)));
    }, 500);
    return () => window.clearInterval(timer);
  }, [job, reportToolResult]);

  useEffect(() => {
    if (!sessionId || selectedPage === null || !layer || layer.layer_status !== 'active') {
      setPageDetail(null);
      return;
    }
    void axios
      .get(`${API_BASE_URL}/api/documents/${sessionId}/ocr/layer/pages/${selectedPage}`)
      .then(response => {
        const next = responseData<OCRPage>(response);
        setPageDetail(next);
        setEdits(Object.fromEntries(next.words.map(word => [word.id, word.text])));
      })
      .catch(loadError => setError(errorMessage(loadError)));
  }, [layer, selectedPage, sessionId]);

  const dirtyCorrections = useMemo(() => {
    if (!pageDetail) return [];
    return pageDetail.words
      .filter(word => edits[word.id] !== undefined && edits[word.id] !== word.text)
      .map(word => ({ id: word.id, text: edits[word.id] }));
  }, [edits, pageDetail]);

  const toggleLanguage = (language: string) => {
    setLanguages(current =>
      current.includes(language)
        ? current.filter(item => item !== language)
        : current.length < 8
          ? [...current, language]
          : current,
    );
  };

  const queueOCR = async () => {
    if (!sessionId || !languages.length) return;
    setBusy('queue');
    setError(null);
    setNotice(null);
    try {
      const response = await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/ocr/jobs`, {
        page_range: pageRange,
        languages,
        dpi,
        auto_rotate: autoRotate,
        deskew,
        minimum_confidence: minimumConfidence,
      });
      setJob(responseData<OCRJob>(response));
      setNotice('OCR queued in the bounded local background worker.');
    } catch (queueError) {
      setError(errorMessage(queueError));
    } finally {
      setBusy(null);
    }
  };

  const cancelJob = async () => {
    if (!job) return;
    setBusy('cancel');
    setError(null);
    try {
      const response = await axios.delete(
        `${API_BASE_URL}/api/documents/${job.source_document_id}/ocr/jobs/${job.id}`,
      );
      setJob(responseData<OCRJob>(response));
      setNotice('Cancellation requested. The active page process will stop safely.');
    } catch (cancelError) {
      setError(errorMessage(cancelError));
    } finally {
      setBusy(null);
    }
  };

  const retryJob = async () => {
    if (!job) return;
    setBusy('retry');
    setError(null);
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${job.source_document_id}/ocr/jobs/${job.id}/retry`,
      );
      setJob(responseData<OCRJob>(response));
      setNotice('Retry queued from a fresh source snapshot.');
    } catch (retryError) {
      setError(errorMessage(retryError));
    } finally {
      setBusy(null);
    }
  };

  const openSearchableCopy = async () => {
    if (!job?.result) return;
    setBusy('open');
    setError(null);
    try {
      const response = await axios.get(
        `${API_BASE_URL}${job.result.download_url}`,
        { responseType: 'blob' },
      );
      const file = new File([response.data], job.result.filename, { type: 'application/pdf' });
      restoreRecoveredDocument(file, job.result.document_id, job.result.page_count);
      setNotice('Searchable copy opened. The original session remains stored separately.');
    } catch (openError) {
      setError(errorMessage(openError));
    } finally {
      setBusy(null);
    }
  };

  const runSearch = async () => {
    if (!sessionId || !search.trim()) return;
    setBusy('search');
    setError(null);
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/ocr/search`,
        { text: search.trim() },
      );
      const data = responseData<{ matches: SearchMatch[]; truncated: boolean }>(response);
      setMatches(data.matches);
      setNotice(
        `${data.matches.length} local match${data.matches.length === 1 ? '' : 'es'}${data.truncated ? ' (first 200)' : ''}.`,
      );
    } catch (searchError) {
      setError(errorMessage(searchError));
    } finally {
      setBusy(null);
    }
  };

  const saveCorrections = async () => {
    if (!sessionId || selectedPage === null || !dirtyCorrections.length) return;
    setBusy('correct');
    setError(null);
    try {
      for (let offset = 0; offset < dirtyCorrections.length; offset += 500) {
        await axios.put(
          `${API_BASE_URL}/api/documents/${sessionId}/ocr/layer/pages/${selectedPage}`,
          { corrections: dirtyCorrections.slice(offset, offset + 500) },
        );
      }
      await loadLayer(sessionId);
      setSelectedPage(selectedPage);
      setNotice(`${dirtyCorrections.length} OCR correction${dirtyCorrections.length === 1 ? '' : 's'} saved locally.`);
      reportToolResult('success', 'OCR text corrected; source scan unchanged.', true);
    } catch (saveError) {
      setError(errorMessage(saveError));
    } finally {
      setBusy(null);
    }
  };

  const removeLayer = async () => {
    if (!sessionId || !removeConfirmed) return;
    setBusy('remove');
    setError(null);
    try {
      await axios.delete(`${API_BASE_URL}/api/documents/${sessionId}/ocr/layer`);
      await loadLayer(sessionId);
      setRemoveConfirmed(false);
      setMatches([]);
      setNotice('OCR text layer removed. The visual source scan is still present.');
      reportToolResult('success', 'OCR layer removed; source scan preserved.', true);
    } catch (removeError) {
      setError(errorMessage(removeError));
    } finally {
      setBusy(null);
    }
  };

  if (!sessionId) {
    return (
      <div className="min-h-full bg-slate-950 px-4 py-10 text-slate-100 sm:px-8">
        <div className="mx-auto max-w-3xl rounded-[2rem] border border-cyan-400/20 bg-slate-900/80 p-8 text-center shadow-2xl shadow-cyan-950/30">
          <ScanSearch className="mx-auto h-12 w-12 text-cyan-300" aria-hidden="true" />
          <h2 className="mt-4 font-display text-3xl font-bold">OCR &amp; Search</h2>
          <p className="mx-auto mt-3 max-w-lg text-slate-300">
            Upload a scanned PDF first. OCR runs locally and always creates a separate searchable copy.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-[radial-gradient(circle_at_top_left,_rgba(8,145,178,0.16),_transparent_36%),linear-gradient(145deg,#020617,#0f172a_55%,#111827)] px-3 py-5 text-slate-100 sm:px-6 sm:py-8">
      <div className="mx-auto max-w-7xl space-y-5">
        <header className="overflow-hidden rounded-[2rem] border border-cyan-300/20 bg-slate-900/75 p-6 shadow-2xl shadow-cyan-950/20 backdrop-blur sm:p-8">
          <div className="flex flex-col justify-between gap-5 lg:flex-row lg:items-end">
            <div>
              <div className="flex flex-wrap items-center gap-2 text-xs font-bold uppercase tracking-[0.2em] text-cyan-300">
                <ShieldCheck className="h-4 w-4" aria-hidden="true" /> Local evidence layer
                <span className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-3 py-1 text-emerald-200">No hidden downloads</span>
              </div>
              <h2 className="mt-4 font-display text-3xl font-black tracking-tight sm:text-5xl">OCR &amp; Search</h2>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-300 sm:text-base">
                Keep every scan pixel, add a removable invisible text layer, then inspect confidence and repair recognition before sharing.
              </p>
            </div>
            <div className="grid grid-cols-2 gap-2 text-center text-xs sm:grid-cols-3">
              <div className="rounded-2xl border border-slate-700 bg-slate-950/60 px-4 py-3"><strong className="block text-lg text-white">{pageCount}</strong>pages</div>
              <div className="rounded-2xl border border-slate-700 bg-slate-950/60 px-4 py-3"><strong className="block text-lg text-white">{capabilities?.languages.length ?? '…'}</strong>packs</div>
              <div className="col-span-2 rounded-2xl border border-slate-700 bg-slate-950/60 px-4 py-3 sm:col-span-1"><strong className="block text-lg text-emerald-300">0</strong>network calls</div>
            </div>
          </div>
        </header>

        {(notice || error) && (
          <div role={error ? 'alert' : 'status'} className={`flex items-start gap-3 rounded-2xl border px-4 py-3 text-sm ${error ? 'border-rose-400/40 bg-rose-950/40 text-rose-100' : 'border-cyan-400/30 bg-cyan-950/40 text-cyan-100'}`}>
            {error ? <CircleX className="mt-0.5 h-4 w-4 shrink-0" /> : <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />}
            <span>{error ?? notice}</span>
          </div>
        )}

        <div className="grid gap-5 xl:grid-cols-[minmax(0,0.88fr)_minmax(0,1.12fr)]">
          <section className="space-y-5" aria-labelledby="ocr-config-heading">
            <div className="rounded-[1.75rem] border border-slate-700/80 bg-slate-900/80 p-5 shadow-xl">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs font-bold uppercase tracking-[0.18em] text-cyan-300">01 · Configure</p>
                  <h3 id="ocr-config-heading" className="mt-2 text-xl font-bold">Recognition recipe</h3>
                </div>
                <span className={`rounded-full px-3 py-1 text-xs font-bold ${capabilities?.available ? 'bg-emerald-400/10 text-emerald-300' : 'bg-rose-400/10 text-rose-300'}`}>
                  {capabilities?.available ? capabilities.version : 'Tesseract unavailable'}
                </span>
              </div>

              <div className="mt-5 grid gap-4 sm:grid-cols-2">
                <label className="text-sm font-semibold text-slate-200">
                  Page range
                  <input value={pageRange} onChange={event => setPageRange(event.target.value)} placeholder="all or 1-3, 7" className="mt-2 w-full rounded-xl border border-slate-600 bg-slate-950 px-3 py-2.5 text-white outline-none transition focus:border-cyan-400 focus:ring-2 focus:ring-cyan-400/20" />
                </label>
                <label className="text-sm font-semibold text-slate-200">
                  Render quality
                  <select value={dpi} onChange={event => setDpi(Number(event.target.value))} className="mt-2 w-full rounded-xl border border-slate-600 bg-slate-950 px-3 py-2.5 text-white outline-none focus:border-cyan-400">
                    <option value={120}>Fast · 120 DPI</option>
                    <option value={180}>Balanced · 180 DPI</option>
                    <option value={240}>Detailed · 240 DPI</option>
                    <option value={300}>Maximum · 300 DPI</option>
                  </select>
                </label>
              </div>

              <fieldset className="mt-5">
                <legend className="flex items-center gap-2 text-sm font-semibold text-slate-200"><Languages className="h-4 w-4 text-cyan-300" /> Installed language packs</legend>
                <div className="mt-3 flex max-h-40 flex-wrap gap-2 overflow-y-auto rounded-2xl border border-slate-700 bg-slate-950/60 p-3">
                  {capabilities?.languages.map(language => (
                    <label key={language} className={`cursor-pointer rounded-full border px-3 py-2 text-xs font-semibold transition ${languages.includes(language) ? 'border-cyan-300 bg-cyan-400/15 text-cyan-100' : 'border-slate-700 text-slate-400 hover:border-slate-500'}`}>
                      <input type="checkbox" checked={languages.includes(language)} onChange={() => toggleLanguage(language)} className="sr-only" />
                      {LANGUAGE_NAMES[language] ?? language} <span className="font-mono text-[10px] opacity-60">{language}</span>
                    </label>
                  ))}
                  {capabilities && !capabilities.languages.length && <p className="text-sm text-rose-300">Install Tesseract and explicit language packs before starting.</p>}
                </div>
                <p className="mt-2 text-xs text-slate-500">Only recognition packs already on this machine appear here. Orientation data: {capabilities?.orientation_data_available ? 'installed' : 'not installed'}. This app never downloads either while processing.</p>
              </fieldset>

              <div className="mt-5 grid gap-3 sm:grid-cols-2">
                <label className="flex items-start gap-3 rounded-2xl border border-slate-700 bg-slate-950/50 p-3 text-sm">
                  <input type="checkbox" checked={autoRotate} disabled={!capabilities?.orientation_data_available} onChange={event => setAutoRotate(event.target.checked)} className="mt-1 accent-cyan-400 disabled:opacity-40" />
                  <span><strong className="block text-white">Auto-rotation</strong><span className="text-xs text-slate-400">Use installed orientation data when available.</span></span>
                </label>
                <label className="flex items-start gap-3 rounded-2xl border border-slate-700 bg-slate-950/50 p-3 text-sm">
                  <input type="checkbox" checked={deskew} onChange={event => setDeskew(event.target.checked)} className="mt-1 accent-cyan-400" />
                  <span><strong className="block text-white">Deskew analysis</strong><span className="text-xs text-slate-400">Correct small scan angles for recognition only.</span></span>
                </label>
              </div>

              <label className="mt-5 block text-sm font-semibold text-slate-200">
                Ignore words below {minimumConfidence}% confidence
                <input aria-label="Minimum OCR confidence" type="range" min="0" max="90" step="5" value={minimumConfidence} onChange={event => setMinimumConfidence(Number(event.target.value))} className="mt-3 w-full accent-cyan-400" />
              </label>

              <button type="button" onClick={() => void queueOCR()} disabled={!capabilities?.available || !languages.length || busy !== null || Boolean(job && !TERMINAL_STATES.has(job.status))} className="mt-5 flex w-full items-center justify-center gap-2 rounded-2xl bg-gradient-to-r from-cyan-400 to-sky-500 px-4 py-3 font-black text-slate-950 shadow-lg shadow-cyan-950/40 transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-40">
                {busy === 'queue' ? <LoaderCircle className="h-5 w-5 animate-spin" /> : <WandSparkles className="h-5 w-5" />}
                Create searchable copy
              </button>
            </div>

            {job && (
              <div className="rounded-[1.75rem] border border-slate-700/80 bg-slate-900/80 p-5 shadow-xl" aria-labelledby="ocr-job-heading">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <p className="text-xs font-bold uppercase tracking-[0.18em] text-violet-300">02 · Background job</p>
                    <h3 id="ocr-job-heading" className="mt-2 text-xl font-bold capitalize">{job.status}</h3>
                  </div>
                  <span className="font-mono text-2xl font-black text-white">{job.progress}%</span>
                </div>
                <div className="mt-4 h-2 overflow-hidden rounded-full bg-slate-950" role="progressbar" aria-label="OCR progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={job.progress}>
                  <div className="h-full rounded-full bg-gradient-to-r from-violet-400 via-cyan-300 to-emerald-300 transition-all" style={{ width: `${job.progress}%` }} />
                </div>
                <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-slate-400">
                  <span>{job.pages_completed} / {job.pages_total} pages</span>
                  <span>Stage: {job.stage.replaceAll('_', ' ')}</span>
                  {job.current_page && <span>Page {job.current_page}</span>}
                </div>
                {job.error && <p className="mt-4 rounded-xl bg-rose-950/50 p-3 text-sm text-rose-200">{job.error.message}</p>}
                {job.result && (
                  <div className="mt-4 grid grid-cols-3 gap-2 text-center text-xs">
                    <div className="rounded-xl bg-slate-950/70 p-3"><strong className="block text-lg text-white">{job.result.pages_processed}</strong>pages</div>
                    <div className="rounded-xl bg-slate-950/70 p-3"><strong className="block text-lg text-white">{job.result.word_count}</strong>words</div>
                    <div className="rounded-xl bg-slate-950/70 p-3"><strong className={`block text-lg ${confidenceTone(job.result.average_confidence)}`}>{job.result.average_confidence ?? '—'}%</strong>confidence</div>
                  </div>
                )}
                <div className="mt-4 flex flex-wrap gap-2">
                  {job.can_cancel && <button type="button" onClick={() => void cancelJob()} disabled={busy !== null} className="inline-flex items-center gap-2 rounded-xl border border-rose-400/40 px-4 py-2 text-sm font-bold text-rose-200 hover:bg-rose-400/10"><Square className="h-3.5 w-3.5 fill-current" /> Cancel</button>}
                  {job.can_retry && <button type="button" onClick={() => void retryJob()} disabled={busy !== null} className="inline-flex items-center gap-2 rounded-xl border border-violet-400/40 px-4 py-2 text-sm font-bold text-violet-200 hover:bg-violet-400/10"><RefreshCcw className="h-4 w-4" /> Retry</button>}
                  {job.result && <button type="button" onClick={() => void openSearchableCopy()} disabled={busy !== null} className="inline-flex flex-1 items-center justify-center gap-2 rounded-xl bg-emerald-400 px-4 py-2 text-sm font-black text-emerald-950 hover:bg-emerald-300"><FileSearch className="h-4 w-4" /> Open searchable copy</button>}
                </div>
                <p className="mt-3 flex items-center gap-2 text-xs text-emerald-300"><ShieldCheck className="h-3.5 w-3.5" /> The active source session is never overwritten.</p>
              </div>
            )}
          </section>

          <section className="rounded-[1.75rem] border border-slate-700/80 bg-slate-900/80 p-5 shadow-xl" aria-labelledby="ocr-layer-heading">
            <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
              <div>
                <p className="text-xs font-bold uppercase tracking-[0.18em] text-emerald-300">03 · Inspect &amp; correct</p>
                <h3 id="ocr-layer-heading" className="mt-2 text-xl font-bold">Searchable evidence layer</h3>
              </div>
              {layer && <span className={`rounded-full px-3 py-1 text-xs font-bold ${layer.layer_status === 'active' ? 'bg-emerald-400/10 text-emerald-300' : 'bg-slate-700 text-slate-300'}`}>{layer.layer_status}</span>}
            </div>

            {!layer ? (
              <div className="mt-8 rounded-3xl border-2 border-dashed border-slate-700 bg-slate-950/40 px-5 py-12 text-center">
                <ScanSearch className="mx-auto h-10 w-10 text-slate-500" />
                <p className="mt-4 font-bold text-slate-200">No inspectable OCR layer on this session</p>
                <p className="mx-auto mt-2 max-w-md text-sm text-slate-500">Create a searchable copy, then open it here to search text, review confidence and correct individual words.</p>
              </div>
            ) : layer.layer_status !== 'active' ? (
              <div className="mt-8 rounded-3xl border border-slate-700 bg-slate-950/50 p-8 text-center">
                <RotateCcw className="mx-auto h-9 w-9 text-slate-500" />
                <p className="mt-3 font-bold">The OCR layer was removed</p>
                <p className="mt-2 text-sm text-slate-400">The visual scan remains intact. Run OCR again to create another separate copy.</p>
              </div>
            ) : (
              <>
                <div className="mt-5 grid grid-cols-3 gap-2 text-center text-xs">
                  <div className="rounded-2xl border border-slate-700 bg-slate-950/60 p-3"><Gauge className="mx-auto mb-1 h-4 w-4 text-cyan-300" /><strong className={`block text-xl ${confidenceTone(layer.average_confidence)}`}>{layer.average_confidence ?? '—'}%</strong>average</div>
                  <div className="rounded-2xl border border-slate-700 bg-slate-950/60 p-3"><FileSearch className="mx-auto mb-1 h-4 w-4 text-violet-300" /><strong className="block text-xl text-white">{layer.word_count}</strong>words</div>
                  <div className="rounded-2xl border border-slate-700 bg-slate-950/60 p-3"><ShieldCheck className="mx-auto mb-1 h-4 w-4 text-emerald-300" /><strong className="block text-xl text-white">{layer.pages_processed}</strong>pages</div>
                </div>

                <form className="mt-5 flex gap-2" onSubmit={event => { event.preventDefault(); void runSearch(); }}>
                  <label className="sr-only" htmlFor="ocr-search">Search OCR text</label>
                  <input id="ocr-search" value={search} onChange={event => setSearch(event.target.value)} placeholder="Search recognized text…" className="min-w-0 flex-1 rounded-xl border border-slate-600 bg-slate-950 px-3 py-2.5 text-white outline-none focus:border-emerald-400" />
                  <button type="submit" disabled={!search.trim() || busy !== null} className="inline-flex items-center gap-2 rounded-xl bg-emerald-400 px-4 py-2 font-black text-emerald-950 disabled:opacity-40"><Search className="h-4 w-4" /> Search</button>
                </form>

                {matches.length > 0 && (
                  <div className="mt-3 max-h-40 space-y-2 overflow-y-auto rounded-2xl border border-emerald-400/20 bg-emerald-950/20 p-2" aria-label="OCR search results">
                    {matches.map(match => (
                      <button key={`${match.page}-${match.word_id}`} type="button" onClick={() => { setSelectedPage(match.page); setCurrentPage(match.page); }} className="w-full rounded-xl border border-slate-700 bg-slate-950/70 px-3 py-2 text-left text-xs hover:border-emerald-400/50">
                        <span className="font-bold text-emerald-300">Page {match.page + 1}</span>
                        <span className="ml-2 text-slate-300">{match.context}</span>
                      </button>
                    ))}
                  </div>
                )}

                <div className="mt-5 flex gap-2 overflow-x-auto pb-2" aria-label="OCR pages">
                  {layer.pages.filter(page => page.layer_status === 'active').map(page => (
                    <button key={page.page} type="button" onClick={() => { setSelectedPage(page.page); setCurrentPage(page.page); }} className={`min-w-28 rounded-2xl border p-3 text-left transition ${selectedPage === page.page ? 'border-cyan-300 bg-cyan-400/10' : 'border-slate-700 bg-slate-950/50 hover:border-slate-500'}`}>
                      <strong className="block text-sm text-white">Page {page.page + 1}</strong>
                      <span className={`text-xs ${confidenceTone(page.average_confidence)}`}>{page.average_confidence ?? '—'}% · {page.word_count} words</span>
                      {(page.deskew_degrees !== 0 || page.orientation_degrees !== 0) && <span className="mt-1 block text-[10px] text-slate-500">deskew {page.deskew_degrees}° · rotate {page.orientation_degrees}°</span>}
                    </button>
                  ))}
                </div>

                {pageDetail && (
                  <div className="mt-3">
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <h4 className="font-bold text-white">Page {pageDetail.page + 1} words</h4>
                        <p className="text-xs text-slate-500">Low-confidence items appear first. Edits rebuild only this invisible text stream.</p>
                      </div>
                      <button type="button" onClick={() => void saveCorrections()} disabled={!dirtyCorrections.length || busy !== null} className="rounded-xl bg-cyan-400 px-4 py-2 text-sm font-black text-cyan-950 disabled:opacity-40">Save {dirtyCorrections.length || ''} correction{dirtyCorrections.length === 1 ? '' : 's'}</button>
                    </div>
                    <div className="mt-3 max-h-[32rem] space-y-2 overflow-y-auto pr-1">
                      {[...pageDetail.words].sort((a, b) => a.confidence - b.confidence).map(word => (
                        <label key={word.id} className="grid gap-2 rounded-2xl border border-slate-700 bg-slate-950/55 p-3 sm:grid-cols-[90px_minmax(0,1fr)] sm:items-center">
                          <span className={`font-mono text-xs font-bold ${confidenceTone(word.confidence)}`}>{word.confidence.toFixed(1)}%</span>
                          <span>
                            <span className="sr-only">OCR text {word.id}</span>
                            <input aria-label={`OCR text ${word.id}`} value={edits[word.id] ?? ''} onChange={event => setEdits(current => ({ ...current, [word.id]: event.target.value }))} maxLength={512} className={`w-full rounded-xl border bg-slate-950 px-3 py-2 text-sm text-white outline-none focus:border-cyan-400 ${edits[word.id] !== word.text ? 'border-amber-400/70' : 'border-slate-700'}`} />
                          </span>
                        </label>
                      ))}
                    </div>
                  </div>
                )}

                <div className="mt-6 rounded-2xl border border-rose-400/20 bg-rose-950/20 p-4">
                  <div className="flex items-start gap-3">
                    <Trash2 className="mt-0.5 h-5 w-5 shrink-0 text-rose-300" />
                    <div className="flex-1">
                      <strong className="text-sm text-rose-100">Remove the complete OCR layer</strong>
                      <p className="mt-1 text-xs leading-5 text-rose-200/70">This deletes searchable text from this copy, while preserving every original visual object and scan image.</p>
                      <label className="mt-3 flex items-center gap-2 text-xs text-rose-100"><input type="checkbox" checked={removeConfirmed} onChange={event => setRemoveConfirmed(event.target.checked)} className="accent-rose-400" /> I understand this copy will no longer be searchable.</label>
                      <button type="button" onClick={() => void removeLayer()} disabled={!removeConfirmed || busy !== null} className="mt-3 inline-flex items-center gap-2 rounded-xl border border-rose-400/50 px-4 py-2 text-xs font-black text-rose-200 hover:bg-rose-400/10 disabled:opacity-40"><Trash2 className="h-3.5 w-3.5" /> Remove OCR layer</button>
                    </div>
                  </div>
                </div>
              </>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
