import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import '@testing-library/jest-dom';

import AnnotationTools from '../src/components/tools/AnnotationTools';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  post: ReturnType<typeof vi.fn>;
};

describe('AnnotationTools', () => {
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
    mockedAxios.post.mockResolvedValue({ data: { success: true } });
  });

  it('uploads file attachment annotation using multipart endpoint', async () => {
    render(<AnnotationTools />);
    const submitButton = screen.getByRole('button', { name: /Add File Attachment/i });
    const fileForm = submitButton.closest('form') as HTMLFormElement;
    const fileInput = fileForm.querySelector('input[type="file"]') as HTMLInputElement;

    fireEvent.change(fileInput, {
      target: { files: [new File(['x'], 'attachment.txt', { type: 'text/plain' })] },
    });
    fireEvent.submit(fileForm);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/annotations/file/upload'),
        expect.any(FormData)
      );
    });
  });

  it('uploads sound annotation using multipart endpoint', async () => {
    render(<AnnotationTools />);
    fireEvent.click(screen.getByRole('button', { name: /Sound/i }));

    const submitButton = screen.getByRole('button', { name: /Add Sound Annotation/i });
    const soundForm = submitButton.closest('form') as HTMLFormElement;
    const fileInput = soundForm.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(fileInput, {
      target: { files: [new File(['audio'], 'audio.wav', { type: 'audio/wav' })] },
    });
    fireEvent.submit(soundForm);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/documents/doc-1/annotations/sound/upload'),
        expect.any(FormData)
      );
    });
  });
});
