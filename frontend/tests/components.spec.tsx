import React from 'react';
import { fireEvent, render } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

import FileUpload from '../src/components/FileUpload';
import Header from '../src/components/Header';
import { ThemeProvider } from '../src/contexts/ThemeContext';

const useEditorMock = vi.fn();

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
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

  it('calls setDocument when a file is selected', () => {
    const file = new File(['content'], 'resume.pdf', { type: 'application/pdf' });
    const setDocument = vi.fn();
    useEditorMock.mockReturnValue({ setDocument });

    const { container } = render(<FileUpload />);
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [file] } });

    expect(setDocument).toHaveBeenCalledWith(file);
  });
});

describe('Header', () => {
  const renderHeader = () => render(
    <ThemeProvider>
      <Header />
    </ThemeProvider>
  );

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
});
