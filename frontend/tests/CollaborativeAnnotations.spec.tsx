import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';

import CollaborativeAnnotations from '../src/components/CollaborativeAnnotations';

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

describe('CollaborativeAnnotations', () => {
  beforeEach(() => {
    localStorage.clear();
    useEditorMock.mockReturnValue({ currentPage: 0 });
  });

  it('keeps username prompt visible when trying to add a comment without a username', () => {
    render(<CollaborativeAnnotations />);

    expect(screen.getByText('Enter your name for comments')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Add comment/i }));

    expect(screen.getByText('Enter your name for comments')).toBeInTheDocument();
    expect(screen.queryByPlaceholderText('Write your comment...')).not.toBeInTheDocument();
  });

  it('opens add-comment form when username exists', () => {
    localStorage.setItem('pdf_editor_username', 'Alice');
    render(<CollaborativeAnnotations />);

    fireEvent.click(screen.getByRole('button', { name: /Add comment/i }));

    expect(screen.getByPlaceholderText('Write your comment...')).toBeInTheDocument();
  });
});
