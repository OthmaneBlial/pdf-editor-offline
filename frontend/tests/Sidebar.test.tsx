import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Sidebar from '../components/Sidebar';

// Mock the EditorContext
vi.mock('../contexts/EditorContext', () => ({
  useEditor: vi.fn(() => ({
    currentPage: 0,
    pageCount: 5,
    sessionId: 'test-session-123',
    hasUnsavedChanges: false,
    zoom: 1,
    setDocument: vi.fn(),
    setCurrentPage: vi.fn(),
    setCanvas: vi.fn(),
    setDrawingMode: vi.fn(),
    setColor: vi.fn(),
    setStrokeWidth: vi.fn(),
    setFontSize: vi.fn(),
    setFontFamily: vi.fn(),
    setCanvasObjects: vi.fn(),
    setSessionId: vi.fn(),
    setPageCount: vi.fn(),
    setZoom: vi.fn(),
    setIsUploading: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    saveChanges: vi.fn(),
    exportPDF: vi.fn(),
    reorderPages: vi.fn(),
    clearHistory: vi.fn(),
    history: [],
    historyStep: -1,
    canUndo: false,
    canRedo: false,
  })),
}));

// Mock child components
vi.mock('../components/Toolbar', () => ({
  default: () => <div data-testid="toolbar">Toolbar</div>,
}));

vi.mock('../components/HistoryPanel', () => ({
  default: () => <div data-testid="history-panel">History Panel</div>,
}));

vi.mock('../components/CollaborativeAnnotations', () => ({
  default: () => <div data-testid="comments">Comments</div>,
}));

vi.mock('../components/ImageUpload', () => ({
  default: () => <div data-testid="image-upload">Image Upload</div>,
}));

vi.mock('../components/FileUpload', () => ({
  default: () => <div data-testid="file-upload">File Upload</div>,
}));

vi.mock('../components/RecentFiles', () => ({
  default: () => <div data-testid="recent-files">Recent Files</div>,
}));

vi.mock('../components/FullscreenButton', () => ({
  default: () => <button aria-label="Exit fullscreen mode">Fullscreen</button>,
}));

describe('Sidebar Component', () => {
  const mockProps = {
    activeView: 'editor' as const,
    onViewChange: vi.fn(),
    onShowShortcuts: vi.fn(),
  };

  it('renders correctly with default state', () => {
    render(<Sidebar {...mockProps} />);

    expect(screen.getByText('PDF Editor Offline')).toBeInTheDocument();
    expect(screen.getByText('PRO')).toBeInTheDocument();
  });

  it('renders all tab buttons', () => {
    render(<Sidebar {...mockProps} />);

    expect(screen.getByRole('tab', { name: /navigation/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /tools/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /history/i })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /comments/i })).toBeInTheDocument();
  });

  it('switches between tabs correctly', () => {
    render(<Sidebar {...mockProps} />);

    // Initially Navigation tab should be selected
    const navigationTab = screen.getByRole('tab', { name: /navigation/i });
    expect(navigationTab).toHaveAttribute('aria-selected', 'true');

    // Click on Tools tab
    const toolsTab = screen.getByRole('tab', { name: /tools/i });
    fireEvent.click(toolsTab);
    expect(toolsTab).toHaveAttribute('aria-selected', 'true');
    expect(navigationTab).toHaveAttribute('aria-selected', 'false');
  });

  it('calls onViewChange when navigation item is clicked', () => {
    render(<Sidebar {...mockProps} />);

    const manipulationButton = screen.getByRole('menuitem', { name: /manipulation/i });
    fireEvent.click(manipulationButton);

    expect(mockProps.onViewChange).toHaveBeenCalledWith('manipulation');
  });

  it('calls onShowShortcuts when keyboard button is clicked', () => {
    render(<Sidebar {...mockProps} />);

    const keyboardButton = screen.getByLabelText(/show keyboard shortcuts/i);
    fireEvent.click(keyboardButton);

    expect(mockProps.onShowShortcuts).toHaveBeenCalled();
  });

  it('shows navigation items by default', () => {
    render(<Sidebar {...mockProps} />);

    expect(screen.getByRole('menuitem', { name: /editor/i })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /manipulation/i })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /conversion/i })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /security/i })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /advanced/i })).toBeInTheDocument();
  });

  it('marks active navigation item with aria-current', () => {
    render(<Sidebar {...mockProps} activeView="manipulation" />);

    const manipulationButton = screen.getByRole('menuitem', { name: /manipulation/i });
    expect(manipulationButton).toHaveAttribute('aria-current', 'page');
  });

  it('shows toolbar in tools tab', () => {
    render(<Sidebar {...mockProps} />);

    // Switch to tools tab
    const toolsTab = screen.getByRole('tab', { name: /tools/i });
    fireEvent.click(toolsTab);

    expect(screen.getByTestId('toolbar')).toBeInTheDocument();
  });

  it('has proper accessibility attributes', () => {
    render(<Sidebar {...mockProps} />);

    const tablist = screen.getByRole('tab', { name: /navigation/i }).parentElement;
    expect(tablist).toHaveAttribute('role', 'group');

    const tabs = screen.getAllByRole('tab');
    tabs.forEach(tab => {
      expect(tab).toHaveAttribute('aria-selected');
      expect(tab).toHaveAttribute('aria-controls');
    });
  });
});
