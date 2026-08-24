import { useEffect } from 'react';
import { useEditor } from '../contexts/EditorContext';

export interface Shortcut {
    keys: string[];
    description: string;
    action: (e: KeyboardEvent) => void;
}

export const KEYBOARD_SHORTCUTS = [
    { keys: ['Ctrl', 'K'], description: 'Find a workflow or tool' },
    { keys: ['Ctrl', 'S'], description: 'Save changes' },
    { keys: ['Ctrl', 'Z'], description: 'Undo' },
    { keys: ['Ctrl', 'Y'], description: 'Redo' },
    { keys: ['Ctrl', 'O'], description: 'Open file' },
    { keys: ['Use PageUp/Down'], description: 'Previous/Next page' },
    { keys: ['Ctrl', '+'], description: 'Zoom in' },
    { keys: ['Ctrl', '-'], description: 'Zoom out' },
    { keys: ['?'], description: 'Show shortcuts' },
];

interface UseKeyboardShortcutsProps {
    onShowHelp?: () => void;
}

export function useKeyboardShortcuts({ onShowHelp }: UseKeyboardShortcutsProps = {}) {
    const {
        saveChanges,
        undo,
        redo,
        currentPage,
        setCurrentPage,
        pageCount,
        setZoom,
        zoom
    } = useEditor();

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            // Ignore if typing in an input
            if ((e.target as HTMLElement).tagName === 'INPUT' || (e.target as HTMLElement).tagName === 'TEXTAREA') {
                return;
            }

            // Save: Ctrl + S
            if ((e.ctrlKey || e.metaKey) && e.key === 's') {
                e.preventDefault();
                saveChanges();
            }

            // Undo: Ctrl + Z
            if ((e.ctrlKey || e.metaKey) && e.key === 'z') {
                e.preventDefault();
                undo();
            }

            // Redo: Ctrl + Y or Ctrl + Shift + Z
            if (((e.ctrlKey || e.metaKey) && e.key === 'y') || ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'z')) {
                e.preventDefault();
                redo();
            }

            // Open: Ctrl + O
            if ((e.ctrlKey || e.metaKey) && e.key === 'o') {
                e.preventDefault();
                document.getElementById('file-upload')?.click();
            }

            // Help: ?
            if (e.key === '?' || ((e.shiftKey) && e.key === '?')) {
                e.preventDefault();
                onShowHelp?.();
            }

            // Navigation
            if (e.key === 'PageUp' || e.key === 'ArrowLeft') {
                if (currentPage > 0) setCurrentPage(currentPage - 1);
            }
            if (e.key === 'PageDown' || e.key === 'ArrowRight') {
                if (currentPage < pageCount - 1) setCurrentPage(currentPage + 1);
            }

            // Zoom
            if ((e.ctrlKey || e.metaKey) && (e.key === '=' || e.key === '+')) {
                e.preventDefault();
                setZoom(zoom + 0.1);
            }
            if ((e.ctrlKey || e.metaKey) && e.key === '-') {
                e.preventDefault();
                setZoom(Math.max(0.1, zoom - 0.1));
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [saveChanges, undo, redo, currentPage, pageCount, setCurrentPage, setZoom, zoom, onShowHelp]);
}
