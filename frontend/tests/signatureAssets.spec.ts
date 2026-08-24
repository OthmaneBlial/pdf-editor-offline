import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  clearSignatureAssets,
  deleteSignatureAsset,
  loadSignatureAssets,
  saveSignatureAsset,
} from '../src/services/signatureAssets';

describe('local visual signature assets', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.spyOn(crypto, 'randomUUID').mockReturnValue('00000000-0000-4000-8000-000000000001');
  });

  it('stores only local image data with explicit delete and clear controls', () => {
    const saved = saveSignatureAsset('typed', 'data:image/png;base64,c2ln');
    expect(saved).toHaveLength(1);
    expect(loadSignatureAssets()[0]).toMatchObject({ id: '00000000-0000-4000-8000-000000000001', kind: 'typed' });

    expect(deleteSignatureAsset(saved[0].id)).toEqual([]);
    saveSignatureAsset('drawn', 'data:image/png;base64,aW5r');
    clearSignatureAssets();
    expect(loadSignatureAssets()).toEqual([]);
  });

  it('rejects non-image payloads', () => {
    expect(() => saveSignatureAsset('imported', 'data:text/plain;base64,bm8=')).toThrow('must be an image');
  });
});
