import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';

import OCRSearchWorkflow from '../src/components/workflows/OCRSearchWorkflow';

const editor = vi.hoisted(() => ({
  sessionId: 'source-1',
  pageCount: 3,
  restoreRecoveredDocument: vi.fn(),
  reportToolResult: vi.fn(),
  setCurrentPage: vi.fn(),
}));

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => editor,
}));

vi.mock('../src/lib/apiClient', () => ({
  API_BASE_URL: 'http://127.0.0.1:8000',
}));

vi.mock('axios', () => ({
  default: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
    isAxiosError: vi.fn((error: { isAxiosError?: boolean }) => Boolean(error?.isAxiosError)),
  },
}));

const mockedAxios = vi.mocked(axios, true);
const data = <T,>(value: T) => ({ data: { data: value } });
const notFound = { isAxiosError: true, response: { status: 404, data: { detail: 'OCR layer not found' } } };

const capabilities = {
  available: true,
  engine: 'tesseract',
  version: 'tesseract 5.5.0',
  languages: ['eng', 'fra'],
  hidden_downloads: false,
  orientation_data_available: true,
};

const layer = {
  layer_status: 'active',
  source_preserved: true,
  visual_source_preserved: true,
  word_count: 2,
  average_confidence: 77.25,
  pages_processed: 1,
  pages: [{
    page: 0,
    word_count: 2,
    average_confidence: 77.25,
    minimum_confidence: 61.5,
    deskew_degrees: 1.5,
    orientation_degrees: 0,
    correction_count: 0,
    layer_status: 'active',
  }],
};

const page = {
  ...layer.pages[0],
  words: [
    { id: 'p1-w1', text: 'OFFL1NE', confidence: 61.5, bbox: [1, 2, 3, 4] },
    { id: 'p1-w2', text: 'SEARCH', confidence: 93, bbox: [5, 6, 7, 8] },
  ],
};

