import { Moon, Sun } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';

export default function ThemeToggle() {
    const { theme, toggleTheme } = useTheme();

    return (
        <button
            onClick={toggleTheme}
            className="relative p-2.5 rounded-lg bg-[var(--bg-interactive)] hover:bg-[var(--bg-interactive-hover)] border border-[var(--border-subtle)] transition-all duration-200 group"
            aria-label={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
            title={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
        >
            {theme === 'light' ? (
                <Moon className="w-4 h-4 text-[var(--text-secondary)] group-hover:text-[var(--accent-tertiary)] transition-colors" />
            ) : (
                <Sun className="w-4 h-4 text-[var(--accent-primary)] group-hover:text-[var(--accent-primary-dim)] transition-colors" />
            )}
            {/* Glow effect in dark mode */}
            {theme === 'dark' && (
                <div className="absolute inset-0 rounded-lg bg-[var(--accent-primary)]/10 opacity-0 group-hover:opacity-100 transition-opacity" />
            )}
        </button>
    );
}
