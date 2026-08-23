import { useEffect, useRef, useState } from 'react';
import axios from 'axios';
import {
  Activity,
  CheckCircle2,
  HardDrive,
  ShieldCheck,
  Trash2,
  TriangleAlert,
  X,
} from 'lucide-react';
import { API_BASE_URL } from '../lib/apiClient';
import { clearRecentFiles, loadRecentFiles } from '../services/recentFiles';

interface ToolCapability {
  available: boolean;
  path: string | null;
  languages?: string[];
  enables: string[];
}

interface RuntimeCapabilities {
  version: string;
  ready: boolean;
  all_optional_tools_available: boolean;
  runtime: { python: string; platform: string; architecture: string };
  network: {
    telemetry: boolean;
    api_auth_required: boolean;
    bind_host: string;
    processing: string;
  };
  external_tools: Record<string, ToolCapability>;
  storage: {
    session_bytes: number;
    temporary_bytes: number;
    scope: string;
  };
}

interface StorageInventory {
  session_files: number;
  active_sessions: number;
  session_bytes: number;
  report_files: number;
  report_bytes: number;
  draft_files: number;
  draft_bytes: number;
  temporary_files: number;
  temporary_bytes: number;
}

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const TOOL_LABELS: Record<string, string> = {
  libreoffice: 'LibreOffice',
  tesseract: 'Tesseract OCR',
  ghostscript: 'Ghostscript',
};

