import { useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, Command, FileSearch, Search, X } from 'lucide-react';

import {
  ALL_COMMANDS,
  commandGroupLabel,
  commandSearchText,
  type CommandGroup,
  type ViewMode,
} from '../lib/workflowCatalog';

interface CommandPaletteProps {
  open: boolean;
  activeView: ViewMode;
  onClose: () => void;
  onSelect: (view: ViewMode) => void;
}

const GROUP_ORDER: CommandGroup[] = ['workflow', 'workspace', 'specialist'];

export default function CommandPalette({
  open,
  activeView,
  onClose,
  onSelect,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);

  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return ALL_COMMANDS;
    return ALL_COMMANDS.filter(command => commandSearchText(command).includes(needle));
  }, [query]);

  useEffect(() => {
    if (!open) return;
    previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
    const frame = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => {
      window.cancelAnimationFrame(frame);
      previouslyFocusedRef.current?.focus();
    };
  }, [open]);

  if (!open) return null;

  const safeActiveIndex = Math.min(activeIndex, Math.max(0, filtered.length - 1));

  const close = () => {
    setQuery('');
    setActiveIndex(0);
    onClose();
  };

  const choose = (view: ViewMode) => {
    onSelect(view);
    close();
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
      return;
    }
    if (event.key === 'ArrowDown' && filtered.length) {
      event.preventDefault();
      setActiveIndex(current => (current + 1) % filtered.length);
      return;
    }
    if (event.key === 'ArrowUp' && filtered.length) {
      event.preventDefault();
      setActiveIndex(current => (current - 1 + filtered.length) % filtered.length);
      return;
    }
    if (event.key === 'Enter' && filtered[safeActiveIndex]) {
      event.preventDefault();
      choose(filtered[safeActiveIndex].id);
      return;
    }
    if (event.key === 'Tab') {
      const focusable = [inputRef.current, closeRef.current].filter(Boolean) as HTMLElement[];
      if (!focusable.length) return;
      const current = focusable.indexOf(document.activeElement as HTMLElement);
      const next = event.shiftKey
        ? (current - 1 + focusable.length) % focusable.length
        : (current + 1) % focusable.length;
      event.preventDefault();
      focusable[next]?.focus();
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center bg-slate-950/80 px-3 pt-[8vh] backdrop-blur-md sm:px-6 sm:pt-[12vh]"
      role="presentation"
      onMouseDown={event => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="command-palette-title"
        aria-describedby="command-palette-description"
        onKeyDown={handleKeyDown}
        className="w-full max-w-3xl overflow-hidden rounded-[1.75rem] border border-cyan-300/25 bg-[#090f1f] text-white shadow-[0_28px_100px_rgba(0,0,0,.62)]"
      >
        <div className="border-b border-white/10 p-4 sm:p-5">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-cyan-300 text-slate-950">
              <Command className="h-5 w-5" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <h2 id="command-palette-title" className="text-base font-bold sm:text-lg">Go straight to the job</h2>
              <p id="command-palette-description" className="text-[11px] text-slate-300 sm:text-xs">Search workflows and specialist tools. Arrow keys move; Enter opens.</p>
            </div>
            <button
              ref={closeRef}
              type="button"
              onClick={close}
              className="touch-target inline-flex h-11 w-11 items-center justify-center rounded-xl border border-white/10 text-slate-400 transition hover:border-white/25 hover:text-white"
              aria-label="Close command palette"
            >
              <X className="h-5 w-5" aria-hidden="true" />
            </button>
          </div>
          <label className="mt-4 flex min-h-12 items-center gap-3 rounded-2xl border border-white/15 bg-white/[.055] px-4 focus-within:border-cyan-300 focus-within:ring-2 focus-within:ring-cyan-300/25">
            <Search className="h-5 w-5 shrink-0 text-cyan-300" aria-hidden="true" />
            <span className="sr-only">Search commands</span>
            <input
              ref={inputRef}
              value={query}
              onChange={event => {
                setQuery(event.target.value);
                setActiveIndex(0);
              }}
              role="combobox"
              aria-label="Search commands"
              aria-expanded="true"
              aria-controls="command-results"
              aria-activedescendant={filtered[safeActiveIndex] ? `command-${filtered[safeActiveIndex].id}` : undefined}
              autoComplete="off"
              placeholder="Try “redact”, “merge”, “certificate”, “OCR”…"
              className="min-w-0 flex-1 bg-transparent py-3 text-sm text-white outline-none placeholder:text-slate-400"
            />
            <kbd className="hidden rounded-lg border border-white/10 bg-black/25 px-2 py-1 font-mono text-[10px] text-slate-400 sm:block">ESC</kbd>
          </label>
        </div>

        <div id="command-results" role="listbox" aria-label="Command results" className="max-h-[58vh] overflow-y-auto p-2 sm:p-3">
          {!filtered.length && (
            <div className="px-4 py-12 text-center">
              <FileSearch className="mx-auto h-8 w-8 text-slate-600" aria-hidden="true" />
              <p className="mt-3 font-bold text-slate-200">No local command matches</p>
              <p className="mt-1 text-xs text-slate-400">Try a workflow, file operation, or privacy term.</p>
            </div>
          )}
          {GROUP_ORDER.map(group => {
            const commands = filtered.filter(command => command.group === group);
            if (!commands.length) return null;
            return (
              <section key={group} role="group" aria-label={commandGroupLabel(group)} className="mb-3 last:mb-0">
                <p className="px-3 pb-1 pt-2 text-[9px] font-black uppercase tracking-[.22em] text-slate-400">{commandGroupLabel(group)}</p>
                <div className="space-y-1">
                  {commands.map(command => {
                    const globalIndex = filtered.indexOf(command);
                    const Icon = command.icon;
                    const selected = globalIndex === safeActiveIndex;
                    const current = command.id === activeView;
                    return (
                      <button
                        key={command.id}
                        id={`command-${command.id}`}
                        type="button"
                        role="option"
                        aria-selected={selected}
                        onMouseEnter={() => setActiveIndex(globalIndex)}
                        onClick={() => choose(command.id)}
                        className={`group flex min-h-14 w-full items-center gap-3 rounded-2xl border px-3 py-2.5 text-left transition sm:px-4 ${selected ? 'border-cyan-300/45 bg-cyan-300/10' : 'border-transparent hover:bg-white/[.045]'}`}
                      >
                        <span className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br ${command.accent} text-slate-950 shadow-sm`}>
                          <Icon className="h-5 w-5" aria-hidden="true" />
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="flex items-center gap-2 font-display text-sm font-bold text-white">
                            {command.label}
                            {current && <span className="rounded-full bg-white/10 px-2 py-0.5 font-mono text-[8px] uppercase tracking-wider text-cyan-200">Open</span>}
                          </span>
                          <span className="mt-0.5 block truncate text-[10px] text-slate-400 sm:text-xs">{command.description}</span>
                        </span>
                        <ArrowRight className={`h-4 w-4 shrink-0 transition ${selected ? 'translate-x-0 text-cyan-200' : '-translate-x-1 text-slate-700 group-hover:translate-x-0 group-hover:text-slate-400'}`} aria-hidden="true" />
                      </button>
                    );
                  })}
                </div>
              </section>
            );
          })}
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2 border-t border-white/10 bg-black/20 px-4 py-3 text-[9px] uppercase tracking-[.12em] text-slate-400 sm:px-5">
          <span>Local navigation only</span>
          <span>{filtered.length} command{filtered.length === 1 ? '' : 's'}</span>
        </div>
      </div>
    </div>
  );
}
