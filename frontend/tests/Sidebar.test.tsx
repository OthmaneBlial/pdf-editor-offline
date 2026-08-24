import React from 'react';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import Sidebar from '../src/components/Sidebar';

vi.mock('../src/components/Toolbar', () => ({ default: () => <div data-testid="toolbar">Toolbar</div> }));
vi.mock('../src/components/HistoryPanel', () => ({ default: () => <div data-testid="history-panel">History Panel</div> }));
vi.mock('../src/components/CollaborativeAnnotations', () => ({ default: () => <div data-testid="comments">Comments</div> }));
vi.mock('../src/components/ImageUpload', () => ({ default: () => <div data-testid="image-upload">Image Upload</div> }));
vi.mock('../src/components/FileUpload', () => ({ default: () => <div data-testid="file-upload">File Upload</div> }));
vi.mock('../src/components/RecentFiles', () => ({ default: () => <div data-testid="recent-files">Recent Files</div> }));
vi.mock('../src/components/FullscreenButton', () => ({ default: () => <button aria-label="Exit fullscreen mode">Fullscreen</button> }));

describe('Sidebar Component', () => {
  const createProps = () => ({
    activeView: 'editor' as const,
    onViewChange: vi.fn(),
    onShowShortcuts: vi.fn(),
    onOpenCommandPalette: vi.fn(),
  });

  it('leads with exactly five task-based primary workflows', () => {
    render(<Sidebar {...createProps()} />);

    const primary = screen.getByRole('navigation', { name: 'Primary workflows' });
    const workflows = within(primary).getAllByRole('button');
    expect(workflows).toHaveLength(5);
    expect(workflows.map(button => button.textContent)).toEqual(expect.arrayContaining([
      expect.stringContaining('Redact & Prove'),
      expect.stringContaining('Fill & Sign'),
      expect.stringContaining('Organize Pages'),
      expect.stringContaining('Sanitize & Share'),
      expect.stringContaining('OCR & Search'),
    ]));
  });

  it('opens a primary workflow without expanding All tools', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /Organize Pages/i }));

    expect(props.onViewChange).toHaveBeenCalledWith('manipulation');
    expect(screen.queryByRole('button', { name: /^Editor$/i })).not.toBeInTheDocument();
  });

  it('opens the command palette from the searchable navigation action', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /Search all workflows and tools/i }));

    expect(props.onOpenCommandPalette).toHaveBeenCalledTimes(1);
  });

  it('progressively reveals specialist tools', () => {
    render(<Sidebar {...createProps()} />);
    const trigger = screen.getByRole('button', { name: /All tools/i });

    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('button', { name: /^Editor$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Advanced$/i })).toBeInTheDocument();
  });

  it('reveals workspace utilities independently', () => {
    render(<Sidebar {...createProps()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Canvas tools' }));
    expect(screen.getByTestId('toolbar')).toBeInTheDocument();
    expect(screen.getByTestId('image-upload')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    expect(screen.queryByTestId('toolbar')).not.toBeInTheDocument();
    expect(screen.getByTestId('history-panel')).toBeInTheDocument();
  });

  it('calls onShowShortcuts from the keyboard help action', () => {
    const props = createProps();
    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole('button', { name: /Show keyboard shortcuts/i }));

    expect(props.onShowShortcuts).toHaveBeenCalledTimes(1);
  });
});