export default function RuntimeHealthPanel() {
  const [open, setOpen] = useState(false);
  const [capabilities, setCapabilities] = useState<RuntimeCapabilities | null>(null);
  const [error, setError] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteConfirmed, setDeleteConfirmed] = useState(false);
  const [inventory, setInventory] = useState<StorageInventory | null>(null);
  const [recentReferences, setRecentReferences] = useState(0);
  const [storageMessage, setStorageMessage] = useState('');
  const triggerRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const loadCapabilities = async () => {
    try {
      const response = await axios.get<RuntimeCapabilities>(`${API_BASE_URL}/api/capabilities`);
      setCapabilities(response.data);
      setError(false);
    } catch {
      setError(true);
    }
  };

  useEffect(() => {
    void loadCapabilities();
  }, []);

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

  const loadStorageInventory = async () => {
    try {
      const [response, recentFiles] = await Promise.all([
        axios.get(`${API_BASE_URL}/api/documents/maintenance/storage`),
        loadRecentFiles(),
      ]);
      setInventory(response.data?.data ?? null);
      setRecentReferences(recentFiles.length);
    } catch {
      setStorageMessage('Local storage inventory could not be refreshed.');
    }
  };

  useEffect(() => {
    if (open) void loadStorageInventory();
  }, [open]);

  const cleanup = async () => {
    setCleaning(true);
    try {
      await axios.post(`${API_BASE_URL}/api/documents/maintenance/cleanup`, {
        temp_max_age_minutes: 0,
        session_max_age_hours: 24,
        include_active_sessions: false,
      });
      await Promise.all([loadCapabilities(), loadStorageInventory()]);
      setStorageMessage('Stale app-owned files were cleaned.');
    } finally {
      setCleaning(false);
    }
  };

  const deleteAllLocalData = async () => {
    if (!deleteConfirmed) return;
    setDeleting(true);
    setStorageMessage('');
    try {
      await axios.post(`${API_BASE_URL}/api/documents/maintenance/cleanup`, {
        delete_all_app_data: true,
      });
      await clearRecentFiles();
      window.dispatchEvent(new CustomEvent('pdf-local-data-cleared'));
      setInventory({
        session_files: 0,
        active_sessions: 0,
        session_bytes: 0,
        report_files: 0,
        report_bytes: 0,
        draft_files: 0,
        draft_bytes: 0,
        temporary_files: 0,
        temporary_bytes: 0,
      });
      setRecentReferences(0);
      setDeleteConfirmed(false);
      setStorageMessage('All app-owned documents, reports, drafts, temporary files, and recent references were deleted.');
      await loadCapabilities();
    } catch {
      setStorageMessage('Some local data could not be deleted. Review the inventory and retry.');
    } finally {
      setDeleting(false);
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen(true)}
        className="group flex items-center gap-2 rounded-full border border-emerald-500/25 bg-emerald-500/8 px-2.5 py-2 text-[11px] font-semibold tracking-wide text-emerald-700 transition hover:border-emerald-500/50 hover:bg-emerald-500/12 sm:px-3 sm:py-1.5 dark:text-emerald-300"
        aria-label="Open local runtime status"
      >
        <span className={`h-2 w-2 rounded-full ${error ? 'bg-amber-400' : 'bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,.75)]'}`} />
        <span className="hidden sm:inline">{error ? 'Runtime offline' : 'On-device'}</span>
      </button>

      {open && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-slate-950/70 p-4 backdrop-blur-sm" role="presentation" onMouseDown={() => setOpen(false)}>
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="runtime-title"
            onMouseDown={(event) => event.stopPropagation()}
            className="max-h-[90dvh] w-full max-w-2xl overflow-y-auto rounded-[1.75rem] border border-emerald-400/15 bg-[#091510] text-slate-100 shadow-[0_30px_100px_rgba(0,0,0,.55)]"
          >
            <header className="relative overflow-hidden border-b border-white/10 px-6 py-6">
              <div className="absolute inset-0 opacity-40 [background-image:radial-gradient(circle_at_80%_0%,rgba(52,211,153,.35),transparent_45%)]" />
              <div className="relative flex items-start justify-between gap-4">
                <div>
                  <div className="mb-3 flex items-center gap-2 text-[11px] font-bold uppercase tracking-[.22em] text-emerald-300">
                    <ShieldCheck className="h-4 w-4" /> Local trust console
                  </div>
                  <h2 id="runtime-title" className="font-display text-2xl font-bold">Processed on this device</h2>
                  <p className="mt-2 max-w-lg text-sm leading-6 text-slate-400">The editor talks to a loopback API. Telemetry is disabled and document processing stays in this runtime.</p>
                </div>
                <button ref={closeRef} type="button" onClick={() => { setOpen(false); triggerRef.current?.focus(); }} className="rounded-full border border-white/10 p-2 text-slate-400 transition hover:bg-white/10 hover:text-white" aria-label="Close runtime status"><X className="h-4 w-4" /></button>
              </div>
            </header>

            {error || !capabilities ? (
              <div className="flex items-center gap-3 p-6 text-amber-200"><TriangleAlert className="h-5 w-5" /> The local API status could not be verified.</div>
            ) : (
              <div className="grid gap-5 p-6 md:grid-cols-[1.1fr_.9fr]">
                <div className="space-y-3">
                  <h3 className="text-xs font-bold uppercase tracking-[.18em] text-slate-500">Runtime capabilities</h3>
                  {Object.entries(capabilities.external_tools).map(([key, tool]) => (
                    <div key={key} className="flex items-start justify-between gap-4 rounded-2xl border border-white/8 bg-white/[.035] p-4">
                      <div>
                        <p className="text-sm font-semibold">{TOOL_LABELS[key] ?? key}</p>
                        <p className="mt-1 text-xs leading-5 text-slate-500">{tool.enables.join(' · ')}</p>
                      </div>
                      <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider ${tool.available ? 'bg-emerald-400/10 text-emerald-300' : 'bg-amber-400/10 text-amber-200'}`}>
                        {tool.available ? <CheckCircle2 className="h-3 w-3" /> : <TriangleAlert className="h-3 w-3" />}
                        {tool.available ? 'Ready' : 'Optional'}
                      </span>
                    </div>
                  ))}
                </div>

                <div className="space-y-4">
                  <div className="rounded-2xl border border-white/8 bg-black/20 p-4">
                    <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-[.16em] text-emerald-300"><Activity className="h-4 w-4" /> Network contract</div>
                    <dl className="mt-4 space-y-3 text-xs">
                      <div className="flex justify-between gap-4"><dt className="text-slate-500">Binding</dt><dd className="font-mono text-slate-200">{capabilities.network.bind_host}</dd></div>
                      <div className="flex justify-between gap-4"><dt className="text-slate-500">API token</dt><dd>{capabilities.network.api_auth_required ? 'Required' : 'Development mode'}</dd></div>
                      <div className="flex justify-between gap-4"><dt className="text-slate-500">Telemetry</dt><dd>{capabilities.network.telemetry ? 'Enabled' : 'Off'}</dd></div>
                    </dl>
                    <p className="mt-4 rounded-xl border border-emerald-300/10 bg-emerald-300/5 px-3 py-2 font-mono text-[10px] leading-5 text-emerald-100/70">Workspace → token-protected loopback → local API → app-owned storage</p>
                  </div>

                  <div className="rounded-2xl border border-white/8 bg-black/20 p-4">
                    <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-[.16em] text-cyan-300"><HardDrive className="h-4 w-4" /> Local storage</div>
                    <p className="mt-3 text-sm text-slate-300">{formatBytes(capabilities.storage.session_bytes + capabilities.storage.temporary_bytes)} in app-owned local storage.</p>
                    {inventory && (
                      <dl className="mt-4 space-y-2 border-t border-white/8 pt-3 text-[11px]">
                        <div className="flex justify-between gap-3"><dt className="text-slate-500">Session PDFs</dt><dd>{inventory.session_files} · {formatBytes(inventory.session_bytes)}</dd></div>
                        <div className="flex justify-between gap-3"><dt className="text-slate-500">Audit reports</dt><dd>{inventory.report_files} · {formatBytes(inventory.report_bytes)}</dd></div>
                        <div className="flex justify-between gap-3"><dt className="text-slate-500">Drafts / recovery</dt><dd>{inventory.draft_files} · {formatBytes(inventory.draft_bytes)}</dd></div>
                        <div className="flex justify-between gap-3"><dt className="text-slate-500">Temporary outputs</dt><dd>{inventory.temporary_files} · {formatBytes(inventory.temporary_bytes)}</dd></div>
                        <div className="flex justify-between gap-3"><dt className="text-slate-500">Recent references</dt><dd>{recentReferences}</dd></div>
                      </dl>
                    )}
                    <button type="button" onClick={() => void cleanup()} disabled={cleaning} className="mt-4 flex w-full items-center justify-center gap-2 rounded-xl border border-white/10 bg-white/5 px-3 py-2.5 text-xs font-semibold transition hover:bg-white/10 disabled:opacity-50"><Trash2 className="h-4 w-4" /> {cleaning ? 'Cleaning…' : 'Clean stale local data'}</button>
                    <label className="mt-3 flex items-start gap-2 rounded-xl border border-rose-300/10 bg-rose-300/5 p-3 text-[10px] leading-4 text-rose-100/70"><input type="checkbox" checked={deleteConfirmed} onChange={(event) => setDeleteConfirmed(event.target.checked)} className="mt-0.5 accent-rose-400" /><span>Close current documents and delete all app-owned sessions, reports, drafts, temporary files, and recent references.</span></label>
                    <button type="button" onClick={() => void deleteAllLocalData()} disabled={!deleteConfirmed || deleting} className="mt-2 flex w-full items-center justify-center gap-2 rounded-xl border border-rose-400/25 bg-rose-400/10 px-3 py-2.5 text-xs font-semibold text-rose-200 transition hover:bg-rose-400/20 disabled:cursor-not-allowed disabled:opacity-35"><Trash2 className="h-4 w-4" /> {deleting ? 'Deleting all local data…' : 'Delete all local workspace data'}</button>
                    {storageMessage && <p className="mt-3 text-[10px] leading-4 text-slate-400" role="status" aria-live="polite">{storageMessage}</p>}
                  </div>

                  <p className="text-[11px] leading-5 text-slate-600">Version {capabilities.version} · Python {capabilities.runtime.python} · {capabilities.runtime.platform}/{capabilities.runtime.architecture}</p>
                </div>
              </div>
            )}
          </section>
        </div>
      )}
    </>
  );
}
