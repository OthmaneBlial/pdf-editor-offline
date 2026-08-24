import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import axe from 'axe-core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AccessibilityInspector from '../src/components/tools/AccessibilityInspector';

const mocks = vi.hoisted(() => ({
  sessionId: '',
  documentMutationVersion: 0,
  get: vi.fn(),
}));

vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => ({
    sessionId: mocks.sessionId,
    documentMutationVersion: mocks.documentMutationVersion,
  }),
}));

vi.mock('axios', () => ({
  default: {
    get: mocks.get,
    create: vi.fn(() => ({})),
    defaults: { headers: { common: {} } },
    interceptors: { response: { use: vi.fn() } },
  },
}));

const report = {
  audit_sha256: 'a'.repeat(64),
  automated_remediation: false,
  pdf_ua_conformance_claim: false,
  summary: {
    status: 'needs_attention',
    total_pages: 2,
    pages_scanned: 2,
    partial: false,
    checks_passed: 1,
    checks_needing_attention: 1,
    checks_requiring_manual_review: 1,
    high_priority_issues: 1,
  },
  inventory: {
    language: { present: true, valid_format: true, value: 'en-US' },
    tags: { present: true, elements: 4 },
    forms: { fields: 1, labeled_fields: 0, unlabeled_fields: 1 },
    images: { page_images: 0, tagged_figures: 0, figures_with_alt_text: 0 },
  },
  checks: [
    {
      id: 'document-language',
      title: 'Document language',
      status: 'pass',
      severity: 'high',
      summary: 'A valid document language is declared.',
      count: 0,
      page_hints: [],
      guidance: ['Keep the language declaration current.'],
    },
    {
      id: 'reading-order',
      title: 'Reading order',
      status: 'manual_review',
      severity: 'high',
      summary: 'Reading order requires human review.',
      count: 1,
      page_hints: [2],
      guidance: ['Test with a screen reader.'],
    },
    {
      id: 'form-labels',
      title: 'Form labels',
      status: 'needs_attention',
      severity: 'high',
      summary: 'Some form fields lack alternate labels.',
      count: 1,
      page_hints: [1],
      guidance: ['Give every field an accessible label.'],
    },
  ],
};

describe('AccessibilityInspector', () => {
  beforeEach(() => {
    mocks.sessionId = '';
    mocks.documentMutationVersion = 0;
    mocks.get.mockReset();
  });

  it('explains that a PDF must be opened without implying a cloud upload', () => {
    render(<AccessibilityInspector />);

    expect(screen.getByRole('heading', { name: 'Open a PDF first' })).toBeInTheDocument();
    expect(screen.getByText(/never sends the file/i)).toBeInTheDocument();
  });

  it('renders evidence, manual guidance, page hints, and preservation warnings', async () => {
    mocks.sessionId = 'session-1';
    mocks.get.mockResolvedValue({ data: { data: report } });
    const { container } = render(<AccessibilityInspector />);

    fireEvent.click(screen.getByRole('button', { name: 'Inspect accessibility' }));

    await waitFor(() => expect(screen.getByText('This PDF already has tagged accessibility semantics')).toBeInTheDocument());
    expect(mocks.get).toHaveBeenCalledWith(expect.stringContaining('/api/documents/session-1/accessibility'));
    expect(screen.getByRole('heading', { name: 'What the file can — and cannot — prove' })).toBeInTheDocument();
    expect(screen.getByText('Review page: 2')).toBeInTheDocument();
    expect(screen.getByText('Give every field an accessible label.')).toBeInTheDocument();
    expect(screen.getByText(/does not rewrite tags/i)).toBeInTheDocument();

    const results = await axe.run(container, {
      runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag22aa'] },
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });

  it('fails safely without claiming the document changed', async () => {
    mocks.sessionId = 'session-2';
    mocks.get.mockRejectedValue(new Error('local failure'));
    render(<AccessibilityInspector />);

    fireEvent.click(screen.getByRole('button', { name: 'Inspect accessibility' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Your document was not changed');
  });
});
