import { useEffect, useMemo, useState } from 'react';
import axios from 'axios';
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronRight,
  Download,
  Eraser,
  FileJson,
  FileText,
  Layers3,
  LoaderCircle,
  RefreshCcw,
  ScanLine,
  Shield,
  Sparkles,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';

interface SanitizationProfile {
  id: string;
  label: string;
  description: string;
  rasterizes_pages: boolean;
  destructive_effects: string[];
}

interface Inventory {
  pages: number;
  file_bytes: number;
  metadata_fields: number;
  xml_metadata: number;
  attachments: number;
  annotations: number;
  links: number;
  form_fields: number;
  populated_form_fields: number;
  signature_fields: number;
  javascript_actions: number;
  thumbnails: number;
  layers: number;
  previous_revisions: number;
}

interface SanitizationPreview {
  profile: string;
  profile_label: string;
  description: string;
  before: Inventory;
  planned_removals: Record<string, number>;
  destructive_effects: string[];
  warnings: string[];
  source_will_be_preserved: boolean;
  preview_token: string;
}

interface SanitizationReport {
  status: 'completed' | 'completed_with_warnings';
  profile: string;
  profile_label: string;
  app_version: string;
  output_sha256: string;
  before: Inventory;
  after: Inventory;
  removed: Record<string, number>;
  destructive_effects: string[];
  warnings: string[];
}

interface SanitizationResult {
  status: string;
  source_preserved: boolean;
  copy: { id: string; filename: string; download_url: string };
  report: SanitizationReport;
  reports: { json: string; markdown: string };
}

const categoryLabels: Record<string, string> = {
  file_bytes: 'File size',
  metadata_fields: 'Metadata fields',
  xml_metadata: 'XML metadata',
  attachments: 'Attachments',
  annotations: 'Comments & annotations',
  links: 'Links',
  form_fields: 'Form fields',
  populated_form_fields: 'Populated form values',
  signature_fields: 'Signature fields',
  javascript_actions: 'JavaScript actions',
  thumbnails: 'Embedded thumbnails',
  layers: 'Layers',
  previous_revisions: 'Previous revisions',
};

const effectLabels: Record<string, string> = {
  metadata_provenance_removed: 'Authorship and provenance metadata will be removed.',
  comments_removed: 'Comments and annotations will be permanently removed.',
  attachments_removed: 'Embedded attachments will be permanently removed.',
  form_values_reset: 'Entered form values will be reset; form structure remains.',
  scripts_removed: 'Document JavaScript will be removed.',
  pending_redactions_applied: 'Any pending redaction annotations will be applied.',
  searchable_text_removed: 'Searchable text will be replaced by page images.',
  accessibility_tags_removed: 'Accessibility tags and semantic reading order will be lost.',
  bookmarks_removed: 'Bookmarks will not survive the rasterized copy.',
  forms_flattened: 'Interactive forms will be removed.',
  links_removed: 'Clickable links will be removed.',
  layers_flattened: 'Layers will be flattened into page images.',
  existing_signatures_invalidated: 'Existing digital signatures will no longer validate.',
  pages_rasterized_150_dpi: 'Every page will be rasterized at 150 DPI.',
};

const profileIcons = {
  minimal_metadata: Shield,
  collaboration_cleanup: Eraser,
  maximum_sanitization: Layers3,
};

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const displayValue = (category: string, value: number) =>
  category === 'file_bytes' ? formatBytes(value) : value.toLocaleString();

