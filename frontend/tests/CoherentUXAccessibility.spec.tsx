import React from 'react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import axe from 'axe-core';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import CommandPalette from '../src/components/CommandPalette';
import ExpertDisclosure from '../src/components/ExpertDisclosure';
import WorkflowFeedback from '../src/components/WorkflowFeedback';

describe('Coherent UX accessibility contract', () => {
  it('has no detectable WCAG A/AA semantic violation in shared workflow surfaces', async () => {
    const { container } = render(
      <main>
        <ExpertDisclosure title="Expert controls" summary="Optional quality settings">
          <label>Quality<input /></label>
        </ExpertDisclosure>
        <WorkflowFeedback tone="warning" title="Review required">One feature may change.</WorkflowFeedback>
        <CommandPalette open activeView="redact" onClose={() => undefined} onSelect={() => undefined} />
      </main>,
    );

    const results = await axe.run(container, {
      runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
      rules: { 'color-contrast': { enabled: false } },
    });

    expect(results.violations).toEqual([]);
  });

  it('ships visible focus, reduced-motion, forced-color, reflow, and 44px target rules', () => {
    const cssPath = resolve(process.cwd(), 'src/index.css');
    const css = readFileSync(cssPath, 'utf8');

    expect(css).toContain('.touch-target');
    expect(css).toMatch(/min-height:\s*2\.75rem/);
    expect(css).toContain(':focus-visible');
    expect(css).toContain('@media (prefers-reduced-motion: reduce)');
    expect(css).toContain('@media (forced-colors: active)');
    expect(css).toContain('@media (max-width: 20rem)');
  });
});
