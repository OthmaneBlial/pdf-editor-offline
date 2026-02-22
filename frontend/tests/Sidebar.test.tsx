import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import Sidebar from '../src/components/Sidebar';

vi.mock('../src/components/Toolbar', () => ({
  default: () => <div data-testid="toolbar">Toolbar</div>,
}));

vi.mock('../src/components/HistoryPanel', () => ({
  default: () => <div data-testid="history-panel">History Panel</div>,
}));

vi.mock('../src/components/CollaborativeAnnotations', () => ({
  default: () => <div data-testid="comments">Comments</div>,
}));

vi.mock('../src/components/ImageUpload', () => ({
  default: () => <div data-testid="image-upload">Image Upload</div>,
}));

vi.mock('../src/components/FileUpload', () => ({
  default: () => <div data-testid="file-upload">File Upload</div>,
}));

vi.mock('../src/components/RecentFiles', () => ({
  default: () => <div data-testid="recent-files">Recent Files</div>,
}));

vi.mock('../src/components/FullscreenButton', () => ({
  default: () => <button aria-label="Exit fullscreen mode">Fullscreen</button>,
}));

describe('Sidebar Component', () => {
  const createProps = () => ({
    activeView: 'editor' as const,
    onViewChange: vi.fn(),
    onShowShortcuts: vi.fn(),
  });

  it('renders brand and primary sections', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    expect(screen.getByText('PDF Editor Offline')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Basic Tools/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Advanced Editing/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Tools$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /History/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Comments/i })).toBeInTheDocument();
  });

  it('calls onViewChange when a navigation item is clicked', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /Manipulation/i }));

    expect(props.onViewChange).toHaveBeenCalledWith('manipulation');
  });

  it('reveals tool content when Tools section is expanded', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /^Tools$/i }));

    expect(screen.getByTestId('toolbar')).toBeInTheDocument();
    expect(screen.getByTestId('image-upload')).toBeInTheDocument();
  });

  it('calls onShowShortcuts from the shortcuts action button', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /Show keyboard shortcuts/i }));

    expect(props.onShowShortcuts).toHaveBeenCalledTimes(1);
  });
});