const SanitizeShareWorkflow = () => {
  const { sessionId } = useEditor();
  const [profiles, setProfiles] = useState<SanitizationProfile[]>([]);
  const [selectedProfile, setSelectedProfile] = useState('collaboration_cleanup');
  const [preview, setPreview] = useState<SanitizationPreview | null>(null);
  const [result, setResult] = useState<SanitizationResult | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [loading, setLoading] = useState<'profiles' | 'preview' | 'apply' | null>('profiles');
  const [message, setMessage] = useState('');

  useEffect(() => {
    let cancelled = false;
    const loadProfiles = async () => {
      try {
        const response = await axios.get(`${API_BASE_URL}/api/sanitization/profiles`);
        if (!cancelled) setProfiles(response.data?.data?.profiles ?? []);
      } catch {
        if (!cancelled) setMessage('Sanitization profiles could not be loaded from the local API.');
      } finally {
        if (!cancelled) setLoading(null);
      }
    };
    loadProfiles();
    return () => { cancelled = true; };
  }, []);

  const activeProfile = useMemo(
    () => profiles.find(profile => profile.id === selectedProfile),
    [profiles, selectedProfile],
  );

  const chooseProfile = (profileId: string) => {
    setSelectedProfile(profileId);
    setPreview(null);
    setResult(null);
    setAcknowledged(false);
    setMessage('');
  };

  const requestPreview = async () => {
    if (!sessionId) return;
    setLoading('preview');
    setMessage('');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/sanitize/preview`,
        { profile: selectedProfile },
      );
      setPreview(response.data.data as SanitizationPreview);
      setAcknowledged(false);
      setMessage('Preview ready. Review every planned removal and damage warning.');
    } catch {
      setMessage('The local preview failed. The source document was not changed.');
    } finally {
      setLoading(null);
    }
  };

  const applyProfile = async () => {
    if (!sessionId || !preview || !acknowledged) return;
    setLoading('apply');
    setMessage('Building and reopening a separate sanitized copy on this device…');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/sanitize/apply`,
        {
          profile: selectedProfile,
          preview_token: preview.preview_token,
          review_acknowledged: true,
        },
      );
      setResult(response.data.data as SanitizationResult);
      setMessage('Sanitized copy ready. The original document was preserved.');
    } catch {
      setMessage('Sanitization stopped safely. No new copy was offered.');
    } finally {
      setLoading(null);
    }
  };

  const downloadAsset = async (url: string, filename: string) => {
    try {
      const response = await axios.get(`${API_BASE_URL}${url}`, { responseType: 'blob' });
      await saveBlob(response.data, filename);
    } catch {
      setMessage('The local file could not be saved. It remains available in this session.');
    }
  };

  if (!sessionId) {
    return (
      <section className="min-h-full bg-[#f4f7f5] p-5 sm:p-10 flex items-center justify-center" aria-labelledby="sanitize-title">
        <div className="max-w-xl rounded-[2rem] border border-emerald-900/10 bg-white p-8 text-center shadow-xl">
          <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-emerald-950 text-emerald-200"><Sparkles className="h-7 w-7" /></div>
          <h2 id="sanitize-title" className="mt-5 font-display text-3xl font-bold text-slate-950">Sanitize &amp; Share</h2>
          <p className="mt-3 text-sm leading-6 text-slate-600">Upload a PDF to preview hidden structures and create a separate sharing copy with a content-free audit report.</p>
        </div>
      </section>
    );
  }

  return (
    <section className="min-h-full bg-[#f4f7f5] text-slate-950" aria-labelledby="sanitize-title">
      <div className="mx-auto max-w-7xl px-4 py-7 sm:px-8 sm:py-10">
        <header className="overflow-hidden rounded-[2rem] bg-emerald-950 p-6 text-white shadow-xl sm:p-9">
          <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
            <div className="max-w-2xl">
              <div className="inline-flex items-center gap-2 rounded-full border border-emerald-200/20 bg-emerald-200/10 px-3 py-1 text-xs font-bold uppercase tracking-[.16em] text-emerald-200"><ScanLine className="h-3.5 w-3.5" /> Content-free inspection</div>
              <h2 id="sanitize-title" className="mt-5 font-display text-4xl font-bold tracking-tight sm:text-5xl">Sanitize <span className="font-serif font-normal italic text-lime-200">before you share.</span></h2>
              <p className="mt-3 max-w-xl text-sm leading-6 text-emerald-50/65 sm:text-base">Choose the smallest cleanup profile that fits your risk. Preview exact removals and likely damage before a separate copy is created.</p>
            </div>
            <div className="rounded-2xl border border-emerald-100/15 bg-black/15 px-5 py-4 text-sm"><p className="text-emerald-100/55">Original document</p><p className="mt-1 flex items-center gap-2 font-bold text-lime-200"><CheckCircle2 className="h-4 w-4" /> Always preserved</p></div>
          </div>
        </header>

        {message && <div className={`my-6 rounded-2xl border px-4 py-3 text-sm ${result ? 'border-emerald-300 bg-emerald-50 text-emerald-900' : message.includes('failed') || message.includes('stopped') ? 'border-rose-200 bg-rose-50 text-rose-800' : 'border-sky-200 bg-sky-50 text-sky-800'}`} role={message.includes('failed') || message.includes('stopped') ? 'alert' : 'status'} aria-live="polite">{message}</div>}

        <div className="mt-6 grid gap-6 lg:grid-cols-[.8fr_1.2fr]">
          <div>
            <div className="mb-4 flex items-center justify-between"><div><p className="text-xs font-bold uppercase tracking-[.14em] text-emerald-800">01 · Choose a profile</p><h3 className="mt-1 font-display text-2xl font-bold">Cleanup depth</h3></div>{loading === 'profiles' && <LoaderCircle className="h-5 w-5 animate-spin text-emerald-800" />}</div>
            <div className="space-y-3" role="radiogroup" aria-label="Sanitization profile">
              {profiles.map(profile => {
                const Icon = profileIcons[profile.id as keyof typeof profileIcons] ?? Shield;
                const active = selectedProfile === profile.id;
                return (
                  <button key={profile.id} type="button" role="radio" aria-checked={active} onClick={() => chooseProfile(profile.id)} className={`w-full rounded-2xl border p-5 text-left transition ${active ? 'border-emerald-800 bg-emerald-950 text-white shadow-lg' : 'border-slate-200 bg-white hover:border-emerald-400'}`}>
                    <div className="flex items-start gap-4"><div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ${active ? 'bg-lime-200 text-emerald-950' : 'bg-emerald-50 text-emerald-800'}`}><Icon className="h-5 w-5" /></div><div className="min-w-0 flex-1"><div className="flex items-center justify-between gap-3"><span className="font-display text-lg font-bold">{profile.label}</span>{profile.rasterizes_pages && <span className="rounded-full bg-amber-200 px-2 py-1 text-[9px] font-black uppercase tracking-wider text-amber-950">Rasterizes</span>}</div><p className={`mt-1 text-sm leading-5 ${active ? 'text-emerald-50/65' : 'text-slate-600'}`}>{profile.description}</p><p className={`mt-3 text-xs font-semibold ${active ? 'text-lime-200' : 'text-emerald-800'}`}>{profile.destructive_effects.length} documented effect{profile.destructive_effects.length === 1 ? '' : 's'}</p></div></div>
                  </button>
                );
              })}
            </div>
            <button type="button" onClick={requestPreview} disabled={!activeProfile || loading !== null} className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-emerald-700 px-5 py-3 text-sm font-bold text-white hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-40">{loading === 'preview' ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <ScanLine className="h-4 w-4" />} Preview this profile</button>
          </div>

          <div className="rounded-[2rem] border border-slate-200 bg-white p-5 shadow-sm sm:p-7">
            {!preview && !result ? (
              <div className="flex min-h-[480px] flex-col items-center justify-center text-center"><div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-slate-100 text-slate-400"><ChevronRight className="h-7 w-7" /></div><h3 className="mt-5 font-display text-2xl font-bold">Preview before applying</h3><p className="mt-2 max-w-md text-sm leading-6 text-slate-500">The preview inventories sensitive structures by count and shows exactly what the chosen profile plans to remove. It never displays document values.</p></div>
            ) : result ? (
              <ResultPanel result={result} downloadAsset={downloadAsset} onReset={() => { setResult(null); setPreview(null); setAcknowledged(false); setMessage(''); }} />
            ) : preview && (
              <div>
                <div className="flex flex-col gap-3 border-b border-slate-200 pb-5 sm:flex-row sm:items-start sm:justify-between"><div><p className="text-xs font-bold uppercase tracking-[.14em] text-emerald-700">02 · Review preview</p><h3 className="mt-1 font-display text-2xl font-bold">{preview.profile_label}</h3><p className="mt-1 text-sm text-slate-500">{Object.keys(preview.planned_removals).length} removal categories planned</p></div><span className="inline-flex items-center gap-1.5 self-start rounded-full bg-emerald-50 px-3 py-1.5 text-xs font-bold text-emerald-800"><Check className="h-3.5 w-3.5" /> Source preserved</span></div>

                <div className="mt-5 overflow-hidden rounded-2xl border border-slate-200">
                  <div className="grid grid-cols-[1fr_auto] bg-slate-50 px-4 py-2 text-[10px] font-bold uppercase tracking-wider text-slate-500"><span>Planned removal</span><span>Detected</span></div>
                  {Object.entries(preview.planned_removals).map(([category, count]) => <div key={category} className="grid grid-cols-[1fr_auto] items-center border-t border-slate-100 px-4 py-3 text-sm"><span className="text-slate-700">{categoryLabels[category] ?? category.replaceAll('_', ' ')}</span><span className="rounded-lg bg-rose-50 px-2 py-1 font-mono text-xs font-bold text-rose-700">{displayValue(category, count)}</span></div>)}
                </div>

                {preview.warnings.length > 0 && <div className="mt-5 rounded-2xl border border-amber-200 bg-amber-50 p-4"><div className="flex items-center gap-2 font-bold text-amber-950"><AlertTriangle className="h-4 w-4" /> Capabilities that may be damaged</div><ul className="mt-3 space-y-2">{preview.warnings.map(effect => <li key={effect} className="flex gap-2 text-xs leading-5 text-amber-900"><span aria-hidden="true">•</span>{effectLabels[effect] ?? effect.replaceAll('_', ' ')}</li>)}</ul></div>}

                <label className="mt-5 flex cursor-pointer items-start gap-3 rounded-2xl border border-slate-200 p-4 text-sm leading-5 text-slate-700"><input type="checkbox" checked={acknowledged} onChange={event => setAcknowledged(event.target.checked)} className="mt-0.5 h-4 w-4 accent-emerald-700" /><span>I reviewed the planned removals and understand the listed capability damage. Create a separate copy; do not replace the source.</span></label>
                <button type="button" onClick={applyProfile} disabled={!acknowledged || loading !== null} className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-emerald-950 px-5 py-3 text-sm font-bold text-white hover:bg-emerald-900 disabled:cursor-not-allowed disabled:opacity-40">{loading === 'apply' ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <Sparkles className="h-4 w-4" />} Create sanitized copy</button>
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
};

const ResultPanel = ({ result, downloadAsset, onReset }: { result: SanitizationResult; downloadAsset: (url: string, filename: string) => Promise<void>; onReset: () => void }) => {
  const changed = Object.entries(result.report.removed).filter(([, removed]) => removed > 0);
  return (
    <div>
      <div className="flex items-start gap-4"><div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-emerald-100 text-emerald-800"><CheckCircle2 className="h-6 w-6" /></div><div><p className="text-xs font-bold uppercase tracking-[.14em] text-emerald-700">03 · Sharing copy ready</p><h3 className="mt-1 font-display text-2xl font-bold">{result.report.profile_label}</h3><p className="mt-1 text-sm text-slate-500">Reopened after save · source preserved</p></div></div>
      <div className="mt-6 overflow-hidden rounded-2xl border border-slate-200"><div className="grid grid-cols-[1fr_repeat(3,auto)] gap-3 bg-slate-50 px-4 py-2 text-[10px] font-bold uppercase tracking-wider text-slate-500"><span>Category</span><span>Before</span><span>After</span><span>Removed</span></div>{changed.map(([category, removed]) => <div key={category} className="grid grid-cols-[1fr_repeat(3,auto)] items-center gap-3 border-t border-slate-100 px-4 py-3 text-xs"><span className="min-w-0 text-slate-700">{categoryLabels[category] ?? category.replaceAll('_', ' ')}</span><span className="font-mono text-slate-500">{displayValue(category, result.report.before[category as keyof Inventory])}</span><span className="font-mono text-slate-900">{displayValue(category, result.report.after[category as keyof Inventory])}</span><span className="font-mono font-bold text-emerald-700">−{displayValue(category, removed)}</span></div>)}</div>
      <div className="mt-5 rounded-xl bg-slate-950 p-4 text-white"><p className="text-[10px] font-bold uppercase tracking-wider text-slate-500">Output SHA-256</p><p className="mt-2 break-all font-mono text-[11px] leading-5 text-slate-300">{result.report.output_sha256}</p></div>
      <div className="mt-5 grid gap-3 sm:grid-cols-3"><button type="button" onClick={() => downloadAsset(result.copy.download_url, result.copy.filename)} className="inline-flex items-center justify-center gap-2 rounded-xl bg-emerald-700 px-3 py-3 text-sm font-bold text-white hover:bg-emerald-800"><Download className="h-4 w-4" /> PDF copy</button><button type="button" onClick={() => downloadAsset(result.reports.json, 'privacy-report.json')} className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-200 px-3 py-3 text-sm font-bold hover:bg-slate-50"><FileJson className="h-4 w-4" /> JSON</button><button type="button" onClick={() => downloadAsset(result.reports.markdown, 'privacy-report.md')} className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-200 px-3 py-3 text-sm font-bold hover:bg-slate-50"><FileText className="h-4 w-4" /> Markdown</button></div>
      <button type="button" onClick={onReset} className="mt-5 inline-flex items-center gap-2 text-sm font-bold text-slate-500 hover:text-slate-900"><RefreshCcw className="h-4 w-4" /> Sanitize another copy</button>
    </div>
  );
};

export default SanitizeShareWorkflow;
