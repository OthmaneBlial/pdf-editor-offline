import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';

import RedactProveWorkflow from '../src/components/workflows/RedactProveWorkflow';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
  isAxiosError: ReturnType<typeof vi.fn>;
};

const target = 'SYNTHETIC_SECRET';
const reviewToken = 'a'.repeat(64);

const reviewResponse = {
  data: {
    success: true,
    data: {
      mark_count: 1,
      target_count: 1,
      pages_affected: [0],
      actions: [
        'permanently_remove_marked_content',
        'remove_hidden_data_and_previous_revisions',
        'reopen_with_independent_engines',
        'save_as_a_new_verified_copy',
      ],
      source_will_be_preserved: true,
      review_token: reviewToken,
    },
  },
};

const verifiedResponse = {
  data: {
    success: true,
    data: {
      status: 'verified',
      copy: {
        id: 'copy-1',
        filename: 'guarded-redacted-verified.pdf',
        download_url: '/api/documents/copy-1/download',
      },
      verification: {
        status: 'verified',
        app_version: '2.1.0',
        output_sha256: 'b'.repeat(64),
        output_bytes: 4096,
        page_count: 1,
        target_count: 1,
        checks: Array.from({ length: 10 }, (_, index) => ({
          id: `check-${index}`,
          label: `Check ${index}`,
          status: 'passed',
          items_checked: 1,
          matches: 0,
        })),
        warnings: [],
      },
      reports: {
        json: '/api/documents/copy-1/redaction-report/json',
        markdown: '/api/documents/copy-1/redaction-report/markdown',
      },
    },
  },
};

const searchResponse = {
  data: {
    success: true,
    data: {
      count: 1,
      matches: [{ index: 0, rect: [72, 60, 180, 82] }],
    },
  },
};

const buildReviewedPlan = async () => {
  render(<RedactProveWorkflow />);
  fireEvent.change(screen.getByLabelText('Exact text to remove'), {
    target: { value: target },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Search page' }));
  await screen.findByText('1 exact occurrence found on page 1.');
  fireEvent.click(screen.getByRole('button', { name: 'Add all to plan' }));
  fireEvent.click(screen.getByRole('button', { name: 'Review destructive actions' }));
  await screen.findByText('Final review');
};

describe('RedactProveWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorMock.mockReturnValue({ sessionId: 'doc-1', currentPage: 0 });
    mockedAxios.post.mockImplementation((url: string) => {
      if (url.endsWith('/text/search')) return Promise.resolve(searchResponse);
      if (url.endsWith('/redaction/review')) return Promise.resolve(reviewResponse);
      if (url.endsWith('/redaction/apply')) return Promise.resolve(verifiedResponse);
      return Promise.reject(new Error('Unexpected endpoint'));
    });
    mockedAxios.isAxiosError.mockReturnValue(false);
  });

  it('requires a document before exposing the workflow', () => {
    useEditorMock.mockReturnValue({ sessionId: '', currentPage: 0 });
    render(<RedactProveWorkflow />);
    expect(screen.getByText(/Upload a PDF first/i)).toBeInTheDocument();
  });

  it('moves from exact matches through review to a verified copy', async () => {
    await buildReviewedPlan();

    expect(screen.queryByText(target)).not.toBeInTheDocument();
    expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/redaction/review'),
      expect.objectContaining({
        targets: [target],
        review_acknowledged: false,
        review_token: null,
      }),
    );

    const applyButton = screen.getByRole('button', { name: 'Apply & verify copy' });
    expect(applyButton).toBeDisabled();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(applyButton);

    expect(await screen.findByText('Removal verified')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Download verified PDF' })).toBeInTheDocument();
    expect(screen.getByText('b'.repeat(64))).toBeInTheDocument();
    expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/redaction/apply'),
      expect.objectContaining({
        targets: [target],
        review_acknowledged: true,
        review_token: reviewToken,
      }),
    );
  });

  it('shows a fail-closed state when verification is incomplete', async () => {
    mockedAxios.post.mockImplementation((url: string) => {
      if (url.endsWith('/text/search')) return Promise.resolve(searchResponse);
      if (url.endsWith('/redaction/review')) return Promise.resolve(reviewResponse);
      if (url.endsWith('/redaction/apply')) {
        return Promise.reject({
          response: {
            status: 422,
            data: {
              data: {
                verification: {
                  status: 'incomplete',
                  warnings: ['rendered_ocr_unavailable'],
                },
              },
            },
          },
        });
      }
      return Promise.reject(new Error('Unexpected endpoint'));
    });
    mockedAxios.isAxiosError.mockReturnValue(true);
    await buildReviewedPlan();

    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByRole('button', { name: 'Apply & verify copy' }));

    expect(await screen.findByText('Verification stopped safely')).toBeInTheDocument();
    expect(screen.getByText(/No green status was issued/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Download verified PDF' })).not.toBeInTheDocument();
  });
});
