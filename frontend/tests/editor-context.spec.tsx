import React from 'react';
import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, vi } from 'vitest';

import axios from 'axios';
import { EditorProvider, useEditor } from '../src/contexts/EditorContext';

vi.mock('axios');
const mockedAxios = axios as {
  post: ReturnType<typeof vi.fn>;
  get: ReturnType<typeof vi.fn>;
};

let exposedContext: ReturnType<typeof useEditor> | null = null;

const Tracker = () => {
  const ctx = useEditor();
  React.useEffect(() => {
    exposedContext = ctx;
  }, [ctx]);
  return null;
};

const ensureContext = () => {
  if (!exposedContext) {
    throw new Error('Context not mounted yet');
  }
  return exposedContext;
};

describe('EditorContext', () => {
  beforeEach(() => {
    exposedContext = null;
    mockedAxios.post.mockReset();
    mockedAxios.get.mockReset();
    vi.stubGlobal('alert', vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('posts canvas data when saveChanges is forced', async () => {
    render(
      <EditorProvider>
        <Tracker />
      </EditorProvider>
    );

    const canvasStub = {
      getObjects: vi.fn().mockReturnValue([{ toObject: () => ({ stroke: '#000' }) }]),
      getZoom: vi.fn().mockReturnValue(1),
      toDataURL: vi.fn().mockReturnValue('data:image/png;base64,AAA'),
      requestRenderAll: vi.fn(),
      backgroundImage: null,
      toJSON: vi.fn(() => ({ objects: [] })),
      on: vi.fn(),
      off: vi.fn(),
    };

    act(() => {
      const ctx = ensureContext();
      ctx.setSessionId('session-1');
      ctx.setCurrentPage(0);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ctx.setCanvas(canvasStub as any);
    });

    await act(async () => {
      const ctx = ensureContext();
      await ctx.saveChanges(true);
    });

    expect(mockedAxios.post).toHaveBeenCalled();
    const [[, payload]] = mockedAxios.post.mock.calls;
    expect(payload.overlay_image).toContain('data:image/png;base64,');
  });

  it('exports PDF after forcing save', async () => {
    render(
      <EditorProvider>
        <Tracker />
      </EditorProvider>
    );

    const canvasStub = {
      getObjects: vi.fn().mockReturnValue([{ toObject: () => ({ stroke: '#000' }) }]),
      getZoom: vi.fn().mockReturnValue(1),
      toDataURL: vi.fn().mockReturnValue('data:image/png;base64,ABC'),
      requestRenderAll: vi.fn(),
      backgroundImage: null,
      toJSON: vi.fn(() => ({ objects: [] })),
      on: vi.fn(),
      off: vi.fn(),
    };

    mockedAxios.get.mockResolvedValue({
      data: new Blob(['x']),
      headers: { 'content-disposition': 'filename=test.pdf' }
    });

    act(() => {
      const ctx = ensureContext();
      ctx.setSessionId('session-export');
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ctx.setCanvas(canvasStub as any);
    });

    const createObjectURL = vi.fn().mockReturnValue('blob:url');
    const originalURL = window.URL.createObjectURL;
    window.URL.createObjectURL = createObjectURL;

    const revokeObjectURL = vi.fn();
    window.URL.revokeObjectURL = revokeObjectURL;

    const clickFn = vi.fn();
    const createElementSpy = vi.spyOn(document, 'createElement').mockImplementation(() => ({
      tagName: 'A',
      href: '',
      setAttribute: vi.fn(),
      click: clickFn,
      remove: vi.fn(),
    }));
    vi.spyOn(document.body, 'appendChild').mockImplementation(() => ({}));

    vi.stubGlobal('alert', vi.fn());

    await act(async () => {
      const ctx = ensureContext();
      await ctx.exportPDF();
    });

    expect(mockedAxios.post).toHaveBeenCalled();
    expect(mockedAxios.get).toHaveBeenCalledWith(
      expect.stringContaining('/api/documents/session-export/download'),
      expect.objectContaining({ responseType: 'blob' })
    );
    expect(createObjectURL).toHaveBeenCalled();

    // Cleanup
    window.URL.createObjectURL = originalURL;
    window.URL.revokeObjectURL = revokeObjectURL;
    createElementSpy.mockRestore();
  });
});
