import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ThemeToggle from '../components/ThemeToggle';
import { ThemeProvider } from '../contexts/ThemeContext';

const renderWithTheme = (ui: React.ReactElement) => {
    return render(
        <ThemeProvider>
            {ui}
        </ThemeProvider>
    );
};

describe('ThemeToggle Component', () => {
    beforeEach(() => {
        // Reset localStorage before each test
        localStorage.clear();
    });

    it('renders the toggle button', () => {
        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');
        expect(button).toBeInTheDocument();
    });

    it('has proper aria-label', () => {
        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');
        expect(button).toHaveAttribute('aria-label');
    });

    it('starts with light mode by default', () => {
        renderWithTheme(<ThemeToggle />);

        // In light mode, moon icon should be shown
        const button = screen.getByRole('button');
        expect(button.getAttribute('aria-label')).toContain('dark');
    });

    it('toggles theme when clicked', () => {
        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');

        // Should be light mode initially
        expect(button.getAttribute('aria-label')).toContain('dark');

        // Click to toggle
        fireEvent.click(button);

        // Should now be dark mode
        expect(button.getAttribute('aria-label')).toContain('light');
    });

    it('has title attribute for tooltip', () => {
        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');
        expect(button).toHaveAttribute('title');
    });
});

describe('ThemeToggle Persistence', () => {
    beforeEach(() => {
        localStorage.clear();
    });

    it('persists theme preference to localStorage', () => {
        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');
        fireEvent.click(button);

        expect(localStorage.getItem('theme')).toBe('dark');
    });

    it('loads theme from localStorage on mount', () => {
        localStorage.setItem('theme', 'dark');

        renderWithTheme(<ThemeToggle />);

        const button = screen.getByRole('button');
        // In dark mode, aria-label should mention switching to light
        expect(button.getAttribute('aria-label')).toContain('light');
    });
});
