import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import ExpertDisclosure from '../src/components/ExpertDisclosure';
import WorkflowFeedback from '../src/components/WorkflowFeedback';

describe('Coherent UX primitives', () => {
  it('keeps expert controls collapsed behind an explanatory native disclosure', () => {
    render(<ExpertDisclosure title="Expert controls" summary="Optional quality settings"><label>Quality<input /></label></ExpertDisclosure>);
    const trigger = screen.getByText('Expert controls').closest('summary');
    const details = trigger?.closest('details');

    expect(details).not.toHaveAttribute('open');
    fireEvent.click(trigger!);
    expect(details).toHaveAttribute('open');
    expect(screen.getByLabelText('Quality')).toBeInTheDocument();
  });

  it('announces errors urgently and exposes deterministic progress semantics', () => {
    const { rerender } = render(<WorkflowFeedback tone="error" title="Could not export">Try again.</WorkflowFeedback>);
    expect(screen.getByRole('alert')).toHaveTextContent('Try again.');

    rerender(<WorkflowFeedback tone="progress" progress={37} progressLabel="OCR progress">Processing locally.</WorkflowFeedback>);
    expect(screen.getByRole('status')).toHaveTextContent('Processing locally.');
    expect(screen.getByRole('progressbar', { name: 'OCR progress' })).toHaveAttribute('aria-valuenow', '37');
  });
});
