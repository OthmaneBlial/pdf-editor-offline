import { describe, expect, it } from 'vitest';
import { getApiData, isApiSuccess } from '../src/utils/apiResponse';

describe('apiResponse helpers', () => {
  it('detects successful API envelopes', () => {
    expect(isApiSuccess({ success: true, data: {} })).toBe(true);
    expect(isApiSuccess({ success: false, data: {} })).toBe(false);
    expect(isApiSuccess({ data: {} })).toBe(false);
    expect(isApiSuccess(null)).toBe(false);
  });

  it('extracts typed data only from valid envelopes', () => {
    const valid = { success: true, data: { image: 'data:image/png;base64,AAA' } };
    const invalidNoData = { success: true };
    const invalidWrongShape = { success: true, data: 'oops' };

    expect(getApiData<{ image: string }>(valid)).toEqual({ image: 'data:image/png;base64,AAA' });
    expect(getApiData(invalidNoData)).toBeNull();
    expect(getApiData(invalidWrongShape)).toBeNull();
    expect(getApiData({ success: false, data: {} })).toBeNull();
  });
});
