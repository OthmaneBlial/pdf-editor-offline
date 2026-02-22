import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import '@testing-library/jest-dom';

import AdvancedTextTools from '../src/components/tools/AdvancedTextTools';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
};

describe('AdvancedTextTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorMock.mockReturnValue({
      sessionId: 'doc-1',
      currentPage: 0,
      saveChanges: vi.fn(),
      exportPDF: vi.fn(),
      hasUnsavedChanges: false,
      reportToolResult: vi.fn(),
    });

    mockedAxios.get.mockImplementation((url: string) => {
      if (url.includes('/text/search')) {
        return Promise.resolve({
          data: {
            success: true,
            data: {
              count: 1,
              matches: [
                {
                  index: 0,
                  rect: [10, 20, 30, 40],
                  quad_points: [[10, 20], [30, 20], [10, 40], [30, 40]],
                },
              ],
            },
          },
        });
      }
      return Promise.resolve({
        data: {
          success: true,
          data: { total_fonts: 0, fonts: [] },
        },
      });
    });
    mockedAxios.post.mockResolvedValue({ data: { success: true, data: {} } });
  });

  it('calls the text search endpoint and renders search results', async () => {
    render(<AdvancedTextTools />);

    fireEvent.change(screen.getByPlaceholderText('Text to search for...'), {
      target: { value: 'Page' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Search$/i }));

    await waitFor(() => {
      expect(mockedAxios.get).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/pages/0/text/search'),
        expect.objectContaining({
          params: { text: 'Page' },
        })
      );
    });

    expect(await screen.findByText(/Match #1/i)).toBeInTheDocument();
  });
});
