import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import axios from 'axios';

import ConversionTools from '../src/components/tools/ConversionTools';

vi.mock('axios');
const mockedAxios = axios as unknown as {
  post: ReturnType<typeof vi.fn>;
};

describe('Conversion Features', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('submits PDF to Word conversion from the default tab', async () => {
    mockedAxios.post.mockResolvedValue({
      data: new Blob(['test content'], {
        type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      }),
      headers: {
        'content-disposition': 'attachment; filename="converted.docx"',
      },
    });

    render(<ConversionTools />);

    const convertButton = screen.getByRole('button', { name: /Convert to Word/i });
    const form = convertButton.closest('form') as HTMLFormElement;
    const fileInput = form.querySelector('input[type="file"]') as HTMLInputElement;

    fireEvent.change(fileInput, {
      target: {
        files: [new File(['pdf'], 'input.pdf', { type: 'application/pdf' })],
      },
    });

    fireEvent.submit(form);

    await waitFor(() => {
      expect(mockedAxios.post).toHaveBeenCalledWith(
        expect.stringContaining('/api/tools/pdf-to-word'),
        expect.any(FormData),
        expect.objectContaining({ responseType: 'blob' })
      );
    });
  });
});
