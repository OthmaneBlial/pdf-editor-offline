import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';

import PDFViewer from '../src/components/PDFViewer';

const { useEditorMock, fabricState } = vi.hoisted(() => ({
  useEditorMock: vi.fn(),
  fabricState: {
    fromURLMock: vi.fn(),
    setDimensionsMock: vi.fn(),
    setZoomMock: vi.fn(),
    requestRenderAllMock: vi.fn(),
    onMock: vi.fn(),
    offMock: vi.fn(),
    disposeMock: vi.fn(),
    lastCanvas: null as unknown,
  },
}));

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

vi.mock('axios');

vi.mock('fabric', () => {
  class Canvas {
    lower = { el: document.createElement('canvas') };
    disposed = false;
    destroyed = false;
    backgroundImage: unknown = null;
    isDrawingMode = false;
    selection = true;
    freeDrawingBrush: unknown = null;

    constructor() {
      fabricState.lastCanvas = this;
    }

    setDimensions = fabricState.setDimensionsMock;
    setZoom = fabricState.setZoomMock;
    requestRenderAll = fabricState.requestRenderAllMock;
    on = fabricState.onMock;
    off = fabricState.offMock;
    getActiveObject = vi.fn(() => null);
    getPointer = vi.fn(() => ({ x: 0, y: 0 }));
    add = vi.fn();
    setActiveObject = vi.fn();
    dispose = vi.fn(() => {
      this.disposed = true;
      this.destroyed = true;
      this.lower = undefined as unknown as { el: HTMLCanvasElement };
      fabricState.disposeMock();
    });
  }

  class PencilBrush {
    color = '';
    width = 1;
    constructor() {}
  }

  class IText {
    type = 'i-text';
    constructor() {}
    set = vi.fn();
    enterEditing = vi.fn();
    selectAll = vi.fn();
  }

  class Rect {
    type = 'rect';
    constructor() {}
    set = vi.fn();
  }

  class Circle {
    type = 'circle';
    constructor() {}
    set = vi.fn();
  }

  const FabricImage = {
    fromURL: fabricState.fromURLMock,
  };

  return {
    Canvas,
    PencilBrush,
    IText,
    Rect,
    Circle,
    FabricImage,
    Image: FabricImage,
  };
});

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
};

