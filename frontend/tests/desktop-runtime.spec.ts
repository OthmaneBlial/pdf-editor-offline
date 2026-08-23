import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

describe('desktop runtime helpers', () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    delete window.__TAURI_INTERNALS__;
    delete window.__PDF_EDITOR_OFFLINE_API_BASE_URL__;
    delete window.__PDF_EDITOR_OFFLINE_API_TOKEN__;
  });

  it('initializes the API base URL from Tauri before the app mounts', async () => {
    window.__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValueOnce({
      baseUrl: 'http://127.0.0.1:49152',
      token: 'desktop-token',
    });

    const { initializeDesktopRuntime } = await import('../src/lib/desktop');
    await initializeDesktopRuntime();

    expect(invokeMock).toHaveBeenCalledWith('get_api_connection');
    expect(window.__PDF_EDITOR_OFFLINE_API_BASE_URL__).toBe('http://127.0.0.1:49152');
    expect(window.__PDF_EDITOR_OFFLINE_API_TOKEN__).toBe('desktop-token');
  });

  it('falls back to browser download behavior outside Tauri', async () => {
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click');
    const { saveBlob } = await import('../src/lib/downloads');

    await saveBlob(new Blob(['pdf']), 'document.pdf');

    expect(clickSpy).toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
