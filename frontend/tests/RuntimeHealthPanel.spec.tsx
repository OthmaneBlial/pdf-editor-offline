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
      session_location: '/local/storage',
      temporary_location: '/local/temp',
    },
  },
};

describe('RuntimeHealthPanel', () => {
  beforeEach(() => {
    getMock.mockReset().mockResolvedValue(capabilityResponse);
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
    fireEvent.click(screen.getByRole('button', { name: 'Clean stale local data' }));

    await waitFor(() => expect(postMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(2));
  });
});
