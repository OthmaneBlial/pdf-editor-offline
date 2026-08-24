export type SignatureKind = 'typed' | 'drawn' | 'imported';

export interface SignatureAsset {
  id: string;
  kind: SignatureKind;
  dataUrl: string;
  createdAt: string;
}

const STORAGE_KEY = 'pdf-editor-visual-signatures';
const MAX_ASSETS = 8;

const isSignatureAsset = (value: unknown): value is SignatureAsset => {
  if (!value || typeof value !== 'object') return false;
  const asset = value as Partial<SignatureAsset>;
  return typeof asset.id === 'string'
    && ['typed', 'drawn', 'imported'].includes(asset.kind ?? '')
    && typeof asset.dataUrl === 'string'
    && asset.dataUrl.startsWith('data:image/')
    && typeof asset.createdAt === 'string';
};

export const loadSignatureAssets = (): SignatureAsset[] => {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter(isSignatureAsset).slice(0, MAX_ASSETS) : [];
  } catch {
    return [];
  }
};

export const saveSignatureAsset = (kind: SignatureKind, dataUrl: string): SignatureAsset[] => {
  if (!dataUrl.startsWith('data:image/')) throw new Error('Signature asset must be an image');
  const asset: SignatureAsset = {
    id: crypto.randomUUID(),
    kind,
    dataUrl,
    createdAt: new Date().toISOString(),
  };
  const next = [asset, ...loadSignatureAssets()].slice(0, MAX_ASSETS);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  return next;
};

export const deleteSignatureAsset = (id: string): SignatureAsset[] => {
  const next = loadSignatureAssets().filter(asset => asset.id !== id);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  return next;
};

export const clearSignatureAssets = () => {
  localStorage.removeItem(STORAGE_KEY);
};

export const signatureAssetToFile = async (asset: SignatureAsset): Promise<File> => {
  const response = await fetch(asset.dataUrl);
  const blob = await response.blob();
  const extension = blob.type === 'image/jpeg' ? 'jpg' : blob.type === 'image/webp' ? 'webp' : 'png';
  return new File([blob], `visual-signature.${extension}`, { type: blob.type || 'image/png' });
};
