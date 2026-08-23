import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import OrganizePagesWorkflow from '../src/components/workflows/OrganizePagesWorkflow';

const editor = vi.hoisted(() => ({
  sessionId: 'doc-1' as string | null,
  pageCount: 3,
  currentPage: 0,
  setCurrentPage: vi.fn(),
  setPageCount: vi.fn(),
  documentMutationVersion: 0,
  reportToolResult: vi.fn(),
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
    isAxiosError: vi.fn(() => false),
  },
}));

const mockedAxios = vi.mocked(axios, true);

describe('OrganizePagesWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    editor.sessionId = 'doc-1';
    editor.pageCount = 3;
    editor.currentPage = 0;
    mockedAxios.get.mockResolvedValue({ data: { data: { image: 'data:image/png;base64,cGFnZQ==' } } });
    mockedAxios.post.mockResolvedValue({
      data: {
        data: {
          page_count: 3,
          warnings: ['bookmarks_may_require_review'],
          can_undo: true,
          can_redo: false,
        },
      },
    });
    mockedAxios.put.mockResolvedValue({
      data: {
        data: {
          page_count: 3,
          warnings: ['document_reading_order_changes'],
          can_undo: true,
          can_redo: false,
        },
      },
    });
  });

  it('shows a focused upload prompt without a document', () => {
    editor.sessionId = null;
    render(<OrganizePagesWorkflow />);
    expect(screen.getByRole('heading', { name: 'Organize Pages' })).toBeInTheDocument();
    expect(screen.getByText(/Upload a PDF to select/)).toBeInTheDocument();
  });

  it('renders real previews and runs a batch operation on an odd-page selection', async () => {
    render(<OrganizePagesWorkflow />);

    expect(await screen.findByAltText('Page 1 preview')).toBeInTheDocument();
    expect(screen.getByAltText('Page 2 preview')).toBeInTheDocument();
    expect(screen.getByAltText('Page 3 preview')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Odd' }));
    expect(screen.getByRole('checkbox', { name: 'Select page 1' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select page 2' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select page 3' })).toBeChecked();

    fireEvent.click(screen.getByRole('button', { name: /Rotate right/ }));
    await waitFor(() => expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/api/documents/doc-1/pages/organize'),
      expect.objectContaining({ action: 'rotate_right', pages: [0, 2] }),
    ));
    expect(await screen.findByRole('status')).toHaveTextContent('Bookmarks may need destination review.');
    expect(screen.getByRole('button', { name: 'Undo page operation' })).toBeEnabled();
  });

  it('supports custom ranges, non-drag reordering, and unified undo', async () => {
    render(<OrganizePagesWorkflow />);
    fireEvent.change(screen.getByPlaceholderText('1-3, 7, 10-12'), { target: { value: '1-2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Select range' }));

    fireEvent.click(screen.getByRole('button', { name: 'Move selected pages right' }));
    await waitFor(() => expect(mockedAxios.put).toHaveBeenCalledWith(
      expect.stringContaining('/api/documents/doc-1/pages/reorder'),
      { page_order: [0, 2, 1] },
    ));

    fireEvent.click(screen.getByRole('button', { name: 'Undo page operation' }));
    await waitFor(() => expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/api/documents/doc-1/pages/organize/undo'),
    ));
  });

  it('keeps at least one page when every page is selected', () => {
    render(<OrganizePagesWorkflow />);
    fireEvent.click(screen.getByRole('button', { name: 'All' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(screen.getByText('Keep at least one page in the PDF.')).toBeInTheDocument();
    expect(mockedAxios.post).not.toHaveBeenCalled();
  });
});
