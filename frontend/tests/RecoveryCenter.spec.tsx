import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { getMock, postMock, deleteMock, restoreMock } = vi.hoisted(() => ({
  getMock: vi.fn(),
  postMock: vi.fn(),
  deleteMock: vi.fn(),
  restoreMock: vi.fn(),
}));

vi.mock('axios', () => ({
  default: {
    defaults: { headers: { common: {} } },
    create: vi.fn(() => ({ get: getMock, post: postMock, delete: deleteMock })),
    get: getMock,
    post: postMock,
    delete: deleteMock,
  },
}));

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => ({ restoreRecoveredDocument: restoreMock }),
}));

import RecoveryCenter from '../src/components/RecoveryCenter';

const draft = {
  recovery_id: 'draft-1',
  page_count: 2,
  bytes: 2048,
  last_modified: '2026-08-24T00:00:00',
  stage: 'autosave',
  autosave_sequence: 4,
};

describe('RecoveryCenter', () => {
  beforeEach(() => {
    getMock.mockReset().mockImplementation((url: string) => {
      if (url.endsWith('/recovery')) {
        return Promise.resolve({ data: { data: { drafts: [draft] } } });
      }
      if (url.endsWith('/preview')) {
        return Promise.resolve({ data: new Blob(['png']) });
      }
      if (url.endsWith('/download')) {
        return Promise.resolve({ data: new Blob(['pdf']) });
      }
      return Promise.reject(new Error(`Unexpected GET ${url}`));
    });
    postMock.mockReset().mockResolvedValue({
      data: { data: { id: 'restored-1', page_count: 2 } },
    });
    deleteMock.mockReset().mockResolvedValue({ data: { success: true } });
    restoreMock.mockReset();
  });

  it('previews and restores a local copy into the editor', async () => {
    const restoredEvent = vi.fn();
    window.addEventListener('pdf-recovery-restored', restoredEvent);
    render(<RecoveryCenter />);
    expect(await screen.findByRole('button', { name: 'Open recovery drafts (1)' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Open recovery drafts (1)' }));
    expect(screen.getByRole('dialog', { name: 'Continue from a safe copy' })).toBeInTheDocument();
    expect(await screen.findByAltText('Local first-page recovery preview')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Restore copy' }));

    await waitFor(() => expect(postMock).toHaveBeenCalledWith(expect.stringContaining('/draft-1/restore')));
    await waitFor(() => expect(restoreMock).toHaveBeenCalledWith(
      expect.any(File),
      'restored-1',
      2,
    ));
    expect(restoredEvent).toHaveBeenCalledTimes(1);
    window.removeEventListener('pdf-recovery-restored', restoredEvent);
  });

  it('requires a second explicit action before deleting a draft', async () => {
    render(<RecoveryCenter />);
    fireEvent.click(await screen.findByRole('button', { name: 'Open recovery drafts (1)' }));

    fireEvent.click(screen.getByRole('button', { name: 'Delete draft' }));
    expect(deleteMock).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete' }));

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith(expect.stringContaining('/draft-1')));
    expect(await screen.findByText('Recovery copy deleted.')).toBeInTheDocument();
  });
});
