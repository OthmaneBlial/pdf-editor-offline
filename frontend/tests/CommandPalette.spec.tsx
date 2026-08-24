import React from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import CommandPalette from '../src/components/CommandPalette';

const renderPalette = (overrides: Partial<React.ComponentProps<typeof CommandPalette>> = {}) => {
  const props = {
    open: true,
    activeView: 'redact' as const,
    onClose: vi.fn(),
    onSelect: vi.fn(),
    ...overrides,
  };
  return { props, ...render(<CommandPalette {...props} />) };
};

describe('CommandPalette', () => {
  it('focuses search and groups every local command in an accessible dialog', async () => {
    renderPalette();

    expect(screen.getByRole('dialog', { name: 'Go straight to the job' })).toBeInTheDocument();
    const search = screen.getByRole('combobox', { name: 'Search commands' });
    await waitFor(() => expect(search).toHaveFocus());
    expect(screen.getByRole('listbox', { name: 'Command results' })).toBeInTheDocument();
    expect(screen.getByText('Primary workflows')).toBeInTheDocument();
  });

  it('finds merge through workflow keywords and opens Organize Pages with Enter', () => {
    const { props } = renderPalette();
    const search = screen.getByRole('combobox', { name: 'Search commands' });

    fireEvent.change(search, { target: { value: 'merge' } });
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(within(screen.getByRole('option')).getByText('Organize Pages')).toBeInTheDocument();
    fireEvent.keyDown(search, { key: 'Enter' });

    expect(props.onSelect).toHaveBeenCalledWith('manipulation');
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it('supports arrow navigation, empty results, Escape, and a trapped Tab loop', async () => {
    const { props } = renderPalette();
    const dialog = screen.getByRole('dialog');
    const search = screen.getByRole('combobox', { name: 'Search commands' });
    await waitFor(() => expect(search).toHaveFocus());

    fireEvent.keyDown(dialog, { key: 'ArrowDown' });
    expect(screen.getAllByRole('option')[1]).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(dialog, { key: 'ArrowUp' });
    expect(screen.getAllByRole('option')[0]).toHaveAttribute('aria-selected', 'true');

    fireEvent.change(search, { target: { value: 'nothing-matches-this' } });
    expect(screen.getByText('No local command matches')).toBeInTheDocument();

    fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(screen.getByRole('button', { name: 'Close command palette' })).toHaveFocus();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });
});
