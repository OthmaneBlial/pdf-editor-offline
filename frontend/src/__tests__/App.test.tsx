import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ThemeProvider } from '../contexts/ThemeContext';
import App from '../App';

// Mock child components to isolate App testing
vi.mock('../components/FileUpload', () => ({
    default: () => <div data-testid="file-upload">FileUpload</div>
}));
vi.mock('../components/PDFViewer', () => ({
    default: () => <div data-testid="pdf-viewer">PDFViewer</div>
}));
vi.mock('../components/Header', () => ({
    default: () => <div data-testid="header">Header</div>
}));
vi.mock('../components/Toolbar', () => ({
    default: () => <div data-testid="toolbar">Toolbar</div>
}));
vi.mock('../components/tools/ManipulationTools', () => ({
    default: () => <div data-testid="manipulation-tools">ManipulationTools</div>
}));
vi.mock('../components/tools/ConversionTools', () => ({
    default: () => <div data-testid="conversion-tools">ConversionTools</div>
}));
vi.mock('../components/tools/SecurityTools', () => ({
    default: () => <div data-testid="security-tools">SecurityTools</div>
}));
vi.mock('../components/tools/AdvancedTools', () => ({
    default: () => <div data-testid="advanced-tools">AdvancedTools</div>
}));

const renderWithProviders = (ui: React.ReactElement) => {
    return render(
        <ThemeProvider>
            {ui}
        </ThemeProvider>
    );
};

describe('App Component', () => {
    it('renders the main application layout', () => {
        renderWithProviders(<App />);

        // Check header is rendered
        expect(screen.getByTestId('header')).toBeInTheDocument();
    });

    it('renders the sidebar with navigation', () => {
        renderWithProviders(<App />);

        // Check navigation buttons are present
        expect(screen.getByText('Editor')).toBeInTheDocument();
        expect(screen.getByText('Manipulation')).toBeInTheDocument();
        expect(screen.getByText('Conversion')).toBeInTheDocument();
        expect(screen.getByText('Security')).toBeInTheDocument();
        expect(screen.getByText('Advanced')).toBeInTheDocument();
    });

    it('renders the version number v2.0.0', () => {
        renderWithProviders(<App />);

        expect(screen.getByText('v2.0.0')).toBeInTheDocument();
    });

    it('renders the logo section', () => {
        renderWithProviders(<App />);

        expect(screen.getByText('PDF Smart Editor')).toBeInTheDocument();
    });

    it('renders the PRO badge', () => {
        renderWithProviders(<App />);

        expect(screen.getByText('PRO')).toBeInTheDocument();
    });

    it('renders status indicator', () => {
        renderWithProviders(<App />);

        expect(screen.getByText('All systems operational')).toBeInTheDocument();
    });

    it('renders Premium label in footer', () => {
        renderWithProviders(<App />);

        expect(screen.getByText('Premium')).toBeInTheDocument();
    });

    it('renders file upload button', () => {
        renderWithProviders(<App />);

        expect(screen.getByTestId('file-upload')).toBeInTheDocument();
    });

    it('defaults to editor view', () => {
        renderWithProviders(<App />);

        // PDFViewer should be rendered in editor view
        expect(screen.getByTestId('pdf-viewer')).toBeInTheDocument();
    });
});

describe('App Navigation', () => {
    it('switches to manipulation view when clicked', async () => {
        renderWithProviders(<App />);

        const manipulationButton = screen.getByText('Manipulation');
        manipulationButton.click();

        expect(screen.getByTestId('manipulation-tools')).toBeInTheDocument();
    });

    it('switches to conversion view when clicked', async () => {
        renderWithProviders(<App />);

        const conversionButton = screen.getByText('Conversion');
        conversionButton.click();

        expect(screen.getByTestId('conversion-tools')).toBeInTheDocument();
    });

    it('switches to security view when clicked', async () => {
        renderWithProviders(<App />);

        const securityButton = screen.getByText('Security');
        securityButton.click();

        expect(screen.getByTestId('security-tools')).toBeInTheDocument();
    });

    it('switches to advanced view when clicked', async () => {
        renderWithProviders(<App />);

        const advancedButton = screen.getByText('Advanced');
        advancedButton.click();

        expect(screen.getByTestId('advanced-tools')).toBeInTheDocument();
    });

    it('returns to editor view when clicked', async () => {
        renderWithProviders(<App />);

        // First go to manipulation
        screen.getByText('Manipulation').click();
        expect(screen.getByTestId('manipulation-tools')).toBeInTheDocument();

        // Then back to editor
        screen.getByText('Editor').click();
        expect(screen.getByTestId('pdf-viewer')).toBeInTheDocument();
    });
});

describe('App Accessibility', () => {
    it('has accessible navigation buttons', () => {
        renderWithProviders(<App />);

        const buttons = screen.getAllByRole('button');
        expect(buttons.length).toBeGreaterThan(0);
    });

    it('has proper heading structure', () => {
        renderWithProviders(<App />);

        const heading = screen.getByText('PDF Smart Editor');
        expect(heading.tagName).toBe('H1');
    });
});
