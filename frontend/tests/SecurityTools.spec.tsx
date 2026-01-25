import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import SecurityTools from '../src/components/tools/SecurityTools';
import '@testing-library/jest-dom';

vi.mock('axios');

describe('SecurityTools', () => {
    it('renders security options', () => {
        render(<SecurityTools />);
        expect(screen.getByText('Security Tools')).toBeInTheDocument();
        expect(screen.getAllByText('Protect PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Unlock PDF').length).toBeGreaterThan(0);
    });

    it('switches tabs', () => {
        render(<SecurityTools />);

        // Default might be Protect
        // expect(screen.getByText('Add Password', { exact: false })).toBeInTheDocument();

        // Switch to Unlock
        // Note: The button text is "Unlock PDF" but the header inside the card is also "Unlock PDF"
        // We need to be careful which one we click.
        // Let's find the card header or button specifically if needed.
        // But here we just want to verify the form content changes.
        // Actually, SecurityTools renders all cards at once in a grid, not tabs!
        // My previous assumption was wrong based on the code I read.
        // It renders a grid of cards: Protect, Unlock, Sign, Watermark.

        // So we should just check if all exist.
        expect(screen.getAllByText('Protect PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Unlock PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Sign PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Watermark PDF').length).toBeGreaterThan(0);
    });
});
