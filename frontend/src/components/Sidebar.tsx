import { useState } from 'react';
import {
  ChevronDown,
  ChevronRight,
  Command,
  FileText,
  Fullscreen,
  History,
  Keyboard,
  MessageSquare,
  Search,
  Wrench,
  X,
} from 'lucide-react';

import type { ViewMode } from '../lib/workflowCatalog';
import {
  PRIMARY_WORKFLOWS,
  SECONDARY_COMMANDS,
} from '../lib/workflowCatalog';
import CollaborativeAnnotations from './CollaborativeAnnotations';
import FileUpload from './FileUpload';
import FullscreenButton from './FullscreenButton';
import HistoryPanel from './HistoryPanel';
import ImageUpload from './ImageUpload';
import RecentFiles from './RecentFiles';
import Toolbar from './Toolbar';

interface SidebarProps {
  activeView: ViewMode;
  onViewChange: (view: ViewMode) => void;
  onShowShortcuts: () => void;
  onOpenCommandPalette: () => void;
  isMobileOpen?: boolean;
  onMobileClose?: () => void;
}

type UtilityPanel = 'tools' | 'history' | 'comments';

const utilityItems: Array<{
  id: UtilityPanel;
  label: string;
  icon: typeof Wrench;
}> = [
  { id: 'tools', label: 'Canvas tools', icon: Wrench },
  { id: 'history', label: 'History', icon: History },
  { id: 'comments', label: 'Local comments', icon: MessageSquare },
];

