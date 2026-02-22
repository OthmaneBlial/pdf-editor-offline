import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import '@testing-library/jest-dom';

import NavigationTools from '../src/components/tools/NavigationTools';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
  delete: ReturnType<typeof vi.fn>;
};

describe('NavigationTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorMock.mockReturnValue({
      sessionId: 'doc-1',
      currentPage: 0,
      setCurrentPage: vi.fn(),
      saveChanges: vi.fn(),
      exportPDF: vi.fn(),
      hasUnsavedChanges: false,
      pageCount: 5,
    });

    mockedAxios.get.mockImplementation((url: string) => {
      if (url.includes('/links/')) {
        return Promise.resolve({ data: { success: true, data: { links: [] } } });
      }
      return Promise.resolve({ data: { success: true, data: { toc: [] } } });
    });
    mockedAxios.post.mockResolvedValue({ data: { success: true, data: {} } });
    mockedAxios.delete.mockResolvedValue({ data: { success: true, data: {} } });
  });

  it('loads TOC data on mount', async () => {
    render(<NavigationTools />);

    await waitFor(() => {
      expect(mockedAxios.get).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/toc')
      );
    });
  });

  it('triggers auto TOC generation', async () => {
    render(<NavigationTools />);

    const thresholdsInput = screen.getByPlaceholderText('18,14,12');
    fireEvent.change(thresholdsInput, { target: { value: '20,16,12' } });

    fireEvent.click(screen.getByRole('button', { name: /Auto-Generate from Headers/i }));

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/toc/auto'),
        null,
        expect.objectContaining({
          params: { font_size_thresholds: '20,16,12' },
        })
      );
    });
  });

  it('adds an external link from the new link form', async () => {
    render(<NavigationTools />);

    fireEvent.change(screen.getByPlaceholderText('https://example.com'), {
      target: { value: 'https://openai.com' },
    });

    fireEvent.click(screen.getByRole('button', { name: /Add New Link/i }));

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/links'),
        expect.objectContaining({
          page_num: 0,
          url: 'https://openai.com',
        })
      );
    });
  });
});
