import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { EditorProvider, useEditor } from '../contexts/EditorContext';

// Helper component to test useEditor hook
function TestConsumer() {
    const {
        zoom,
        setZoom,
        currentPage,
        setCurrentPage,
        totalPages,
        selectedTool,
        setSelectedTool,
        hasUnsavedChanges,
        canUndo,
        canRedo,
        undo,
        redo,
        clearHistory,
    } = useEditor();

    return (
        <div>
            <span data-testid="zoom">{zoom}</span>
            <span data-testid="current-page">{currentPage}</span>
            <span data-testid="total-pages">{totalPages}</span>
            <span data-testid="selected-tool">{selectedTool}</span>
            <span data-testid="has-unsaved">{hasUnsavedChanges.toString()}</span>
            <span data-testid="can-undo">{canUndo.toString()}</span>
            <span data-testid="can-redo">{canRedo.toString()}</span>

            <button data-testid="zoom-in" onClick={() => setZoom(zoom + 0.1)}>Zoom In</button>
            <button data-testid="zoom-out" onClick={() => setZoom(zoom - 0.1)}>Zoom Out</button>
            <button data-testid="next-page" onClick={() => setCurrentPage(currentPage + 1)}>Next</button>
            <button data-testid="prev-page" onClick={() => setCurrentPage(currentPage - 1)}>Prev</button>
            <button data-testid="set-tool" onClick={() => setSelectedTool('text')}>Set Text Tool</button>
            <button data-testid="undo-btn" onClick={undo}>Undo</button>
            <button data-testid="redo-btn" onClick={redo}>Redo</button>
            <button data-testid="clear-history" onClick={clearHistory}>Clear</button>
        </div>
    );
}

const renderWithProvider = () => {
    return render(
        <EditorProvider>
            <TestConsumer />
        </EditorProvider>
    );
};

describe('EditorContext', () => {
    describe('Initial State', () => {
        it('provides default zoom level of 1', () => {
            renderWithProvider();
            expect(screen.getByTestId('zoom').textContent).toBe('1');
        });

        it('provides default current page of 1', () => {
            renderWithProvider();
            expect(screen.getByTestId('current-page').textContent).toBe('1');
        });

        it('provides default total pages of 0', () => {
            renderWithProvider();
            expect(screen.getByTestId('total-pages').textContent).toBe('0');
        });

        it('provides default selected tool as select', () => {
            renderWithProvider();
            expect(screen.getByTestId('selected-tool').textContent).toBe('select');
        });

        it('shows no unsaved changes initially', () => {
            renderWithProvider();
            expect(screen.getByTestId('has-unsaved').textContent).toBe('false');
        });

        it('cannot undo initially', () => {
            renderWithProvider();
            expect(screen.getByTestId('can-undo').textContent).toBe('false');
        });

        it('cannot redo initially', () => {
            renderWithProvider();
            expect(screen.getByTestId('can-redo').textContent).toBe('false');
        });
    });

    describe('Zoom Controls', () => {
        it('increases zoom when setZoom is called with higher value', () => {
            renderWithProvider();

            fireEvent.click(screen.getByTestId('zoom-in'));

            const zoom = parseFloat(screen.getByTestId('zoom').textContent!);
            expect(zoom).toBeCloseTo(1.1, 1);
        });

        it('decreases zoom when setZoom is called with lower value', () => {
            renderWithProvider();

            fireEvent.click(screen.getByTestId('zoom-out'));

            const zoom = parseFloat(screen.getByTestId('zoom').textContent!);
            expect(zoom).toBeCloseTo(0.9, 1);
        });
    });

    describe('Tool Selection', () => {
        it('changes selected tool', () => {
            renderWithProvider();

            fireEvent.click(screen.getByTestId('set-tool'));

            expect(screen.getByTestId('selected-tool').textContent).toBe('text');
        });
    });
});

describe('EditorContext Undo/Redo', () => {
    it('clears history when clearHistory is called', () => {
        renderWithProvider();

        // Make some changes first
        fireEvent.click(screen.getByTestId('zoom-in'));

        // Clear history
        fireEvent.click(screen.getByTestId('clear-history'));

        expect(screen.getByTestId('can-undo').textContent).toBe('false');
        expect(screen.getByTestId('can-redo').textContent).toBe('false');
    });
});

describe('EditorProvider Error Handling', () => {
    it('throws error when useEditor is used outside provider', () => {
        // Suppress console.error for this test
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => { });

        expect(() => {
            render(<TestConsumer />);
        }).toThrow();

        consoleSpy.mockRestore();
    });
});