const Sidebar: React.FC<SidebarProps> = ({
  activeView,
  onViewChange,
  onShowShortcuts,
  onOpenCommandPalette,
  isMobileOpen = false,
  onMobileClose,
}) => {
  const [allToolsOpen, setAllToolsOpen] = useState(false);
  const [utilityPanel, setUtilityPanel] = useState<UtilityPanel | null>(null);

  const handleViewSelection = (view: ViewMode) => {
    onViewChange(view);
    onMobileClose?.();
  };

  const renderUtility = () => {
    if (utilityPanel === 'tools') {
      return <div className="space-y-4"><Toolbar /><ImageUpload /></div>;
    }
    if (utilityPanel === 'history') return <HistoryPanel />;
    if (utilityPanel === 'comments') return <CollaborativeAnnotations />;
    return null;
  };

  return (
    <>
      <div
        className={`fixed inset-0 z-30 bg-slate-950/75 backdrop-blur-sm transition-opacity duration-300 lg:hidden ${isMobileOpen ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'}`}
        onClick={onMobileClose}
        aria-hidden="true"
      />
      <aside
        className={`fixed inset-y-0 left-0 z-40 flex w-[88vw] max-w-[320px] flex-col overflow-hidden border-r border-white/[.07] bg-[#090d18] text-white shadow-2xl transition-transform duration-300 lg:relative lg:z-10 lg:w-72 lg:translate-x-0 ${isMobileOpen ? 'translate-x-0' : '-translate-x-full'}`}
        aria-label="Main sidebar"
      >
        <div className="border-b border-white/[.07] p-4">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-cyan-300 text-slate-950 shadow-[0_0_28px_rgba(103,232,249,.2)]">
              <FileText className="h-5 w-5" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <h1 className="truncate text-base font-black tracking-tight">PDF Editor Offline</h1>
              <p className="mt-0.5 font-mono text-[9px] uppercase tracking-[.18em] text-slate-400">Private workbench · v2.1.0</p>
            </div>
            <button
              type="button"
              onClick={onMobileClose}
              className="touch-target inline-flex h-11 w-11 items-center justify-center rounded-xl text-slate-400 hover:bg-white/5 hover:text-white lg:hidden"
              aria-label="Close sidebar menu"
            >
              <X className="h-5 w-5" aria-hidden="true" />
            </button>
          </div>

          <button
            type="button"
            onClick={onOpenCommandPalette}
            className="touch-target mt-4 flex min-h-12 w-full items-center gap-3 rounded-2xl border border-cyan-300/20 bg-cyan-300/[.065] px-3 text-left text-slate-200 transition hover:border-cyan-300/40 hover:bg-cyan-300/10"
            aria-label="Search all workflows and tools"
          >
            <Search className="h-4 w-4 text-cyan-300" aria-hidden="true" />
            <span className="min-w-0 flex-1 text-xs font-bold">Find a workflow or tool</span>
            <kbd className="flex items-center gap-1 rounded-lg border border-white/10 bg-black/20 px-2 py-1 font-mono text-[9px] text-slate-400"><Command className="h-2.5 w-2.5" aria-hidden="true" />K</kbd>
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-3 py-4">
          <nav aria-label="Primary workflows">
            <div className="mb-2 flex items-center justify-between px-2">
              <p className="text-[9px] font-black uppercase tracking-[.22em] text-slate-400">Five primary jobs</p>
              <span className="rounded-full bg-emerald-300/10 px-2 py-1 font-mono text-[8px] uppercase text-emerald-300">On-device</span>
            </div>
            <div className="space-y-1.5">
              {PRIMARY_WORKFLOWS.map((item, index) => {
                const Icon = item.icon;
                const active = activeView === item.id;
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => handleViewSelection(item.id)}
                    aria-current={active ? 'page' : undefined}
                    className={`group flex min-h-14 w-full items-center gap-3 rounded-2xl border px-3 py-2.5 text-left transition ${active ? 'border-cyan-300/45 bg-white/[.075] shadow-[inset_3px_0_0_#67e8f9]' : 'border-transparent text-slate-400 hover:border-white/10 hover:bg-white/[.035] hover:text-white'}`}
                  >
                    <span className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br ${item.accent} text-slate-950 shadow-sm`}>
                      <Icon className="h-5 w-5" aria-hidden="true" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block font-display text-[13px] font-bold">{item.label}</span>
                      <span className="mt-0.5 block truncate font-mono text-[8px] uppercase tracking-wider text-slate-400">0{index + 1} · {item.keywords[0]}</span>
                    </span>
                    <ChevronRight className={`h-4 w-4 shrink-0 transition ${active ? 'text-cyan-300' : 'text-slate-700 group-hover:translate-x-0.5 group-hover:text-slate-400'}`} aria-hidden="true" />
                  </button>
                );
              })}
            </div>
          </nav>

          <section className="mt-5 border-t border-white/[.07] pt-3" aria-labelledby="all-tools-heading">
            <button
              type="button"
              onClick={() => setAllToolsOpen(open => !open)}
              aria-expanded={allToolsOpen}
              aria-controls="all-tools-panel"
              className="touch-target flex min-h-11 w-full items-center gap-3 rounded-xl px-2 text-left text-xs font-bold text-slate-400 transition hover:bg-white/[.035] hover:text-white"
            >
              <Wrench className="h-4 w-4" aria-hidden="true" />
              <span id="all-tools-heading" className="flex-1">All tools</span>
              <span className="font-mono text-[8px] text-slate-400">{SECONDARY_COMMANDS.length}</span>
              <ChevronDown className={`h-4 w-4 transition-transform ${allToolsOpen ? 'rotate-180' : ''}`} aria-hidden="true" />
            </button>
            {allToolsOpen && (
              <div id="all-tools-panel" className="mt-1 space-y-1 animate-fade-in">
                {SECONDARY_COMMANDS.map(item => {
                  const Icon = item.icon;
                  const active = activeView === item.id;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => handleViewSelection(item.id)}
                      aria-current={active ? 'page' : undefined}
                      className={`touch-target flex min-h-11 w-full items-center gap-3 rounded-xl px-3 text-left text-xs transition ${active ? 'bg-cyan-300 text-slate-950' : 'text-slate-400 hover:bg-white/[.035] hover:text-slate-200'}`}
                    >
                      <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
                      <span className="flex-1 font-semibold">{item.shortLabel}</span>
                      {active && <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />}
                    </button>
                  );
                })}
              </div>
            )}
          </section>

          <section className="mt-3 border-t border-white/[.07] pt-3" aria-label="Workspace utilities">
            <p className="px-2 pb-1 text-[9px] font-black uppercase tracking-[.22em] text-slate-400">Workspace utilities</p>
            {utilityItems.map(item => {
              const Icon = item.icon;
              const expanded = utilityPanel === item.id;
              return (
                <div key={item.id}>
                  <button
                    type="button"
                    onClick={() => setUtilityPanel(current => current === item.id ? null : item.id)}
                    aria-expanded={expanded}
                    aria-controls={`utility-${item.id}`}
                    className="touch-target flex min-h-11 w-full items-center gap-3 rounded-xl px-2 text-left text-xs font-semibold text-slate-400 transition hover:bg-white/[.035] hover:text-slate-200"
                  >
                    <Icon className="h-4 w-4" aria-hidden="true" />
                    <span className="flex-1">{item.label}</span>
                    <ChevronDown className={`h-3.5 w-3.5 transition-transform ${expanded ? 'rotate-180' : ''}`} aria-hidden="true" />
                  </button>
                  {expanded && (
                    <div id={`utility-${item.id}`} className="mb-2 rounded-xl border border-white/[.07] bg-black/20 p-3 animate-fade-in">
                      {renderUtility()}
                    </div>
                  )}
                </div>
              );
            })}
          </section>
        </div>

        <div className="border-t border-white/[.07] bg-black/15 p-3">
          <FileUpload compact />
          <div className="mt-2"><RecentFiles /></div>
          <div className="mt-2 flex min-h-11 items-center justify-between rounded-xl bg-white/[.035] px-2">
            <div className="flex items-center gap-2 px-1">
              <span className="h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,.6)]" aria-hidden="true" />
              <span className="font-mono text-[9px] uppercase tracking-wider text-slate-400">Ready locally</span>
            </div>
            <div className="flex items-center gap-1">
              <span className="sr-only"><Fullscreen aria-hidden="true" />Display controls</span>
              <FullscreenButton />
              <button
                type="button"
                onClick={onShowShortcuts}
                className="touch-target inline-flex h-11 w-11 items-center justify-center rounded-xl text-slate-500 transition hover:bg-white/5 hover:text-white"
                title="Keyboard shortcuts"
                aria-label="Show keyboard shortcuts"
              >
                <Keyboard className="h-4 w-4" aria-hidden="true" />
              </button>
            </div>
          </div>
        </div>
      </aside>
    </>
  );
};

export default Sidebar;
