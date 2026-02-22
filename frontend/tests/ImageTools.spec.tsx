import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import '@testing-library/jest-dom';

import ImageTools from '../src/components/tools/ImageTools';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
};

describe('ImageTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorMock.mockReturnValue({
      sessionId: 'doc-1',
      currentPage: 0,
      saveChanges: vi.fn(),
      exportPDF: vi.fn(),
      hasUnsavedChanges: false,
      pageCount: 3,
      reportToolResult: vi.fn(),
    });

    mockedAxios.get.mockResolvedValue({
      data: {
        success: true,
        data: {
          images: [
            {
              index: 0,
              width: 100,
              height: 80,
              color_space: 'DeviceRGB',
              bits_per_component: 8,
              compression: 'DCT',
              bbox: [100, 100, 200, 180],
            },
          ],
        },
      },
    });
    mockedAxios.post.mockResolvedValue({ data: { success: true } });
  });

  it('uploads replacement image through multipart endpoint', async () => {
    render(<ImageTools />);
    await waitFor(() => {
      expect(mockedAxios.get).toHaveBeenCalled();
    });

    const replaceButton = screen.getByRole('button', { name: /Replace Image/i });
    const replaceForm = replaceButton.closest('form') as HTMLFormElement;
    const selectInput = replaceForm.querySelector('select') as HTMLSelectElement;
    const replaceFileInput = replaceForm.querySelector('input[type="file"]') as HTMLInputElement;

    fireEvent.change(selectInput, { target: { value: '0' } });
    fireEvent.change(replaceFileInput, {
      target: { files: [new File(['img'], 'replace.png', { type: 'image/png' })] },
    });
    fireEvent.submit(replaceForm);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/images/replace/upload'),
        expect.any(FormData)
      );
    });
  });

  it('uploads inserted image through multipart endpoint', async () => {
    render(<ImageTools />);
    await waitFor(() => {
      expect(mockedAxios.get).toHaveBeenCalled();
    });

    const insertButton = screen.getByRole('button', { name: /Insert Image/i });
    const insertForm = insertButton.closest('form') as HTMLFormElement;
    const insertFileInput = insertForm.querySelector('input[type="file"]') as HTMLInputElement;

    fireEvent.change(insertFileInput, {
      target: { files: [new File(['img'], 'insert.png', { type: 'image/png' })] },
    });
    fireEvent.submit(insertForm);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/images/insert/upload'),
        expect.any(FormData)
      );
    });
  });
});
