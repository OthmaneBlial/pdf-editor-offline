import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import AdvancedTools from '../src/components/tools/AdvancedTools';
import '@testing-library/jest-dom';

vi.mock('axios');

describe('AdvancedTools', () => {
    it('renders advanced options', () => {
        render(<AdvancedTools />);
        expect(screen.getByText('Advanced Tools')).toBeInTheDocument();
        expect(screen.getAllByText('Repair PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Compare PDF').length).toBeGreaterThan(0);
    });

    it('renders all advanced cards', () => {
        render(<AdvancedTools />);

        expect(screen.getAllByText('Compress PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Repair PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Scan to PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('PDF OCR').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Compare PDF').length).toBeGreaterThan(0);
    });
});
