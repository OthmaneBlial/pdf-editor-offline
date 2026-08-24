import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { getMock, postMock } = vi.hoisted(() => ({
  getMock: vi.fn(),
  postMock: vi.fn(),
}));

vi.mock('axios', () => ({
  default: {
    defaults: { headers: { common: {} } },
    create: vi.fn(() => ({ get: getMock, post: postMock })),
    get: getMock,
    post: postMock,
  },
}));

import RuntimeHealthPanel from '../src/components/RuntimeHealthPanel';

const capabilityResponse = {
  data: {
    version: '2.1.0',
    ready: true,
    all_optional_tools_available: false,
    runtime: { python: '3.12.0', platform: 'darwin', architecture: 'arm64' },
    network: {
      telemetry: false,
      api_auth_required: true,
      bind_host: '127.0.0.1',
      processing: 'this-device',
    },
    external_tools: {
      libreoffice: { available: false, path: null, enables: ['word-to-pdf'] },
      tesseract: { available: true, path: '/local/tesseract', languages: ['eng'], enables: ['ocr'] },
      ghostscript: { available: true, path: '/local/gs', enables: ['pdf-a'] },
    },
    storage: {
      session_bytes: 1024,
      temporary_bytes: 512,
      scope: 'app-owned-local-storage',
    },
  },
};

const inventoryResponse = {
  data: {
    success: true,
    data: {
      session_files: 2,
      active_sessions: 1,
      session_bytes: 1024,
      report_files: 2,
      report_bytes: 512,
      recovery_files: 1,
      recovery_bytes: 384,
      draft_files: 1,
      draft_bytes: 256,
      temporary_files: 3,
      temporary_bytes: 768,
    },
  },
};

describe('RuntimeHealthPanel', () => {
  beforeEach(() => {
    getMock.mockReset().mockImplementation((url: string) =>
      Promise.resolve(url.includes('/maintenance/storage') ? inventoryResponse : capabilityResponse)
    );
    postMock.mockReset().mockResolvedValue({ data: { success: true } });
  });

  it('shows the local trust contract and optional capability state', async () => {
    render(<RuntimeHealthPanel />);
    const trigger = screen.getByRole('button', { name: 'Open local runtime status' });
    await waitFor(() => expect(getMock).toHaveBeenCalled());

    fireEvent.click(trigger);

    expect(screen.getByRole('dialog', { name: 'Processed on this device' })).toBeInTheDocument();
    expect(screen.getByText('127.0.0.1')).toBeInTheDocument();
    expect(screen.getByText('Required')).toBeInTheDocument();
    expect(screen.getByText('Off')).toBeInTheDocument();
    expect(screen.getByText('LibreOffice')).toBeInTheDocument();
    expect(screen.getByText('Optional')).toBeInTheDocument();
  });

  it('closes on Escape and restores focus to the trigger', async () => {
    render(<RuntimeHealthPanel />);
    const trigger = screen.getByRole('button', { name: 'Open local runtime status' });
    await waitFor(() => expect(getMock).toHaveBeenCalled());
    fireEvent.click(trigger);

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it('cleans stale app-owned data and refreshes capability sizes', async () => {
    render(<RuntimeHealthPanel />);
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('button', { name: 'Open local runtime status' }));
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole('button', { name: 'Clean stale local data' }));

    await waitFor(() => expect(postMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(4));
  });

  it('inspects and deletes all app-owned local data after explicit confirmation', async () => {
    const cleared = vi.fn();
    localStorage.setItem('pdf-editor-visual-signatures', '[{"id":"local-signature"}]');
    window.addEventListener('pdf-local-data-cleared', cleared);
    render(<RuntimeHealthPanel />);
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('button', { name: 'Open local runtime status' }));

    expect(await screen.findByText('Session PDFs')).toBeInTheDocument();
    expect(screen.getByText('Drafts / recovery')).toBeInTheDocument();
    const deleteButton = screen.getByRole('button', { name: 'Delete all local workspace data' });
    expect(deleteButton).toBeDisabled();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(deleteButton);

    await waitFor(() => expect(postMock).toHaveBeenCalledWith(
      expect.stringContaining('/maintenance/cleanup'),
      { delete_all_app_data: true },
    ));
    await waitFor(() => expect(cleared).toHaveBeenCalledTimes(1));
    expect(localStorage.getItem('pdf-editor-visual-signatures')).toBeNull();
    expect(screen.getByText(/All app-owned documents/i)).toBeInTheDocument();
    window.removeEventListener('pdf-local-data-cleared', cleared);
  });
});
