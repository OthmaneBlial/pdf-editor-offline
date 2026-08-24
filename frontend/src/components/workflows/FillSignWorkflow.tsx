import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import axios from 'axios';
import {
  Check,
  Download,
  Eraser,
  FileCheck2,
  FileSignature,
  Fingerprint,
  KeyRound,
  Loader2,
  PenLine,
  Redo2,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Type,
  Undo2,
  Upload,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';
import {
  deleteSignatureAsset,
  loadSignatureAssets,
  saveSignatureAsset,
  signatureAssetToFile,
  type SignatureAsset,
  type SignatureKind,
} from '../../services/signatureAssets';

interface FormField {
  id: string;
  page: number;
  name: string;
  label: string;
  value: string | null;
  type: string;
  field_type: 'text' | 'date' | 'checkbox' | 'radio' | 'dropdown' | 'listbox' | 'signature' | 'unknown';
  choices: string[];
  button_values: string[];
  read_only: boolean;
  required: boolean;
  tab_index: number;
}

interface FormInventory {
  fields: FormField[];
  field_count: number;
  has_xfa: boolean;
  javascript_actions: number;
  calculation_actions: number;
  signature_fields: number;
  warnings: string[];
}

interface DigitalSignatureResult {
  status: 'unsigned' | 'signed';
  signature_count: number;
  all_intact: boolean;
  all_cryptographically_valid: boolean;
  all_documents_unchanged_since_signature: boolean;
  all_trusted: boolean;
  trust_roots_supplied: boolean;
  trust_model: 'explicit_roots_only';
  network_fetching: false;
  revocation_status: 'not_checked_offline';
  signatures: Array<{
    index: number;
    field_name: string | null;
    intact: boolean;
    cryptographically_valid: boolean;
    trusted: boolean;
    trust_status: string;
    coverage: string | null;
    modification_level: string | null;
    document_unchanged_since_signature: boolean;
    certificate: null | {
      subject_common_name: string | null;
      issuer_common_name: string | null;
      sha256_fingerprint: string;
      valid_from: string | null;
      valid_until: string | null;
    };
  }>;
}

const warningLabels: Record<string, string> = {
  xfa_unsupported: 'XFA is detected and cannot be edited or flattened safely.',
  javascript_not_executed: 'Embedded JavaScript is not executed by this local editor.',
  calculations_not_executed: 'Automatic PDF calculations are not executed; review calculated fields.',
  existing_signatures_will_be_invalidated: 'Changing this PDF invalidates existing digital signatures.',
  visual_signature_is_not_digital_signature: 'This is a visual mark, not a certificate-backed digital signature.',
  editable_original_preserved: 'The editable original remains in the current workspace.',
};

const emptyInventory: FormInventory = {
  fields: [],
  field_count: 0,
  has_xfa: false,
  javascript_actions: 0,
  calculation_actions: 0,
  signature_fields: 0,
  warnings: [],
};

const createTypedSignature = (value: string) => {
  const canvas = document.createElement('canvas');
  canvas.width = 720;
  canvas.height = 180;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Canvas is unavailable');
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = '#0f172a';
  context.font = 'italic 72px "Instrument Serif", Georgia, serif';
  context.textBaseline = 'middle';
  context.fillText(value.trim(), 28, canvas.height / 2, canvas.width - 56);
  context.strokeStyle = '#0f172a';
  context.lineWidth = 2;
  context.beginPath();
  context.moveTo(24, 145);
  context.lineTo(canvas.width - 24, 145);
  context.stroke();
  return canvas.toDataURL('image/png');
};

export default function FillSignWorkflow() {
  const {
    sessionId,
    pageCount,
    currentPage,
    documentMutationVersion,
    reportToolResult,
  } = useEditor();
  const [inventory, setInventory] = useState<FormInventory>(emptyInventory);
  const [values, setValues] = useState<Record<string, string>>({});
  const [warnings, setWarnings] = useState<string[]>([]);
  const [status, setStatus] = useState('');
  const [busy, setBusy] = useState('');
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [assets, setAssets] = useState<SignatureAsset[]>(() => loadSignatureAssets());
  const [selectedAssetId, setSelectedAssetId] = useState('');
  const [signatureMode, setSignatureMode] = useState<SignatureKind>('typed');
  const [typedName, setTypedName] = useState('');
  const [hasInk, setHasInk] = useState(false);
  const [placement, setPlacement] = useState({ page: currentPage + 1, x: 72, y: 500, width: 180, height: 60 });
  const [certificateFile, setCertificateFile] = useState<File | null>(null);
  const [certificatePassphrase, setCertificatePassphrase] = useState('');
  const [trustRootFile, setTrustRootFile] = useState<File | null>(null);
  const [digitalValidation, setDigitalValidation] = useState<DigitalSignatureResult | null>(null);
  const [digitalDetails, setDigitalDetails] = useState({
    fieldName: 'OfflineSignature',
    reason: '',
    location: '',
    page: currentPage + 1,
    x0: 72,
    y0: 72,
    x1: 300,
    y1: 132,
  });
  const drawCanvasRef = useRef<HTMLCanvasElement>(null);
  const drawingRef = useRef(false);
  const certificateInputRef = useRef<HTMLInputElement>(null);

  const selectedAsset = useMemo(
    () => assets.find(asset => asset.id === selectedAssetId) ?? null,
    [assets, selectedAssetId],
  );
  const visibleWarnings = useMemo(
    () => [...new Set([...(inventory.warnings ?? []), ...warnings])],
    [inventory.warnings, warnings],
  );

  const loadForms = useCallback(async () => {
    if (!sessionId) {
      setInventory(emptyInventory);
      setValues({});
      return;
    }
    setBusy('loading');
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/forms`);
      const data = response.data?.data as FormInventory;
      setInventory(data);
      setValues(Object.fromEntries(data.fields.map(field => [field.name, field.value ?? ''])));
    } catch {
      setStatus('Form fields could not be inspected.');
    } finally {
      setBusy('');
    }
  }, [sessionId]);

  useEffect(() => {
    void loadForms();
  }, [loadForms, documentMutationVersion]);

  useEffect(() => {
    const clearLocalAssets = () => {
      setAssets([]);
      setSelectedAssetId('');
    };
    window.addEventListener('pdf-local-data-cleared', clearLocalAssets);
    return () => window.removeEventListener('pdf-local-data-cleared', clearLocalAssets);
  }, []);

  const rememberAsset = (kind: SignatureKind, dataUrl: string) => {
    try {
      const next = saveSignatureAsset(kind, dataUrl);
      setAssets(next);
      setSelectedAssetId(next[0]?.id ?? '');
      setStatus(`${kind[0].toUpperCase()}${kind.slice(1)} visual signature saved only in this browser profile.`);
    } catch {
      setStatus('The visual signature could not be stored locally. Delete an older asset and retry.');
    }
  };

  const saveTyped = () => {
    if (!typedName.trim()) return;
    try {
      rememberAsset('typed', createTypedSignature(typedName));
      setTypedName('');
    } catch {
      setStatus('Typed signature rendering is unavailable in this browser.');
    }
  };

  const canvasPoint = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = drawCanvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const bounds = canvas.getBoundingClientRect();
    return {
      x: (event.clientX - bounds.left) * (canvas.width / bounds.width),
      y: (event.clientY - bounds.top) * (canvas.height / bounds.height),
    };
  };

  const startDrawing = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = drawCanvasRef.current;
    const context = canvas?.getContext('2d');
    if (!canvas || !context) return;
    drawingRef.current = true;
    canvas.setPointerCapture(event.pointerId);
    const point = canvasPoint(event);
    context.beginPath();
    context.moveTo(point.x, point.y);
  };

  const continueDrawing = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawingRef.current) return;
    const context = drawCanvasRef.current?.getContext('2d');
    if (!context) return;
    const point = canvasPoint(event);
    context.strokeStyle = '#0f172a';
    context.lineWidth = 5;
    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.lineTo(point.x, point.y);
    context.stroke();
    setHasInk(true);
  };

  const stopDrawing = () => {
    drawingRef.current = false;
  };

  const clearDrawing = () => {
    const canvas = drawCanvasRef.current;
    canvas?.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height);
    setHasInk(false);
  };

  const saveDrawing = () => {
    const canvas = drawCanvasRef.current;
    if (canvas && hasInk) {
      rememberAsset('drawn', canvas.toDataURL('image/png'));
      clearDrawing();
    }
  };

  const importSignature = (file: File | null) => {
    if (!file) return;
    if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type) || file.size > 750 * 1024) {
      setStatus('Import a PNG, JPEG, or WebP signature no larger than 750 KB.');
      return;
    }
    const reader = new FileReader();
    reader.onload = () => rememberAsset('imported', String(reader.result));
    reader.onerror = () => setStatus('The signature image could not be read.');
    reader.readAsDataURL(file);
  };

  const saveFields = async () => {
    if (!sessionId || !inventory.fields.length) return;
    const unique = new Map<string, string>();
    inventory.fields.forEach(field => {
      if (!field.read_only && field.field_type !== 'signature') unique.set(field.name, values[field.name] ?? '');
    });
    setBusy('forms');
    try {
      const response = await axios.put(`${API_BASE_URL}/api/documents/${sessionId}/forms`, {
        fields: [...unique].map(([name, value]) => ({ name, value })),
      });
      const data = response.data?.data ?? {};
      setWarnings(data.warnings ?? []);
      setCanUndo(Boolean(data.can_undo));
      setCanRedo(Boolean(data.can_redo));
      setStatus(`Saved ${unique.size} standard form field${unique.size === 1 ? '' : 's'} locally.`);
      reportToolResult('success', 'Form fields saved', true);
      await loadForms();
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'Form values could not be saved.');
    } finally {
      setBusy('');
    }
  };

  const flattenCopy = async () => {
    if (!sessionId) return;
    setBusy('flatten');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/forms/flatten-copy`,
        undefined,
        { responseType: 'blob' },
      );
      await saveBlob(response.data, 'flattened-sharing-copy.pdf');
      setWarnings((response.headers?.['x-pdf-editor-warnings'] ?? '').split(',').filter(Boolean));
      setStatus(`Saved a flattened sharing copy with ${response.headers?.['x-fields-flattened'] ?? inventory.field_count} field appearances. The editable original is still open.`);
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'A flattened sharing copy could not be created.');
    } finally {
      setBusy('');
    }
  };

  const applyVisualSignature = async () => {
    if (!sessionId || !selectedAsset) return;
    setBusy('signature');
    try {
      const file = await signatureAssetToFile(selectedAsset);
      const form = new FormData();
      form.append('signature', file);
      form.append('page_num', String(placement.page - 1));
      form.append('x', String(placement.x));
      form.append('y', String(placement.y));
      form.append('width', String(placement.width));
      form.append('height', String(placement.height));
      const response = await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/visual-signatures`, form);
      const data = response.data?.data ?? {};
      setWarnings(data.warnings ?? []);
      setCanUndo(Boolean(data.can_undo));
      setCanRedo(Boolean(data.can_redo));
      setStatus(`Placed a visual signature on page ${placement.page}. It does not prove identity.`);
      reportToolResult('success', 'Visual signature placed', true);
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'The visual signature could not be placed.');
    } finally {
      setBusy('');
    }
  };

  const createCertificateSignedCopy = async () => {
    if (!sessionId || !certificateFile) return;
    setBusy('certificate-sign');
    try {
      const form = new FormData();
      form.append('certificate', certificateFile);
      form.append('passphrase', certificatePassphrase);
      form.append('field_name', digitalDetails.fieldName);
      form.append('reason', digitalDetails.reason);
      form.append('location', digitalDetails.location);
      form.append('page', String(digitalDetails.page - 1));
      form.append('x0', String(digitalDetails.x0));
      form.append('y0', String(digitalDetails.y0));
      form.append('x1', String(digitalDetails.x1));
      form.append('y1', String(digitalDetails.y1));
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/digital-signatures/sign-copy`,
        form,
        { responseType: 'blob' },
      );
      await saveBlob(response.data, 'certificate-signed-copy.pdf');
      setStatus('A separate certificate-signed copy was created. Reopen that copy here to validate it.');
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'The certificate-signed copy could not be created. Check the P12/PFX and passphrase.');
    } finally {
      setCertificatePassphrase('');
      setCertificateFile(null);
      if (certificateInputRef.current) certificateInputRef.current.value = '';
      setBusy('');
    }
  };

  const validateDigitalSignatures = async () => {
    if (!sessionId) return;
    setBusy('certificate-validate');
    try {
      const form = new FormData();
      if (trustRootFile) form.append('trust_roots', trustRootFile);
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/digital-signatures/validate`,
        form,
      );
      const result = response.data?.data as DigitalSignatureResult;
      setDigitalValidation(result);
      setStatus(result.status === 'unsigned' ? 'No embedded digital signature was found.' : `Inspected ${result.signature_count} digital signature${result.signature_count === 1 ? '' : 's'} entirely offline.`);
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      setStatus(detail || 'Digital signatures could not be inspected safely.');
    } finally {
      setBusy('');
    }
  };

  const history = async (direction: 'undo' | 'redo') => {
    if (!sessionId) return;
    setBusy(direction);
    try {
      const response = await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/pages/organize/${direction}`);
      const data = response.data?.data ?? {};
      setCanUndo(Boolean(data.can_undo));
      setCanRedo(Boolean(data.can_redo));
      setStatus(`${direction === 'undo' ? 'Undid' : 'Redid'} ${data.operation?.replaceAll('_', ' ') ?? 'the last edit'}.`);
      reportToolResult('success', `${direction === 'undo' ? 'Undo' : 'Redo'} complete`, true);
      await loadForms();
    } catch {
      setStatus(`Nothing is available to ${direction}.`);
    } finally {
      setBusy('');
    }
  };

  if (!sessionId) {
    return (
      <div className="flex min-h-full items-center justify-center bg-[#f4f1e9] p-6">
        <div className="max-w-md rounded-3xl border border-dashed border-indigo-300 bg-white p-8 text-center">
          <FileSignature className="mx-auto h-11 w-11 text-indigo-700" />
          <h2 className="mt-4 font-display text-3xl font-bold">Fill & Sign</h2>
          <p className="mt-2 text-sm leading-6 text-slate-600">Upload a PDF to fill standard AcroForms, create a flattened sharing copy, or place a clearly labelled visual signature.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-[#f4f1e9] p-3 text-slate-950 sm:p-6 lg:p-8">
      <div className="mx-auto max-w-7xl">
        <header className="grid gap-5 border-b border-slate-300 pb-6 lg:grid-cols-[1fr_auto] lg:items-end">
          <div>
            <p className="font-mono text-[10px] font-bold uppercase tracking-[.24em] text-indigo-700">Workflow 02 · standard forms + visual marks</p>
            <h2 className="mt-2 font-display text-4xl font-bold">Fill & Sign</h2>
            <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">Complete standard AcroForms in a predictable tab order. Flatten only a separate sharing copy. Visual signatures are local images—not identity, trust, or certificate validation.</p>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" onClick={() => void history('undo')} disabled={!canUndo || Boolean(busy)} aria-label="Undo Fill and Sign edit" className="rounded-xl border border-slate-300 bg-white p-3 disabled:opacity-35"><Undo2 className="h-4 w-4" /></button>
            <button type="button" onClick={() => void history('redo')} disabled={!canRedo || Boolean(busy)} aria-label="Redo Fill and Sign edit" className="rounded-xl border border-slate-300 bg-white p-3 disabled:opacity-35"><Redo2 className="h-4 w-4" /></button>
            <span className="rounded-xl bg-indigo-950 px-4 py-3 font-mono text-[10px] font-bold uppercase tracking-wider text-indigo-100">Local only</span>
          </div>
        </header>

        {(status || visibleWarnings.length > 0) && (
          <section role="status" aria-live="polite" className="mt-4 rounded-2xl border border-indigo-200 bg-white p-4 text-xs shadow-sm">
            {status && <p className="font-bold text-slate-900">{status}</p>}
            {visibleWarnings.length > 0 && <ul className="mt-2 space-y-1 text-amber-800">{visibleWarnings.map(warning => <li key={warning}>• {warningLabels[warning] ?? warning.replaceAll('_', ' ')}</li>)}</ul>}
          </section>
        )}

        <div className="mt-5 grid gap-5 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,.95fr)]">
          <section className="rounded-3xl border border-slate-200 bg-white p-4 shadow-sm sm:p-6" aria-labelledby="form-fields-heading">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <p className="font-mono text-[10px] font-bold uppercase tracking-[.2em] text-indigo-600">AcroForm desk</p>
                <h3 id="form-fields-heading" className="mt-1 font-display text-2xl font-bold">Fields in tab order</h3>
                <p className="mt-1 text-xs text-slate-500">{inventory.field_count} detected · {inventory.signature_fields} digital-signature field{inventory.signature_fields === 1 ? '' : 's'}</p>
              </div>
              <button type="button" onClick={() => void flattenCopy()} disabled={inventory.has_xfa || Boolean(busy)} className="flex items-center gap-2 rounded-xl bg-indigo-950 px-4 py-3 text-xs font-black text-white disabled:opacity-35"><Download className="h-4 w-4" /> Flatten sharing copy</button>
            </div>

            {busy === 'loading' ? (
              <div className="flex min-h-48 items-center justify-center"><Loader2 className="h-6 w-6 animate-spin text-indigo-500" /></div>
            ) : inventory.fields.length === 0 ? (
              <div className="mt-6 rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-8 text-center text-sm text-slate-500">No standard AcroForm fields were detected. You can still use a visual signature below.</div>
            ) : (
              <ol className="mt-6 grid gap-3 sm:grid-cols-2">
                {inventory.fields.map(field => (
                  <li key={field.id} className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
                    <div className="mb-2 flex items-start justify-between gap-2">
                      <label htmlFor={`field-${field.id}`} className="text-xs font-black text-slate-800">{field.tab_index}. {field.label || field.name}{field.required && <span className="text-rose-600"> *</span>}</label>
                      <span className="rounded-full bg-white px-2 py-1 font-mono text-[8px] font-bold uppercase text-slate-500">Page {field.page + 1} · {field.field_type}</span>
                    </div>
                    {field.field_type === 'checkbox' ? (
                      <label className="flex items-center gap-2 text-xs"><input id={`field-${field.id}`} type="checkbox" checked={!['', 'Off', 'false', '0'].includes(values[field.name] ?? '')} disabled={field.read_only} onChange={event => setValues(current => ({ ...current, [field.name]: event.target.checked ? 'true' : 'false' }))} className="h-5 w-5 accent-indigo-700" /> Checked</label>
                    ) : field.field_type === 'radio' ? (
                      <fieldset id={`field-${field.id}`} className="flex flex-wrap gap-4 text-xs">{field.choices.map(choice => <label key={choice}><input type="radio" name={field.id} checked={(values[field.name] ?? '') === choice} disabled={field.read_only} onChange={() => setValues(current => ({ ...current, [field.name]: choice }))} className="mr-1 accent-indigo-700" /> {choice}</label>)}</fieldset>
                    ) : ['dropdown', 'listbox'].includes(field.field_type) ? (
                      <select id={`field-${field.id}`} value={values[field.name] ?? ''} disabled={field.read_only} onChange={event => setValues(current => ({ ...current, [field.name]: event.target.value }))} className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2.5 text-sm"><option value="">Select…</option>{field.choices.map(choice => <option key={choice} value={choice}>{choice}</option>)}</select>
                    ) : field.field_type === 'signature' ? (
                      <div id={`field-${field.id}`} className="flex items-center gap-2 rounded-xl border border-amber-200 bg-amber-50 px-3 py-2.5 text-xs text-amber-900"><ShieldAlert className="h-4 w-4" /> Certificate field is inspect-only here.</div>
                    ) : (
                      <input id={`field-${field.id}`} type={field.field_type === 'date' ? 'date' : 'text'} value={values[field.name] ?? ''} readOnly={field.read_only} onChange={event => setValues(current => ({ ...current, [field.name]: event.target.value }))} className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2.5 text-sm read-only:bg-slate-100" />
                    )}
                  </li>
                ))}
              </ol>
            )}
            <button type="button" onClick={() => void saveFields()} disabled={!inventory.field_count || inventory.has_xfa || Boolean(busy)} className="mt-5 flex w-full items-center justify-center gap-2 rounded-2xl bg-indigo-600 px-4 py-3.5 text-sm font-black text-white disabled:opacity-35"><Check className="h-4 w-4" /> Save standard form values</button>
          </section>

          <section className="rounded-3xl bg-slate-950 p-4 text-white shadow-xl sm:p-6" aria-labelledby="visual-signature-heading">
            <div className="flex items-start justify-between gap-3">
              <div><p className="font-mono text-[10px] font-bold uppercase tracking-[.2em] text-cyan-300">Visual signature studio</p><h3 id="visual-signature-heading" className="mt-1 font-display text-2xl font-bold">Image mark, not digital trust</h3></div>
              <FileCheck2 className="h-8 w-8 text-cyan-300" />
            </div>
            <p className="mt-2 text-xs leading-5 text-slate-300">Typed, drawn, and imported assets stay in local browser storage until you delete them. Applying one sends it only to the token-protected loopback runtime, which removes the temporary image after placement.</p>

            <div className="mt-5 grid grid-cols-3 gap-2" role="tablist" aria-label="Visual signature source">
              {([['typed', Type], ['drawn', PenLine], ['imported', Upload]] as const).map(([mode, Icon]) => <button key={mode} type="button" role="tab" aria-selected={signatureMode === mode} onClick={() => setSignatureMode(mode)} className={`flex items-center justify-center gap-2 rounded-xl border px-2 py-2.5 text-[11px] font-bold capitalize ${signatureMode === mode ? 'border-cyan-300 bg-cyan-300 text-slate-950' : 'border-white/15 text-slate-300'}`}><Icon className="h-4 w-4" /> {mode}</button>)}
            </div>

            {signatureMode === 'typed' && <div className="mt-3 flex gap-2"><input value={typedName} onChange={event => setTypedName(event.target.value)} placeholder="Type your visual signature" maxLength={80} className="min-w-0 flex-1 rounded-xl border border-white/15 bg-white/10 px-3 py-2.5 text-sm text-white placeholder:text-slate-500" /><button type="button" onClick={saveTyped} disabled={!typedName.trim()} className="rounded-xl bg-cyan-300 px-4 text-xs font-black text-slate-950 disabled:opacity-35">Save locally</button></div>}
            {signatureMode === 'drawn' && <div className="mt-3"><canvas ref={drawCanvasRef} width="600" height="180" aria-label="Draw visual signature" onPointerDown={startDrawing} onPointerMove={continueDrawing} onPointerUp={stopDrawing} onPointerCancel={stopDrawing} className="aspect-[10/3] w-full touch-none rounded-xl bg-white" /><div className="mt-2 flex gap-2"><button type="button" onClick={clearDrawing} className="flex items-center gap-2 rounded-xl border border-white/15 px-3 py-2 text-xs"><Eraser className="h-4 w-4" /> Clear</button><button type="button" onClick={saveDrawing} disabled={!hasInk} className="ml-auto rounded-xl bg-cyan-300 px-4 py-2 text-xs font-black text-slate-950 disabled:opacity-35">Save drawing locally</button></div></div>}
            {signatureMode === 'imported' && <label className="mt-3 flex cursor-pointer items-center justify-center gap-2 rounded-xl border border-dashed border-white/25 bg-white/5 p-5 text-xs font-bold text-slate-200"><Upload className="h-4 w-4" /> Import PNG, JPEG, or WebP · max 750 KB<input type="file" accept="image/png,image/jpeg,image/webp" className="sr-only" aria-label="Import visual signature" onChange={event => importSignature(event.target.files?.[0] ?? null)} /></label>}

            <div className="mt-5">
              <div className="flex items-center justify-between"><h4 className="text-xs font-black uppercase tracking-wider text-slate-300">Saved locally · {assets.length}/8</h4>{assets.length > 0 && <span className="font-mono text-[9px] text-slate-500">Explicit delete below</span>}</div>
              <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-3">
                {assets.map(asset => <div key={asset.id} className={`relative rounded-xl border p-2 ${selectedAssetId === asset.id ? 'border-cyan-300 bg-cyan-300/10' : 'border-white/10 bg-white/5'}`}><button type="button" onClick={() => setSelectedAssetId(asset.id)} aria-label={`Select ${asset.kind} visual signature`} className="h-16 w-full rounded-lg bg-white p-2"><img src={asset.dataUrl} alt={`${asset.kind} visual signature preview`} className="h-full w-full object-contain" /></button><div className="mt-1 flex items-center justify-between"><span className="font-mono text-[8px] uppercase text-slate-400">{asset.kind}</span><button type="button" aria-label={`Delete ${asset.kind} visual signature`} onClick={() => { const next = deleteSignatureAsset(asset.id); setAssets(next); if (selectedAssetId === asset.id) setSelectedAssetId(''); }} className="rounded p-1 text-rose-300 hover:bg-rose-300/10"><Trash2 className="h-3.5 w-3.5" /></button></div></div>)}
                {assets.length === 0 && <p className="col-span-full rounded-xl border border-dashed border-white/15 p-4 text-center text-xs text-slate-500">No saved visual signature assets.</p>}
              </div>
            </div>

            <div className="mt-5 grid grid-cols-2 gap-2 sm:grid-cols-5">
              {([['page', 'Page'], ['x', 'X pt'], ['y', 'Y pt'], ['width', 'Width'], ['height', 'Height']] as const).map(([key, label]) => <label key={key} className="text-[9px] font-bold uppercase tracking-wider text-slate-400">{label}<input type="number" min={key === 'page' ? 1 : key === 'width' || key === 'height' ? 1 : 0} max={key === 'page' ? pageCount : undefined} value={placement[key]} onChange={event => setPlacement(current => ({ ...current, [key]: Number(event.target.value) }))} className="mt-1 w-full rounded-lg border border-white/15 bg-white/10 px-2 py-2 text-xs text-white" /></label>)}
            </div>
            <button type="button" onClick={() => void applyVisualSignature()} disabled={!selectedAsset || Boolean(busy)} className="mt-4 flex w-full items-center justify-center gap-2 rounded-2xl bg-cyan-300 px-4 py-3.5 text-sm font-black text-slate-950 disabled:opacity-35"><FileSignature className="h-4 w-4" /> Place visual signature</button>
          </section>
        </div>

        <section className="mt-5 overflow-hidden rounded-3xl border border-emerald-300 bg-emerald-950 text-white shadow-xl" aria-labelledby="certificate-lab-heading">
          <div className="grid gap-4 border-b border-emerald-800 bg-[radial-gradient(circle_at_top_right,rgba(52,211,153,.18),transparent_42%)] p-5 sm:p-7 lg:grid-cols-[1fr_auto] lg:items-end">
            <div>
              <p className="font-mono text-[10px] font-bold uppercase tracking-[.22em] text-emerald-300">Certificate lab · separate trust boundary</p>
              <h3 id="certificate-lab-heading" className="mt-2 font-display text-3xl font-bold">Digital signing & offline validation</h3>
              <p className="mt-2 max-w-3xl text-sm leading-6 text-emerald-100/75">This workflow uses a P12/PFX private key for one request, creates a separate incrementally signed copy, then discards the upload and passphrase. It never treats visual signature images as certificates.</p>
            </div>
            <span className="inline-flex items-center gap-2 rounded-full border border-emerald-400/30 bg-emerald-400/10 px-4 py-2 font-mono text-[10px] font-bold uppercase tracking-wider text-emerald-200"><KeyRound className="h-4 w-4" /> No key library</span>
          </div>

          <div className="grid gap-px bg-emerald-800 lg:grid-cols-2">
            <div className="bg-emerald-950 p-5 sm:p-7">
              <div className="flex items-start gap-3"><Fingerprint className="mt-1 h-7 w-7 text-emerald-300" /><div><h4 className="font-display text-2xl font-bold">Validate this PDF</h4><p className="mt-1 text-xs leading-5 text-emerald-100/65">Integrity is checked without network access. Trust stays false unless you explicitly provide a PEM/DER trust root.</p></div></div>
              <label className="mt-5 block rounded-2xl border border-dashed border-emerald-500/35 bg-white/5 p-4 text-xs font-bold text-emerald-100">Optional explicit trust root · PEM, CRT, CER, or DER
                <input type="file" accept=".pem,.crt,.cer,.der,application/x-pem-file,application/pkix-cert" aria-label="Import explicit trust root" onChange={event => setTrustRootFile(event.target.files?.[0] ?? null)} className="mt-2 block w-full text-[11px] text-emerald-200 file:mr-3 file:rounded-lg file:border-0 file:bg-emerald-300 file:px-3 file:py-2 file:font-bold file:text-emerald-950" />
              </label>
              <button type="button" onClick={() => void validateDigitalSignatures()} disabled={Boolean(busy)} className="mt-4 flex w-full items-center justify-center gap-2 rounded-2xl bg-emerald-300 px-4 py-3.5 text-sm font-black text-emerald-950 disabled:opacity-35">{busy === 'certificate-validate' ? <Loader2 className="h-4 w-4 animate-spin" /> : <ShieldCheck className="h-4 w-4" />} Validate offline</button>

              {digitalValidation && <div className="mt-5 rounded-2xl border border-white/10 bg-black/15 p-4" aria-label="Digital signature validation result">
                {digitalValidation.status === 'unsigned' ? <p className="text-sm font-bold text-amber-200">Unsigned · no embedded digital signature found</p> : <>
                  <div className="grid grid-cols-2 gap-2 text-[11px] sm:grid-cols-4">
                    <div className="rounded-xl bg-white/5 p-3"><span className="block text-emerald-100/55">Cryptography</span><strong className={digitalValidation.all_cryptographically_valid ? 'text-emerald-300' : 'text-rose-300'}>{digitalValidation.all_cryptographically_valid ? 'Valid' : 'Invalid'}</strong></div>
                    <div className="rounded-xl bg-white/5 p-3"><span className="block text-emerald-100/55">Later changes</span><strong className={digitalValidation.all_documents_unchanged_since_signature ? 'text-emerald-300' : 'text-amber-200'}>{digitalValidation.all_documents_unchanged_since_signature ? 'None' : 'Detected'}</strong></div>
                    <div className="rounded-xl bg-white/5 p-3"><span className="block text-emerald-100/55">Explicit trust</span><strong className={digitalValidation.all_trusted ? 'text-emerald-300' : 'text-amber-200'}>{digitalValidation.all_trusted ? 'Trusted' : 'Not established'}</strong></div>
                    <div className="rounded-xl bg-white/5 p-3"><span className="block text-emerald-100/55">Revocation</span><strong className="text-amber-200">Not checked</strong></div>
                  </div>
                  <ul className="mt-3 space-y-2">{digitalValidation.signatures.map(signature => <li key={`${signature.index}-${signature.field_name}`} className="rounded-xl border border-white/10 p-3 text-xs"><strong>{signature.certificate?.subject_common_name ?? signature.field_name ?? `Signature ${signature.index + 1}`}</strong><span className="mt-1 block break-all font-mono text-[9px] text-emerald-100/50">SHA-256 {signature.certificate?.sha256_fingerprint ?? 'unavailable'}</span></li>)}</ul>
                </>}
              </div>}
            </div>

            <div className="bg-emerald-950 p-5 sm:p-7">
              <div className="flex items-start gap-3"><KeyRound className="mt-1 h-7 w-7 text-emerald-300" /><div><h4 className="font-display text-2xl font-bold">Create a signed copy</h4><p className="mt-1 text-xs leading-5 text-emerald-100/65">No timestamp authority or online revocation service is contacted. Any later edit can change the validation result.</p></div></div>
              <label className="mt-5 block text-[10px] font-bold uppercase tracking-wider text-emerald-100/70">P12 or PFX signing identity
                <input ref={certificateInputRef} type="file" accept=".p12,.pfx,application/x-pkcs12" aria-label="Import P12 or PFX certificate" onChange={event => setCertificateFile(event.target.files?.[0] ?? null)} className="mt-1 block w-full rounded-xl border border-white/15 bg-white/5 px-3 py-2 text-xs normal-case tracking-normal text-emerald-100" />
              </label>
              <div className="mt-3 grid gap-3 sm:grid-cols-2">
                <label className="text-[10px] font-bold uppercase tracking-wider text-emerald-100/70">Passphrase · never stored<input type="password" autoComplete="off" value={certificatePassphrase} onChange={event => setCertificatePassphrase(event.target.value)} className="mt-1 w-full rounded-xl border border-white/15 bg-white/5 px-3 py-2.5 text-sm normal-case tracking-normal text-white" /></label>
                <label className="text-[10px] font-bold uppercase tracking-wider text-emerald-100/70">PDF field name<input value={digitalDetails.fieldName} maxLength={80} onChange={event => setDigitalDetails(current => ({ ...current, fieldName: event.target.value }))} className="mt-1 w-full rounded-xl border border-white/15 bg-white/5 px-3 py-2.5 text-sm normal-case tracking-normal text-white" /></label>
                <label className="text-[10px] font-bold uppercase tracking-wider text-emerald-100/70">Reason · optional<input value={digitalDetails.reason} maxLength={200} onChange={event => setDigitalDetails(current => ({ ...current, reason: event.target.value }))} className="mt-1 w-full rounded-xl border border-white/15 bg-white/5 px-3 py-2.5 text-sm normal-case tracking-normal text-white" /></label>
                <label className="text-[10px] font-bold uppercase tracking-wider text-emerald-100/70">Location · optional<input value={digitalDetails.location} maxLength={120} onChange={event => setDigitalDetails(current => ({ ...current, location: event.target.value }))} className="mt-1 w-full rounded-xl border border-white/15 bg-white/5 px-3 py-2.5 text-sm normal-case tracking-normal text-white" /></label>
              </div>
              <div className="mt-3 grid grid-cols-5 gap-2">{([['page', 'Page'], ['x0', 'X₀'], ['y0', 'Y₀'], ['x1', 'X₁'], ['y1', 'Y₁']] as const).map(([key, label]) => <label key={key} className="text-[9px] font-bold uppercase text-emerald-100/60">{label}<input type="number" min={key === 'page' ? 1 : 0} max={key === 'page' ? pageCount : undefined} value={digitalDetails[key]} onChange={event => setDigitalDetails(current => ({ ...current, [key]: Number(event.target.value) }))} className="mt-1 w-full rounded-lg border border-white/15 bg-white/5 px-2 py-2 text-xs text-white" /></label>)}</div>
              <button type="button" onClick={() => void createCertificateSignedCopy()} disabled={!certificateFile || !digitalDetails.fieldName.trim() || Boolean(busy)} className="mt-4 flex w-full items-center justify-center gap-2 rounded-2xl border border-emerald-300 bg-transparent px-4 py-3.5 text-sm font-black text-emerald-200 disabled:opacity-35">{busy === 'certificate-sign' ? <Loader2 className="h-4 w-4 animate-spin" /> : <FileSignature className="h-4 w-4" />} Create separate signed copy</button>
              <p className="mt-3 flex gap-2 text-[10px] leading-4 text-amber-200/80"><ShieldAlert className="h-4 w-4 shrink-0" /> Trust is contextual, not a legal conclusion. This offline workflow does not check certificate revocation or obtain a trusted timestamp.</p>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
