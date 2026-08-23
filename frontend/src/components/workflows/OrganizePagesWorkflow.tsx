import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import axios from 'axios';
import {
  CheckSquare,
  ChevronLeft,
  ChevronRight,
  Copy,
  Crop,
  Download,
  FileInput,
  GripVertical,
  Loader2,
  Redo2,
  RotateCcw,
  RotateCw,
  Trash2,
  Undo2,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';

type BatchAction = 'rotate_left' | 'rotate_right' | 'delete' | 'duplicate' | 'crop';

const warningLabels: Record<string, string> = {
  existing_signatures_will_be_invalidated: 'Existing digital signatures will no longer validate.',
  document_reading_order_changes: 'The document reading order changes and should be reviewed.',
  crop_hides_content_without_removing_it: 'Cropping hides content; it does not securely remove it.',
  bookmarks_may_require_review: 'Bookmarks may need destination review.',
  form_field_identity_may_change: 'Duplicated or moved form fields may need identity review.',
  optional_content_layers_may_require_review: 'Optional-content layers may need review.',
  internal_links_may_require_review: 'Internal link destinations may need review.',
  page_labels_may_require_review: 'Page labels may need review.',
  inserted_bookmarks_are_not_imported: 'Bookmarks from inserted PDFs are not imported.',
  inserted_signatures_will_not_remain_valid: 'Signatures in inserted PDFs will not remain valid.',
  inserted_form_fields_may_require_review: 'Inserted form fields may need review.',
};

const parseRange = (value: string, pageCount: number) => {
  const pages = new Set<number>();
  for (const rawPart of value.split(',')) {
    const part = rawPart.trim();
    if (!part) continue;
    const range = part.match(/^(\d+)\s*-\s*(\d+)$/);
    if (range) {
      const start = Math.max(1, Number(range[1]));
      const end = Math.min(pageCount, Number(range[2]));
      for (let page = Math.min(start, end); page <= Math.max(start, end); page += 1) {
        if (page <= pageCount) pages.add(page - 1);
      }
      continue;
    }
    const page = Number(part);
    if (Number.isInteger(page) && page >= 1 && page <= pageCount) pages.add(page - 1);
  }
  return pages;
};

function PagePreview({ documentId, page, version }: { documentId: string; page: number; version: number }) {
  const [image, setImage] = useState('');
  useEffect(() => {
    let cancelled = false;
    void axios.get(`${API_BASE_URL}/api/documents/${documentId}/pages/${page}`, {
      params: { zoom: 0.45 },
    }).then(response => {
      if (!cancelled) setImage(response.data?.data?.image ?? '');
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [documentId, page, version]);
  return image
    ? <img src={image} alt={`Page ${page + 1} preview`} className="h-full w-full object-contain" />
    : <div className="flex h-full items-center justify-center"><Loader2 className="h-5 w-5 animate-spin text-slate-300" /></div>;
}

export default function OrganizePagesWorkflow() {
  const {
    sessionId,
    pageCount,
    currentPage,
    setCurrentPage,
    setPageCount,
    documentMutationVersion,
    reportToolResult,
  } = useEditor();
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [range, setRange] = useState('');
  const [busy, setBusy] = useState('');
  const [warnings, setWarnings] = useState<string[]>([]);
  const [status, setStatus] = useState('');
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [showCrop, setShowCrop] = useState(false);
  const [crop, setCrop] = useState({ left: 0, top: 0, right: 0, bottom: 0 });
  const [insertPosition, setInsertPosition] = useState<'after-selection' | 'end'>('after-selection');
  const [dragged, setDragged] = useState<number | null>(null);
  const anchorRef = useRef<number | null>(null);
  const insertInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setSelected(current => new Set([...current].filter(page => page < pageCount)));
  }, [pageCount]);

  const selectedPages = useMemo(() => [...selected].sort((a, b) => a - b), [selected]);

  const togglePage = (page: number, extend = false) => {
    setSelected(current => {
      const next = new Set(current);
      if (extend && anchorRef.current !== null) {
        const start = Math.min(anchorRef.current, page);
        const end = Math.max(anchorRef.current, page);
        for (let index = start; index <= end; index += 1) next.add(index);
      } else if (next.has(page)) {
        next.delete(page);
      } else {
        next.add(page);
      }
      return next;
    });
    anchorRef.current = page;
    setCurrentPage(page);
  };

  const applyResult = useCallback((data: { page_count?: number; new_page_count?: number; warnings?: string[]; can_undo?: boolean; can_redo?: boolean }, message: string) => {
    const nextCount = data.page_count ?? data.new_page_count;
    if (typeof nextCount === 'number') setPageCount(nextCount);
    setWarnings(data.warnings ?? []);
    setCanUndo(Boolean(data.can_undo));
    setCanRedo(Boolean(data.can_redo));
    setStatus(message);
    reportToolResult('success', message, true);
  }, [reportToolResult, setPageCount]);

  const runBatch = async (action: BatchAction) => {
    if (!sessionId || selectedPages.length === 0) return;
    if (action === 'delete' && selectedPages.length === pageCount) {
      setStatus('Keep at least one page in the PDF.');
      return;
    }
    setBusy(action);
    setStatus('');
    try {
      const response = await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/pages/organize`, {
        action,
        pages: selectedPages,
        crop_left: crop.left,
        crop_top: crop.top,
        crop_right: crop.right,
        crop_bottom: crop.bottom,
      });
      applyResult(response.data?.data ?? {}, `${action.replace('_', ' ')} completed for ${selectedPages.length} page${selectedPages.length === 1 ? '' : 's'}.`);
      if (action === 'delete' || action === 'duplicate') setSelected(new Set());
      if (action === 'crop') setShowCrop(false);
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'The page operation could not be completed.');
    } finally {
      setBusy('');
    }
  };

  const applyOrder = async (order: number[], message: string) => {
    if (!sessionId || order.every((page, index) => page === index)) return;
    setBusy('reorder');
    try {
      const response = await axios.put(`${API_BASE_URL}/api/documents/${sessionId}/pages/reorder`, {
        page_order: order,
      });
      applyResult(response.data?.data ?? {}, message);
      const newSelection = new Set<number>();
      order.forEach((oldIndex, newIndex) => { if (selected.has(oldIndex)) newSelection.add(newIndex); });
      setSelected(newSelection);
    } catch {
      setStatus('Pages could not be reordered.');
    } finally {
      setBusy('');
    }
  };

  const moveSelection = (direction: -1 | 1) => {
    if (!selected.size) return;
    const order = Array.from({ length: pageCount }, (_, index) => index);
    if (direction === -1) {
      for (const page of selectedPages) {
        if (page > 0 && !selected.has(page - 1)) [order[page - 1], order[page]] = [order[page], order[page - 1]];
      }
    } else {
      for (const page of [...selectedPages].reverse()) {
        if (page < pageCount - 1 && !selected.has(page + 1)) [order[page], order[page + 1]] = [order[page + 1], order[page]];
      }
    }
    void applyOrder(order, `Moved ${selected.size} selected page${selected.size === 1 ? '' : 's'} ${direction === -1 ? 'left' : 'right'}.`);
  };

  const dropAt = (target: number) => {
    if (dragged === null) return;
    const moving = selected.has(dragged) ? selectedPages : [dragged];
    const remaining = Array.from({ length: pageCount }, (_, index) => index).filter(page => !moving.includes(page));
    const insertion = remaining.filter(page => page < target).length;
    remaining.splice(insertion, 0, ...moving);
    setDragged(null);
    void applyOrder(remaining, `Reordered ${moving.length} page${moving.length === 1 ? '' : 's'}.`);
  };

  const history = async (direction: 'undo' | 'redo') => {
    if (!sessionId) return;
    setBusy(direction);
    try {
      const response = await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/pages/organize/${direction}`);
      applyResult(response.data?.data ?? {}, `${direction === 'undo' ? 'Undid' : 'Redid'} ${response.data?.data?.operation?.replace('_', ' ') ?? 'page operation'}.`);
      setSelected(new Set());
    } catch {
      setStatus(`Nothing is available to ${direction}.`);
    } finally {
      setBusy('');
    }
  };

  const extract = async () => {
    if (!sessionId || !selectedPages.length) return;
    setBusy('extract');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/extract`,
        { pages: selectedPages },
        { responseType: 'blob' },
      );
      await saveBlob(response.data, 'extracted-pages.pdf');
      setStatus(`Saved a copy containing ${selectedPages.length} selected page${selectedPages.length === 1 ? '' : 's'}.`);
    } catch {
      setStatus('Selected pages could not be extracted.');
    } finally {
      setBusy('');
    }
  };

  const insertFiles = async (files: FileList | null) => {
    if (!sessionId || !files?.length) return;
    setBusy('insert');
    let position = insertPosition === 'end' || !selectedPages.length ? pageCount : selectedPages[selectedPages.length - 1] + 1;
    let workingCount = pageCount;
    try {
      for (const file of Array.from(files)) {
        const form = new FormData();
        form.append('file', file);
        const response = await axios.post(
          `${API_BASE_URL}/api/documents/${sessionId}/pages/insert`,
          form,
          { params: { position } },
        );
        const data = response.data?.data ?? {};
        const nextCount = data.new_page_count ?? workingCount;
        position += Math.max(nextCount - workingCount, 1);
        workingCount = nextCount;
        applyResult(data, `Inserted ${file.name} as an app-owned copy.`);
      }
      setSelected(new Set());
      if (insertInputRef.current) insertInputRef.current.value = '';
    } catch {
      setStatus('The selected PDF could not be inserted. Earlier completed insertions remain undoable.');
    } finally {
      setBusy('');
    }
  };

  if (!sessionId) {
    return (
      <div className="flex min-h-full items-center justify-center p-6">
        <div className="max-w-md rounded-3xl border border-dashed border-sky-300/30 bg-sky-50 p-8 text-center text-slate-700">
          <CheckSquare className="mx-auto h-10 w-10 text-sky-600" />
          <h2 className="mt-4 font-display text-2xl font-bold text-slate-950">Organize Pages</h2>
          <p className="mt-2 text-sm leading-6">Upload a PDF to select, reorder, rotate, delete, duplicate, extract, insert, merge, crop, and undo from one workspace.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-[#f6f8f4] p-3 text-slate-900 sm:p-6 lg:p-8">
      <div className="mx-auto max-w-7xl">
        <header className="flex flex-col gap-4 border-b border-slate-200 pb-6 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <p className="font-mono text-[10px] font-bold uppercase tracking-[.22em] text-sky-700">Workflow 01 · local page desk</p>
            <h2 className="mt-2 font-display text-3xl font-bold sm:text-4xl">Organize Pages</h2>
            <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-600">Select a set once, then act on it. Every mutation gets a complete local snapshot for undo; inserted sources and the imported original stay untouched.</p>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" onClick={() => void history('undo')} disabled={!canUndo || Boolean(busy)} aria-label="Undo page operation" className="rounded-xl border border-slate-200 bg-white p-3 text-slate-600 shadow-sm transition hover:border-sky-300 disabled:opacity-35"><Undo2 className="h-4 w-4" /></button>
            <button type="button" onClick={() => void history('redo')} disabled={!canRedo || Boolean(busy)} aria-label="Redo page operation" className="rounded-xl border border-slate-200 bg-white p-3 text-slate-600 shadow-sm transition hover:border-sky-300 disabled:opacity-35"><Redo2 className="h-4 w-4" /></button>
            <span className="rounded-xl bg-slate-950 px-4 py-3 font-mono text-xs font-bold text-white">{selected.size} / {pageCount} selected</span>
          </div>
        </header>

        <section aria-label="Selection controls" className="mt-5 flex flex-wrap items-center gap-2 rounded-2xl border border-slate-200 bg-white p-3 shadow-sm">
          <button type="button" onClick={() => setSelected(new Set(Array.from({ length: pageCount }, (_, index) => index)))} className="rounded-lg px-3 py-2 text-xs font-bold hover:bg-slate-100">All</button>
          <button type="button" onClick={() => setSelected(new Set(Array.from({ length: pageCount }, (_, index) => index).filter(index => index % 2 === 0)))} className="rounded-lg px-3 py-2 text-xs font-bold hover:bg-slate-100">Odd</button>
          <button type="button" onClick={() => setSelected(new Set(Array.from({ length: pageCount }, (_, index) => index).filter(index => index % 2 === 1)))} className="rounded-lg px-3 py-2 text-xs font-bold hover:bg-slate-100">Even</button>
          <button type="button" onClick={() => setSelected(new Set())} className="rounded-lg px-3 py-2 text-xs font-bold text-slate-500 hover:bg-slate-100">Clear</button>
          <div className="mx-1 hidden h-6 w-px bg-slate-200 sm:block" />
          <label className="flex min-w-48 flex-1 items-center gap-2 rounded-xl bg-slate-50 px-3 py-2 text-xs"><span className="font-bold text-slate-500">Range</span><input value={range} onChange={event => setRange(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') setSelected(parseRange(range, pageCount)); }} placeholder="1-3, 7, 10-12" className="min-w-0 flex-1 bg-transparent outline-none" /></label>
          <button type="button" onClick={() => setSelected(parseRange(range, pageCount))} className="rounded-xl bg-sky-100 px-3 py-2 text-xs font-black text-sky-900">Select range</button>
          <button type="button" onClick={() => moveSelection(-1)} disabled={!selected.size || Boolean(busy)} className="rounded-xl border border-slate-200 p-2.5 disabled:opacity-35" aria-label="Move selected pages left"><ChevronLeft className="h-4 w-4" /></button>
          <button type="button" onClick={() => moveSelection(1)} disabled={!selected.size || Boolean(busy)} className="rounded-xl border border-slate-200 p-2.5 disabled:opacity-35" aria-label="Move selected pages right"><ChevronRight className="h-4 w-4" /></button>
        </section>

        <section aria-label="Page operations" className="sticky top-0 z-10 mt-4 flex flex-wrap gap-2 rounded-2xl border border-slate-200 bg-white/95 p-3 shadow-lg shadow-slate-200/50 backdrop-blur">
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => void runBatch('rotate_left')} className="organize-action"><RotateCcw className="h-4 w-4" /> Rotate left</button>
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => void runBatch('rotate_right')} className="organize-action"><RotateCw className="h-4 w-4" /> Rotate right</button>
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => void runBatch('duplicate')} className="organize-action"><Copy className="h-4 w-4" /> Duplicate</button>
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => void extract()} className="organize-action"><Download className="h-4 w-4" /> Extract copy</button>
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => setShowCrop(value => !value)} className="organize-action"><Crop className="h-4 w-4" /> Crop</button>
          <button type="button" disabled={!selected.size || Boolean(busy)} onClick={() => void runBatch('delete')} className="organize-action !border-rose-200 !text-rose-700"><Trash2 className="h-4 w-4" /> Delete</button>
          <div className="ml-auto flex min-w-64 flex-1 items-center justify-end gap-2 sm:flex-none">
            <select value={insertPosition} onChange={event => setInsertPosition(event.target.value as typeof insertPosition)} className="min-w-0 rounded-xl border border-slate-200 bg-white px-2 py-2 text-[11px] font-bold"><option value="after-selection">After selection</option><option value="end">Merge at end</option></select>
            <label className="organize-action organize-action--primary cursor-pointer"><FileInput className="h-4 w-4" /> Insert / merge<input ref={insertInputRef} type="file" multiple accept="application/pdf,.pdf" className="sr-only" onChange={event => void insertFiles(event.target.files)} /></label>
          </div>
        </section>

        {showCrop && (
          <section className="mt-3 grid gap-3 rounded-2xl border border-amber-200 bg-amber-50 p-4 sm:grid-cols-[repeat(4,minmax(0,1fr))_auto]">
            {(['left', 'top', 'right', 'bottom'] as const).map(side => <label key={side} className="text-[10px] font-bold uppercase tracking-wider text-amber-900">{side} pt<input type="number" min="0" value={crop[side]} onChange={event => setCrop(current => ({ ...current, [side]: Math.max(0, Number(event.target.value)) }))} className="mt-1 w-full rounded-lg border border-amber-200 bg-white px-3 py-2 text-sm" /></label>)}
            <button type="button" onClick={() => void runBatch('crop')} disabled={Boolean(busy)} className="self-end rounded-xl bg-amber-900 px-4 py-2.5 text-xs font-black text-white">Apply non-destructive crop box</button>
          </section>
        )}

        {(status || warnings.length > 0) && (
          <section role="status" aria-live="polite" className="mt-4 rounded-2xl border border-slate-200 bg-white p-4 text-xs">
            {status && <p className="font-bold text-slate-800">{status}</p>}
            {warnings.length > 0 && <ul className="mt-2 space-y-1 text-amber-800">{warnings.map(warning => <li key={warning}>• {warningLabels[warning] ?? warning.replaceAll('_', ' ')}</li>)}</ul>}
          </section>
        )}

        <section aria-label="Page thumbnails" className="mt-5 grid grid-cols-2 gap-3 pb-16 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
          {Array.from({ length: pageCount }, (_, page) => {
            const active = selected.has(page);
            return (
              <article key={page} draggable onDragStart={() => setDragged(page)} onDragEnd={() => setDragged(null)} onDragOver={event => event.preventDefault()} onDrop={() => dropAt(page)} className={`group relative rounded-2xl border-2 p-2 shadow-sm transition ${active ? 'border-sky-500 bg-sky-50 ring-4 ring-sky-100' : 'border-white bg-white hover:border-slate-300'} ${dragged === page ? 'opacity-45' : ''}`}>
                <div className="flex items-center justify-between px-1 pb-2">
                  <label className="flex items-center gap-2 font-mono text-[10px] font-bold"><input type="checkbox" checked={active} onChange={event => togglePage(page, (event.nativeEvent as MouseEvent).shiftKey)} className="h-4 w-4 rounded accent-sky-600" aria-label={`Select page ${page + 1}`} /> {page + 1}</label>
                  <GripVertical className="h-4 w-4 cursor-grab text-slate-300 group-hover:text-slate-500" aria-hidden="true" />
                </div>
                <button type="button" onClick={event => togglePage(page, event.shiftKey)} onKeyDown={event => { if (event.key === ' ') { event.preventDefault(); togglePage(page, event.shiftKey); } }} className="aspect-[3/4] w-full overflow-hidden rounded-xl border border-slate-200 bg-slate-100" aria-label={`Open and toggle page ${page + 1}`}>
                  <PagePreview documentId={sessionId} page={page} version={documentMutationVersion} />
                </button>
                {currentPage === page && <span className="absolute bottom-4 left-4 rounded-full bg-slate-950 px-2 py-1 font-mono text-[8px] font-bold uppercase tracking-wider text-white">Current</span>}
              </article>
            );
          })}
        </section>
      </div>
    </div>
  );
}
