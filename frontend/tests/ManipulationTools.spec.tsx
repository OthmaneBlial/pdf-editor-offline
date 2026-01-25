import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import ManipulationTools from '../src/components/tools/ManipulationTools';
import '@testing-library/jest-dom';

// Mock axios
vi.mock('axios');

describe('ManipulationTools', () => {
    it('renders manipulation options', () => {
        render(<ManipulationTools />);
        expect(screen.getByText('Manipulation Tools')).toBeInTheDocument();
        expect(screen.getAllByText('Merge PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Split PDF').length).toBeGreaterThan(0);
    });

    it('renders all manipulation cards', () => {
        render(<ManipulationTools />);

        expect(screen.getAllByText('Merge PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Split PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Organize PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Rotate PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Page Numbers').length).toBeGreaterThan(0);
    });
});
