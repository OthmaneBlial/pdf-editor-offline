import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import ToolToast from '../src/components/ToolToast';

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

describe('ToolToast', () => {
  it('renders and dismisses active toast', () => {
    const clearToolToast = vi.fn();
    useEditorMock.mockReturnValue({
      toolToast: { type: 'success', text: 'Mutation saved' },
      clearToolToast,
    });

    render(<ToolToast />);
    expect(screen.getByText('Mutation saved')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Dismiss notification/i }));
    expect(clearToolToast).toHaveBeenCalledTimes(1);
  });

  it('renders nothing when there is no toast', () => {
    useEditorMock.mockReturnValue({
      toolToast: null,
      clearToolToast: vi.fn(),
    });

    const { container } = render(<ToolToast />);
    expect(container.firstChild).toBeNull();
  });
});