describe('PDFViewer lifecycle safety', () => {
  const originalResizeObserver = global.ResizeObserver;
  const originalRequestAnimationFrame = window.requestAnimationFrame;

  let resizeCallback: ResizeObserverCallback | null = null;
  let queuedRaf: FrameRequestCallback | null = null;

  beforeEach(() => {
    vi.clearAllMocks();
    resizeCallback = null;
    queuedRaf = null;

    class MockResizeObserver {
      constructor(callback: ResizeObserverCallback) {
        resizeCallback = callback;
      }
      observe() {}
      disconnect() {}
      unobserve() {}
    }

    vi.stubGlobal('ResizeObserver', MockResizeObserver as unknown as typeof ResizeObserver);
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn((callback: FrameRequestCallback) => {
        queuedRaf = callback;
        return 1;
      })
    );

    mockedAxios.get.mockResolvedValue({
      data: {
        success: true,
        data: {
          image: 'data:image/png;base64,AAA',
        },
      },
    });
    mockedAxios.post.mockResolvedValue({
      data: {
        success: true,
        data: {
          id: 'session-2',
          page_count: 1,
        },
      },
    });

    fabricState.fromURLMock.mockResolvedValue({
      width: 1000,
      height: 1400,
      set: vi.fn(),
    });

    useEditorMock.mockReturnValue({
      document: null,
      currentPage: 0,
      canvas: null,
      setCanvas: vi.fn(),
      drawingMode: 'select',
      setDrawingMode: vi.fn(),
      color: '#000000',
      strokeWidth: 2,
      fontSize: 14,
      fontFamily: 'Arial',
      sessionId: 'session-1',
      setSessionId: vi.fn(),
      documentUploadVersion: 0,
      uploadedDocumentVersion: 0,
      setUploadedDocumentVersion: vi.fn(),
      setPageCount: vi.fn(),
      zoom: 1,
      setCurrentPage: vi.fn(),
      setIsUploading: vi.fn(),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    global.ResizeObserver = originalResizeObserver;
    window.requestAnimationFrame = originalRequestAnimationFrame;
  });

  it('ignores queued resize callbacks after canvas disposal', async () => {
    const { unmount } = render(<PDFViewer />);

    await waitFor(() => {
      expect(fabricState.fromURLMock).toHaveBeenCalledTimes(1);
      expect(resizeCallback).not.toBeNull();
    });

    const initialResizeCalls = fabricState.setDimensionsMock.mock.calls.length;

    act(() => {
      resizeCallback?.([], {} as ResizeObserver);
    });

    expect(queuedRaf).not.toBeNull();

    unmount();

    expect(() => {
      queuedRaf?.(performance.now());
    }).not.toThrow();

    expect(fabricState.setDimensionsMock).toHaveBeenCalledTimes(initialResizeCalls);
  });

  it('shows backend connectivity guidance and retries health check', async () => {
    mockedAxios.get.mockReset();
    mockedAxios.get
      .mockRejectedValueOnce(new Error('Network error'))
      .mockResolvedValueOnce({
        data: { openapi: '3.1.0' },
      });

    useEditorMock.mockReturnValue({
      document: null,
      currentPage: 0,
      canvas: null,
      setCanvas: vi.fn(),
      drawingMode: 'select',
      setDrawingMode: vi.fn(),
      color: '#000000',
      strokeWidth: 2,
      fontSize: 14,
      fontFamily: 'Arial',
      sessionId: '',
      setSessionId: vi.fn(),
      documentUploadVersion: 0,
      uploadedDocumentVersion: 0,
      setUploadedDocumentVersion: vi.fn(),
      setPageCount: vi.fn(),
      zoom: 1,
      setCurrentPage: vi.fn(),
      setIsUploading: vi.fn(),
    });

    render(<PDFViewer />);

    expect(await screen.findByText('Backend API is not reachable.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Retry connection/i }));

    await waitFor(() => {
      expect(screen.queryByText('Backend API is not reachable.')).not.toBeInTheDocument();
    });
  });

  it('swallows known removeChild errors during canvas disposal', async () => {
    fabricState.disposeMock.mockImplementation(() => {
      throw new Error("Failed to execute 'removeChild' on 'Node': The node to be removed is not a child of this node.");
    });

    const { unmount } = render(<PDFViewer />);

    await waitFor(() => {
      expect(fabricState.fromURLMock).toHaveBeenCalledTimes(1);
    });

    expect(() => {
      unmount();
    }).not.toThrow();
  });

  it('does not re-upload an already uploaded document after viewer remounts', async () => {
    const file = new File(['%PDF-1.7'], 'demo.pdf', { type: 'application/pdf' });

    useEditorMock.mockReturnValue({
      document: file,
      currentPage: 0,
      canvas: null,
      setCanvas: vi.fn(),
      drawingMode: 'select',
      setDrawingMode: vi.fn(),
      color: '#000000',
      strokeWidth: 2,
      fontSize: 14,
      fontFamily: 'Arial',
      sessionId: 'session-1',
      setSessionId: vi.fn(),
      documentUploadVersion: 3,
      uploadedDocumentVersion: 3,
      setUploadedDocumentVersion: vi.fn(),
      setPageCount: vi.fn(),
      zoom: 1,
      setCurrentPage: vi.fn(),
      setIsUploading: vi.fn(),
    });

    render(<PDFViewer />);

    await waitFor(() => {
      expect(fabricState.fromURLMock).toHaveBeenCalledTimes(1);
    });

    expect(mockedAxios.post).not.toHaveBeenCalled();
  });

  it('uploads when the selected document version is not bound to the active session', async () => {
    const file = new File(['%PDF-1.7'], 'demo.pdf', { type: 'application/pdf' });
    const setSessionId = vi.fn();
    const setUploadedDocumentVersion = vi.fn();
    const setPageCount = vi.fn();

    useEditorMock.mockReturnValue({
      document: file,
      currentPage: 0,
      canvas: null,
      setCanvas: vi.fn(),
      drawingMode: 'select',
      setDrawingMode: vi.fn(),
      color: '#000000',
      strokeWidth: 2,
      fontSize: 14,
      fontFamily: 'Arial',
      sessionId: 'session-1',
      setSessionId,
      documentUploadVersion: 4,
      uploadedDocumentVersion: 3,
      setUploadedDocumentVersion,
      setPageCount,
      zoom: 1,
      setCurrentPage: vi.fn(),
      setIsUploading: vi.fn(),
    });

    render(<PDFViewer />);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/upload'),
        expect.any(FormData),
        { headers: { 'Content-Type': 'multipart/form-data' } }
      );
    });

    expect(setSessionId).toHaveBeenCalledWith('session-2');
    expect(setPageCount).toHaveBeenCalledWith(1);
    expect(setUploadedDocumentVersion).toHaveBeenCalledWith(4);
  });
});
