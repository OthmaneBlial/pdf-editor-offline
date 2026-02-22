import React from 'react';
import { act, render, waitFor } from '@testing-library/react';
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
    constructor(_canvas: unknown) {}
  }

  class IText {
    type = 'i-text';
    constructor(_text: string, _options: unknown) {}
    set = vi.fn();
    enterEditing = vi.fn();
    selectAll = vi.fn();
  }

  class Rect {
    type = 'rect';
    constructor(_options: unknown) {}
    set = vi.fn();
  }

  class Circle {
    type = 'circle';
    constructor(_options: unknown) {}
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
        data: {
          image: 'data:image/png;base64,AAA',
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
});
