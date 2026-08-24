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

    mockedAxios.get.mockResolvedValue({
      data: {
        success: true,
        data: { total_fonts: 0, fonts: [] },
      },
    });
    mockedAxios.post.mockImplementation((url: string) => {
      if (url.includes('/text/replace/preflight')) {
        return Promise.resolve({
          data: {
            success: true,
            data: {
              status: 'eligible',
              maturity: 'experimental',
              native_in_place_edit: false,
              implementation: 'redaction_plus_new_content_stream',
              rejection_reasons: [],
              thresholds: {
                maximum_matches: 1,
                maximum_replacement_width_ratio: 1,
                maximum_target_render_change_ratio: 0.08,
                maximum_unchanged_page_render_ratio: 0.0001,
              },
              geometry: { replacement_width_ratio: 0.8 },
            },
          },
        });
      }
      if (url.endsWith('/text/replace')) {
        return Promise.resolve({
          data: {
            success: true,
            data: { evidence: { status: 'passed' } },
          },
        });
      }
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
          success: true, data: {},
        },
      });
    });
  });

  it('calls the text search endpoint and renders search results', async () => {
    render(<AdvancedTextTools />);

    fireEvent.change(screen.getByPlaceholderText('Text to search for...'), {
      target: { value: 'Page' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Search$/i }));

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/pages/0/text/search'),
        { text: 'Page' },
      );
    });

    expect(await screen.findByText(/Match #1/i)).toBeInTheDocument();
  });

  it('preflights and labels replacement as experimental before applying it', async () => {
    render(<AdvancedTextTools />);
    fireEvent.change(screen.getByPlaceholderText('Text to find...'), {
      target: { value: 'Page' },
    });
    fireEvent.change(screen.getByPlaceholderText('New text...'), {
      target: { value: 'Part' },
    });

    fireEvent.click(screen.getByRole('button', { name: /Check experimental support/i }));
    expect(await screen.findByText('Bounded input is eligible')).toBeInTheDocument();
    const apply = screen.getByRole('button', { name: /Apply \+ run fidelity gates/i });
    expect(apply).toBeDisabled();

    fireEvent.click(screen.getByRole('checkbox', { name: /I understand this is experimental/i }));
    fireEvent.click(apply);

    await waitFor(() => expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringMatching(/\/text\/replace$/),
      { page_num: 0, search_text: 'Page', new_text: 'Part' },
    ));
    expect(await screen.findByText(/passed extraction, visual, semantic/i)).toBeInTheDocument();
  });

  it('can send rich text through the reflow endpoint', async () => {
    render(<AdvancedTextTools />);

    fireEvent.change(screen.getAllByRole('combobox')[0], {
      target: { value: 'reflow' },
    });
    fireEvent.change(screen.getByPlaceholderText('<p>HTML content here...</p>'), {
      target: { value: '<p>Wrapped text</p>' },
    });
    fireEvent.click(screen.getByRole('button', { name: /Insert Rich Text/i }));

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/pages/0/text/reflow'),
        expect.objectContaining({
          html_content: '<p>Wrapped text</p>',
        })
      );
    });
  });

  it('does not expose the legacy unverified area-redaction action', async () => {
    render(<AdvancedTextTools />);
    expect(screen.queryByRole('button', { name: /Redact Area/i })).not.toBeInTheDocument();
  });
});
