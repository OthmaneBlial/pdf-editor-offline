import { useCallback, useEffect, useRef, useState } from 'react';
import axios from 'axios';
import { ArchiveRestore, FileClock, Loader2, RotateCcw, Trash2, X } from 'lucide-react';
import { useEditor } from '../contexts/EditorContext';
import { API_BASE_URL } from '../lib/apiClient';

interface RecoveryDraft {
  recovery_id: string;
  page_count: number;
  bytes: number;
  last_modified: string;
  stage: string;
  autosave_sequence: number;
}

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const stageLabel = (stage: string) => {
  if (stage.endsWith('_in_progress') || stage.endsWith('_interrupted')) return 'Interrupted operation';
  if (stage === 'autosave') return 'Autosaved edit';
  if (stage === 'export') return 'Export checkpoint';
  if (stage === 'restored') return 'Restored copy';
  return 'Open checkpoint';
};

export default function RecoveryCenter() {
  const { restoreRecoveredDocument } = useEditor();
  const [drafts, setDrafts] = useState<RecoveryDraft[]>([]);
  const [open, setOpen] = useState(false);
  const [selectedId, setSelectedId] = useState('');
  const [previewUrl, setPreviewUrl] = useState('');
  const [busyId, setBusyId] = useState('');
  const [deleteConfirmId, setDeleteConfirmId] = useState('');
  const [status, setStatus] = useState('');
  const triggerRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const loadDrafts = useCallback(async () => {
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/recovery`);
      const nextDrafts = response.data?.data?.drafts;
      setDrafts(Array.isArray(nextDrafts) ? nextDrafts : []);
    } catch {
      setStatus('Recovery copies could not be inspected.');
    }
  }, []);

  useEffect(() => {
    void loadDrafts();
  }, [loadDrafts]);

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [open]);

  useEffect(() => {
    if (!open || !selectedId) {
      setPreviewUrl('');
      return;
    }
    let nextUrl = '';
    let cancelled = false;
    void axios.get(
      `${API_BASE_URL}/api/documents/recovery/${selectedId}/preview`,
      { responseType: 'blob' },
    ).then(response => {
      if (cancelled) return;
      nextUrl = URL.createObjectURL(response.data);
      setPreviewUrl(nextUrl);
    }).catch(() => {
      if (!cancelled) setStatus('The recovery preview could not be rendered.');
    });
    return () => {
      cancelled = true;
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [open, selectedId]);

  const restoreDraft = async (draft: RecoveryDraft) => {
    setBusyId(draft.recovery_id);
    setStatus('');
    try {
      const restored = await axios.post(
        `${API_BASE_URL}/api/documents/recovery/${draft.recovery_id}/restore`,
      );
      const data = restored.data?.data;
      if (!data?.id || typeof data.page_count !== 'number') {
        throw new Error('Unexpected recovery response');
      }
      const downloaded = await axios.get(
        `${API_BASE_URL}/api/documents/${data.id}/download`,
        { responseType: 'blob' },
      );
      const file = new File([downloaded.data], 'Recovered local draft.pdf', {
        type: 'application/pdf',
      });
      restoreRecoveredDocument(file, data.id, data.page_count);
      setDrafts(current => current.filter(item => item.recovery_id !== draft.recovery_id));
      setOpen(false);
      window.dispatchEvent(new CustomEvent('pdf-recovery-restored'));
    } catch {
      setStatus('This recovery copy could not be restored. It has been kept for another attempt.');
    } finally {
      setBusyId('');
    }
  };

  const deleteDraft = async (draft: RecoveryDraft) => {
    if (deleteConfirmId !== draft.recovery_id) {
      setDeleteConfirmId(draft.recovery_id);
      return;
    }
    setBusyId(draft.recovery_id);
    setStatus('');
    try {
      await axios.delete(`${API_BASE_URL}/api/documents/recovery/${draft.recovery_id}`);
      setDrafts(current => current.filter(item => item.recovery_id !== draft.recovery_id));
      setSelectedId(current => current === draft.recovery_id ? '' : current);
      setDeleteConfirmId('');
      setStatus('Recovery copy deleted.');
    } catch {
      setStatus('The recovery copy could not be deleted.');
    } finally {
      setBusyId('');
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => {
          setOpen(true);
          setStatus('');
          setSelectedId(current => current || drafts[0]?.recovery_id || '');
          void loadDrafts();
        }}
        aria-label={`Open recovery drafts${drafts.length ? ` (${drafts.length})` : ''}`}
        className="relative flex items-center gap-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-inset)] px-2 py-2 text-[var(--text-secondary)] transition hover:border-amber-400/40 hover:text-amber-500"
      >
        <FileClock className="h-4 w-4" />
        <span className="hidden xl:inline font-display text-[10px] font-bold uppercase tracking-wide">Recovery</span>
        {drafts.length > 0 && <span className="absolute -right-1.5 -top-1.5 flex min-h-4 min-w-4 items-center justify-center rounded-full bg-amber-400 px-1 text-[9px] font-black text-amber-950">{drafts.length}</span>}
      </button>

      {open && (
        <div className="fixed inset-0 z-[75] flex items-center justify-center bg-slate-950/75 p-3 backdrop-blur-sm" onMouseDown={() => setOpen(false)}>
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="recovery-title"
            onMouseDown={event => event.stopPropagation()}
            className="max-h-[92dvh] w-full max-w-4xl overflow-y-auto rounded-[1.75rem] border border-amber-300/15 bg-[#100f0b] text-stone-100 shadow-[0_30px_100px_rgba(0,0,0,.65)]"
          >
            <header className="sticky top-0 z-10 flex items-start justify-between gap-5 border-b border-white/8 bg-[#100f0b]/95 px-5 py-5 backdrop-blur sm:px-7">
              <div>
                <div className="flex items-center gap-2 font-mono text-[10px] font-bold uppercase tracking-[.22em] text-amber-300"><ArchiveRestore className="h-4 w-4" /> Local recovery</div>
                <h2 id="recovery-title" className="mt-2 font-display text-2xl font-bold">Continue from a safe copy</h2>
                <p className="mt-2 max-w-2xl text-xs leading-5 text-stone-400">Autosaved app copies stay on this device for seven days. Preview, restore into a new session, or explicitly delete them. Your imported original is never changed.</p>
              </div>
              <button ref={closeRef} type="button" aria-label="Close recovery drafts" onClick={() => { setOpen(false); triggerRef.current?.focus(); }} className="rounded-full border border-white/10 p-2 text-stone-400 transition hover:bg-white/8 hover:text-white"><X className="h-5 w-5" /></button>
            </header>

            <div className="grid gap-5 p-5 sm:p-7 lg:grid-cols-[minmax(0,1fr)_minmax(260px,.8fr)]">
              <div className="space-y-3">
                {drafts.length === 0 && (
                  <div className="rounded-2xl border border-dashed border-white/12 bg-white/[.025] p-8 text-center">
                    <FileClock className="mx-auto h-8 w-8 text-stone-600" />
                    <p className="mt-3 font-display text-sm font-bold">No recovery copies waiting</p>
                    <p className="mt-1 text-xs text-stone-500">New work is checkpointed locally after edits and before export.</p>
                  </div>
                )}
                {drafts.map((draft, index) => {
                  const active = selectedId === draft.recovery_id;
                  const busy = busyId === draft.recovery_id;
                  return (
                    <article key={draft.recovery_id} className={`rounded-2xl border p-4 transition ${active ? 'border-amber-300/35 bg-amber-300/[.07]' : 'border-white/8 bg-white/[.025] hover:border-white/15'}`}>
                      <button type="button" className="w-full text-left" onClick={() => { setSelectedId(draft.recovery_id); setDeleteConfirmId(''); }}>
                        <div className="flex items-start justify-between gap-4">
                          <div>
                            <p className="font-display text-sm font-bold">Recovery copy {index + 1}</p>
                            <p className="mt-1 font-mono text-[10px] uppercase tracking-wider text-amber-200/60">{stageLabel(draft.stage)}</p>
                          </div>
                          <span className="rounded-full border border-white/10 px-2 py-1 text-[10px] text-stone-400">{draft.page_count} page{draft.page_count === 1 ? '' : 's'}</span>
                        </div>
                        <p className="mt-3 text-[11px] text-stone-500">{formatBytes(draft.bytes)} · {new Date(draft.last_modified).toLocaleString()} · checkpoint {draft.autosave_sequence}</p>
                      </button>
                      <div className="mt-4 grid grid-cols-2 gap-2">
                        <button type="button" disabled={busy} onClick={() => void restoreDraft(draft)} className="flex items-center justify-center gap-2 rounded-xl bg-amber-300 px-3 py-2.5 text-xs font-black text-stone-950 transition hover:bg-amber-200 disabled:opacity-50">{busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <RotateCcw className="h-4 w-4" />} Restore copy</button>
                        <button type="button" disabled={busy} onClick={() => void deleteDraft(draft)} className={`flex items-center justify-center gap-2 rounded-xl border px-3 py-2.5 text-xs font-bold transition disabled:opacity-50 ${deleteConfirmId === draft.recovery_id ? 'border-rose-400/40 bg-rose-400/15 text-rose-200' : 'border-white/10 text-stone-400 hover:border-rose-400/30 hover:text-rose-300'}`}><Trash2 className="h-4 w-4" /> {deleteConfirmId === draft.recovery_id ? 'Confirm delete' : 'Delete draft'}</button>
                      </div>
                    </article>
                  );
                })}
                {status && <p role="status" aria-live="polite" className="rounded-xl border border-white/8 bg-white/[.025] px-3 py-2 text-[11px] text-stone-400">{status}</p>}
              </div>

              <aside className="min-h-72 rounded-2xl border border-white/8 bg-black/30 p-4">
                <p className="font-mono text-[10px] font-bold uppercase tracking-[.18em] text-stone-500">First-page preview</p>
                <div className="mt-3 flex min-h-64 items-center justify-center overflow-hidden rounded-xl bg-stone-900/80 p-3 shadow-inner">
                  {previewUrl ? <img src={previewUrl} alt="Local first-page recovery preview" className="max-h-[55dvh] w-auto rounded bg-white shadow-2xl" /> : <p className="max-w-44 text-center text-xs leading-5 text-stone-600">Select a recovery copy to render its first page locally.</p>}
                </div>
              </aside>
            </div>
          </section>
        </div>
      )}
    </>
  );
}
