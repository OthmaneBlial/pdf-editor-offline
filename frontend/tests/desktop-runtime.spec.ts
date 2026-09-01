import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
const unlistenMock = vi.fn();
let dragDropHandler: ((event: { payload: { type: string; paths?: string[] } }) => Promise<void>) | undefined;
const onDragDropEventMock = vi.fn(async (handler) => {
  dragDropHandler = handler;
  return unlistenMock;
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: onDragDropEventMock,
  }),
}));

describe('desktop runtime helpers', () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    onDragDropEventMock.mockClear();
    unlistenMock.mockClear();
    dragDropHandler = undefined;
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

  it('opens PDFs dropped through the native Tauri webview on Windows', async () => {
    window.__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValueOnce({
      name: 'Dropped.PDF',
      size: 8,
      bytes: [37, 80, 68, 70, 45, 49, 46, 55],
    });
    const onFile = vi.fn();
    const onDragStateChange = vi.fn();

    const { listenForDesktopPdfDrops } = await import('../src/lib/desktop');
    const unlisten = await listenForDesktopPdfDrops(onFile, onDragStateChange);

    await dragDropHandler?.({ payload: { type: 'over' } });
    await dragDropHandler?.({ payload: { type: 'drop', paths: ['C:\\Users\\Tester\\Dropped.PDF'] } });

    expect(onDragStateChange).toHaveBeenNthCalledWith(1, true);
    expect(onDragStateChange).toHaveBeenLastCalledWith(false);
    expect(invokeMock).toHaveBeenCalledWith('open_pdf_path', {
      path: 'C:\\Users\\Tester\\Dropped.PDF',
    });
    expect(onFile).toHaveBeenCalledWith(expect.objectContaining({
      name: 'Dropped.PDF',
      type: 'application/pdf',
    }));
    expect(unlisten).toBe(unlistenMock);
  });
});
