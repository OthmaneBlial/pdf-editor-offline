import { beforeEach, describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import axios from 'axios';
import AdvancedTools from '../src/components/tools/AdvancedTools';
import '@testing-library/jest-dom';

vi.mock('axios');

describe('AdvancedTools', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders advanced options', () => {
        render(<AdvancedTools />);
        expect(screen.getByText('Advanced Tools')).toBeInTheDocument();
        expect(screen.getAllByText('Repair PDF').length).toBeGreaterThan(0);
        expect(screen.getByText('Visual + semantic review')).toBeInTheDocument();
    });

    it('renders all advanced cards', () => {
        render(<AdvancedTools />);

        expect(screen.getAllByText('Compress PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Repair PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('Scan to PDF').length).toBeGreaterThan(0);
        expect(screen.getAllByText('PDF OCR').length).toBeGreaterThan(0);
        expect(screen.getByText('Visual + semantic review')).toBeInTheDocument();
    });

    it('shows before, after, overlay, semantic counts, and the audit verdict', async () => {
        vi.mocked(axios.get).mockResolvedValue({ data: new Blob(['preview'], { type: 'image/png' }) });
        vi.mocked(axios.post).mockResolvedValueOnce({
            data: {
                data: {
                    review_id: 'review-1',
                    expires_in_hours: 24,
                    artifacts: [
                        { name: 'page-001-before.png', media_type: 'image/png', content_bearing: true, url: '/api/tools/change-review/review-1/artifacts/page-001-before.png' },
                        { name: 'page-001-after.png', media_type: 'image/png', content_bearing: true, url: '/api/tools/change-review/review-1/artifacts/page-001-after.png' },
                        { name: 'page-001-overlay.png', media_type: 'image/png', content_bearing: true, url: '/api/tools/change-review/review-1/artifacts/page-001-overlay.png' },
                        { name: 'page-001-text.diff', media_type: 'text/x-diff', content_bearing: true, url: '/api/tools/change-review/review-1/artifacts/page-001-text.diff' },
                    ],
                    report: {
                        verdict: 'changes_detected',
                        audit_sha256: 'a'.repeat(64),
                        safe_to_publish: true,
                        warnings: [],
                        visual: {
                            unexpected_pages: 1,
                            pages: [{
                                page: 1,
                                changed_ratio_outside_expected: 0.02,
                                within_tolerance: false,
                                artifacts: {
                                    before: 'page-001-before.png',
                                    after: 'page-001-after.png',
                                    overlay: 'page-001-overlay.png',
                                },
                            }],
                        },
                        semantic: {
                            changed_text_pages: 1,
                            characters_added: 9,
                            characters_removed: 2,
                            changed_metadata_keys: 1,
                        },
                        objects: { pages_changed: 1 },
                        annotation_history: { added: 1, removed: 0, modified: 0 },
                    },
                },
            },
        });
        const { container } = render(<AdvancedTools />);
        const inputs = container.querySelectorAll<HTMLInputElement>('input[type="file"]');
        const before = new File(['before'], 'before.pdf', { type: 'application/pdf' });
        const after = new File(['after'], 'after.pdf', { type: 'application/pdf' });
        fireEvent.change(inputs[4], { target: { files: [before] } });
        fireEvent.change(inputs[5], { target: { files: [after] } });
        const reviewButton = screen.getByRole('button', { name: /review both pdfs/i });
        fireEvent.submit(reviewButton.closest('form')!);

        expect(await screen.findByText('changes detected')).toBeInTheDocument();
        expect(screen.getByText('Safe to publish')).toBeInTheDocument();
        expect(await screen.findByAltText('Before rendering for page 1')).toBeInTheDocument();
        expect(screen.getByAltText('After rendering for page 1')).toBeInTheDocument();
        expect(screen.getByAltText('Change overlay rendering for page 1')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /page-001-text.diff/i })).toBeInTheDocument();
    });
});
