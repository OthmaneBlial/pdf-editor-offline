import React from 'react';
import { fireEvent, render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

import FileUpload from '../src/components/FileUpload';
import Header from '../src/components/Header';
import { ThemeProvider } from '../src/contexts/ThemeContext';

const useEditorMock = vi.fn();

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

vi.mock('../src/components/RuntimeHealthPanel', () => ({
  default: () => <button type="button">Runtime status</button>,
}));

vi.mock('../src/components/RecoveryCenter', () => ({
  default: () => <button type="button">Recovery drafts</button>,
}));

describe('FileUpload', () => {
  beforeEach(() => {
    useEditorMock.mockReturnValue({
      setDocument: vi.fn(),
      sessionId: '',
      isUploading: false,
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('opens the same selected file again after resetting the native input', () => {
    const file = new File(['content'], 'resume.pdf', { type: 'application/pdf' });
    const setDocument = vi.fn();
    useEditorMock.mockReturnValue({ setDocument });

    const { container } = render(<FileUpload />);
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [file] } });
    fireEvent.change(input, { target: { files: [file] } });

    expect(setDocument).toHaveBeenCalledTimes(2);
    expect(setDocument).toHaveBeenNthCalledWith(1, file);
    expect(setDocument).toHaveBeenNthCalledWith(2, file);
    expect(input.value).toBe('');
  });

  it('accepts Windows drops whose PDF MIME type is empty', () => {
    const file = new File(['content'], 'windows-sandbox.PDF', { type: '' });
    const setDocument = vi.fn();
    useEditorMock.mockReturnValue({ setDocument });

    const { getByRole } = render(<FileUpload />);
    fireEvent.drop(getByRole('button', { name: /Upload PDF file/i }), {
      dataTransfer: { files: [file] },
    });

    expect(setDocument).toHaveBeenCalledWith(file);
  });
});

describe('Header', () => {
  const renderHeader = () => render(
    <ThemeProvider>
      <Header />
    </ThemeProvider>
  );

  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('invokes exportPDF when Export button is clicked', () => {
    const exportPDF = vi.fn();
    const saveChanges = vi.fn();
    useEditorMock.mockReturnValue({ exportPDF, saveChanges, hasUnsavedChanges: true, sessionId: 'abc', isUploading: false });

    const { getByRole } = renderHeader();
    fireEvent.click(getByRole('button', { name: /Export/i }));

    expect(exportPDF).toHaveBeenCalledTimes(1);
  });

  it('toggles Save button enabled state based on hasUnsavedChanges', () => {
    const saveChanges = vi.fn();
    useEditorMock.mockReturnValue({
      exportPDF: vi.fn(),
      saveChanges,
      hasUnsavedChanges: false,
      sessionId: 'abc',
      isUploading: false,
    });

    const { rerender, getByRole } = renderHeader();
    const saveButton = getByRole('button', { name: /Save/i }) as HTMLButtonElement;
    expect(saveButton.getAttribute('disabled')).not.toBeNull();

    useEditorMock.mockReturnValue({
      exportPDF: vi.fn(),
      saveChanges,
      hasUnsavedChanges: true,
      sessionId: 'abc',
      isUploading: false,
    });
    rerender(
      <ThemeProvider>
        <Header />
      </ThemeProvider>
    );
    const enabledButton = getByRole('button', { name: /Save/i }) as HTMLButtonElement;
    expect(enabledButton.getAttribute('disabled')).toBeNull();
    fireEvent.click(enabledButton);
    expect(saveChanges).toHaveBeenCalledTimes(1);
  });

  it('applies dark mode to the document root and persists it', async () => {
    useEditorMock.mockReturnValue({
      exportPDF: vi.fn(),
      saveChanges: vi.fn(),
      hasUnsavedChanges: false,
      sessionId: '',
      isUploading: false,
    });

    const { getByRole } = renderHeader();
    fireEvent.click(getByRole('button', { name: 'Switch to dark mode' }));

    await waitFor(() => expect(document.documentElement).toHaveAttribute('data-theme', 'dark'));
    expect(window.localStorage.getItem('pdf-editor-theme')).toBe('dark');
  });
});