describe('OCRSearchWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    editor.sessionId = 'source-1';
    editor.pageCount = 3;
    mockedAxios.get.mockImplementation(async url => {
      const path = String(url);
      if (path.endsWith('/api/ocr/capabilities')) return data(capabilities);
      if (path.endsWith('/ocr/jobs')) return data({ jobs: [] });
      if (path.endsWith('/ocr/layer')) return Promise.reject(notFound);
      return data({});
    });
  });

  it('shows a focused source-preserving prompt without a document', () => {
    editor.sessionId = '';
    render(<OCRSearchWorkflow />);
    expect(screen.getByRole('heading', { name: 'OCR & Search' })).toBeInTheDocument();
    expect(screen.getByText(/always creates a separate searchable copy/i)).toBeInTheDocument();
  });

  it('queues an explicit multilingual background recipe with no hidden download', async () => {
    mockedAxios.post.mockResolvedValue(data({
      id: 'job-1',
      source_document_id: 'source-1',
      status: 'queued',
      progress: 0,
      pages_completed: 0,
      pages_total: 2,
      current_page: null,
      stage: 'queued',
      can_cancel: true,
      can_retry: false,
      result: null,
      error: null,
    }));
    render(<OCRSearchWorkflow />);

    expect(await screen.findByText('No hidden downloads')).toBeInTheDocument();
    fireEvent.click(screen.getByText(/French/));
    fireEvent.change(screen.getByLabelText('Page range'), { target: { value: '1-2' } });
    fireEvent.change(screen.getByLabelText('Render quality'), { target: { value: '240' } });
    fireEvent.change(screen.getByLabelText('Minimum OCR confidence'), { target: { value: '65' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create searchable copy' }));

    await waitFor(() => expect(mockedAxios.post).toHaveBeenCalledWith(
      'http://127.0.0.1:8000/api/documents/source-1/ocr/jobs',
      expect.objectContaining({
        page_range: '1-2',
        languages: ['eng', 'fra'],
        dpi: 240,
        auto_rotate: true,
        deskew: true,
        minimum_confidence: 65,
      }),
    ));
    expect(await screen.findByRole('progressbar', { name: 'OCR progress' })).toHaveAttribute('aria-valuenow', '0');
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeEnabled();
  });

  it('searches, exposes confidence, and saves corrected OCR words', async () => {
    mockedAxios.get.mockImplementation(async url => {
      const path = String(url);
      if (path.endsWith('/api/ocr/capabilities')) return data(capabilities);
      if (path.endsWith('/ocr/jobs')) return data({ jobs: [] });
      if (path.endsWith('/ocr/layer')) return data(layer);
      if (path.endsWith('/ocr/layer/pages/0')) return data(page);
      return data({});
    });
    mockedAxios.post.mockImplementation(async (url, body) => {
      if (String(url).endsWith('/ocr/search')) {
        expect(body).toEqual({ text: 'offline' });
        return data({
          matches: [{ page: 0, word_id: 'p1-w1', text: 'OFFL1NE', confidence: 61.5, context: 'OFFL1NE SEARCH' }],
          truncated: false,
        });
      }
      return data({});
    });
    mockedAxios.put.mockResolvedValue(data({ correction_count: 1 }));
    render(<OCRSearchWorkflow />);

    const word = await screen.findByRole('textbox', { name: 'OCR text p1-w1' });
    expect(screen.getByText('61.5%')).toBeInTheDocument();
    fireEvent.change(word, { target: { value: 'OFFLINE' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save 1 correction' }));
    await waitFor(() => expect(mockedAxios.put).toHaveBeenCalledWith(
      'http://127.0.0.1:8000/api/documents/source-1/ocr/layer/pages/0',
      { corrections: [{ id: 'p1-w1', text: 'OFFLINE' }] },
    ));

    fireEvent.change(screen.getByLabelText('Search OCR text'), { target: { value: 'offline' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByText('OFFL1NE SEARCH')).toBeInTheDocument();
    fireEvent.click(screen.getByText('OFFL1NE SEARCH'));
    expect(editor.setCurrentPage).toHaveBeenCalledWith(0);
  });

  it('requires explicit confirmation before removing only the OCR layer', async () => {
    let activeLayer = layer;
    mockedAxios.get.mockImplementation(async url => {
      const path = String(url);
      if (path.endsWith('/api/ocr/capabilities')) return data(capabilities);
      if (path.endsWith('/ocr/jobs')) return data({ jobs: [] });
      if (path.endsWith('/ocr/layer')) return data(activeLayer);
      if (path.endsWith('/ocr/layer/pages/0')) return data(page);
      return data({});
    });
    mockedAxios.delete.mockImplementation(async () => {
      activeLayer = { ...layer, layer_status: 'removed', pages: [{ ...layer.pages[0], layer_status: 'removed' }] };
      return data({ source_scan_preserved: true });
    });
    render(<OCRSearchWorkflow />);

    const remove = await screen.findByRole('button', { name: 'Remove OCR layer' });
    expect(remove).toBeDisabled();
    fireEvent.click(screen.getByLabelText(/I understand this copy/));
    fireEvent.click(remove);

    await waitFor(() => expect(mockedAxios.delete).toHaveBeenCalledWith(
      'http://127.0.0.1:8000/api/documents/source-1/ocr/layer',
    ));
    expect(await screen.findByText(/visual scan remains intact/i)).toBeInTheDocument();
  });

  it('opens a completed result as a separate editor session', async () => {
    const completedJob = {
      id: 'job-done', source_document_id: 'source-1', status: 'succeeded', progress: 100,
      pages_completed: 3, pages_total: 3, current_page: null, stage: 'complete',
      can_cancel: false, can_retry: false, error: null,
      result: {
        document_id: 'copy-2', filename: 'scan-searchable.pdf', page_count: 3,
        download_url: '/api/documents/copy-2/download', source_preserved: true,
        pages_processed: 3, word_count: 42, average_confidence: 88.2,
      },
    };
    mockedAxios.get.mockImplementation(async url => {
      const path = String(url);
      if (path.endsWith('/api/ocr/capabilities')) return data(capabilities);
      if (path.endsWith('/ocr/jobs')) return data({ jobs: [completedJob] });
      if (path.endsWith('/ocr/layer')) return Promise.reject(notFound);
      if (path.endsWith('/api/documents/copy-2/download')) return { data: new Blob(['%PDF'], { type: 'application/pdf' }) };
      return data({});
    });
    render(<OCRSearchWorkflow />);

    fireEvent.click(await screen.findByRole('button', { name: 'Open searchable copy' }));
    await waitFor(() => expect(editor.restoreRecoveredDocument).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'scan-searchable.pdf' }),
      'copy-2',
      3,
    ));
  });
});
