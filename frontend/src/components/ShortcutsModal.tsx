import React, { useEffect, useRef } from 'react';
import { KEYBOARD_SHORTCUTS } from '../hooks/useKeyboardShortcuts';
import { X } from 'lucide-react';

interface ShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const ShortcutsModal: React.FC<ShortcutsModalProps> = ({ isOpen, onClose }) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  // Focus trap implementation
  useEffect(() => {
    if (isOpen) {
      // Focus close button when modal opens
      closeButtonRef.current?.focus();

      const handleTab = (e: KeyboardEvent) => {
        if (e.key !== 'Tab') return;

        const focusableElements = modalRef.current?.querySelectorAll(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (!focusableElements || focusableElements.length === 0) return;

        const firstElement = focusableElements[0] as HTMLElement;
        const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

        if (e.shiftKey) {
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement.focus();
          }
        } else {
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement.focus();
          }
        }
      };

      const handleEscape = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          onClose();
        }
      };

      document.addEventListener('keydown', handleTab);
      document.addEventListener('keydown', handleEscape);

      return () => {
        document.removeEventListener('keydown', handleTab);
        document.removeEventListener('keydown', handleEscape);
      };
    }
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-fade-in"
      onClick={onClose}
      aria-modal="true"
      role="dialog"
      aria-labelledby="modal-title"
    >
      <div
        ref={modalRef}
        className="w-full max-w-md bg-[var(--card-bg)] rounded-2xl shadow-2xl border border-[var(--border-color)] overflow-hidden animate-scale-in"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between p-4 border-b border-[var(--border-color)]">
          <h2 id="modal-title" className="text-lg font-semibold text-[var(--color-primary-600)]">
            Keyboard Shortcuts
          </h2>
          <button
            ref={closeButtonRef}
            onClick={onClose}
            className="p-1 rounded-full hover:bg-[var(--hover-bg)] text-[var(--text-secondary)] transition-colors focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
            aria-label="Close modal"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <ul className="p-4 space-y-3" role="list" aria-label="Keyboard shortcuts list">
          {KEYBOARD_SHORTCUTS.map((shortcut, index) => (
            <li key={index} className="flex items-center justify-between">
              <span className="text-sm text-[var(--text-primary)]">{shortcut.description}</span>
              <div className="flex gap-1" role="group" aria-label={`Keys for ${shortcut.description}`}>
                {shortcut.keys.map((key, i) => (
                  <kbd
                    key={i}
                    className="px-2 py-1 text-xs font-semibold text-[var(--text-secondary)] bg-[var(--bg-primary)] border border-[var(--border-color)] rounded-lg shadow-sm"
                  >
                    {key}
                  </kbd>
                ))}
              </div>
            </li>
          ))}
        </ul>

        <div className="p-4 bg-[var(--bg-primary)]/50 border-t border-[var(--border-color)] text-center">
          <p className="text-xs text-[var(--text-secondary)]">
            Press <kbd className="font-bold">?</kbd> to open this menu anytime
          </p>
        </div>
      </div>
    </div>
  );
};

export default ShortcutsModal;
