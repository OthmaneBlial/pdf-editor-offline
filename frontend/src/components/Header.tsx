import { useEditor } from '../contexts/EditorContext';
import { Command, Download, Save, Loader2, PanelLeft, Search, X } from 'lucide-react';
import ThemeToggle from './ThemeToggle';
import RuntimeHealthPanel from './RuntimeHealthPanel';
import RecoveryCenter from './RecoveryCenter';

interface HeaderProps {
    onToggleSidebar?: () => void;
    isSidebarOpen?: boolean;
    activeViewLabel?: string;
    onOpenCommandPalette?: () => void;
}

const Header: React.FC<HeaderProps> = ({ onToggleSidebar, isSidebarOpen = false, activeViewLabel = 'Editor', onOpenCommandPalette }) => {
    const { exportPDF, saveChanges, hasUnsavedChanges, sessionId, isUploading } = useEditor();
    const disabledActions = !sessionId || isUploading;

    return (
        <header className="h-14 bg-[var(--bg-canvas)] border-b border-[var(--border-subtle)] flex items-center justify-between px-3 sm:px-6 relative z-20">
            {/* Left cluster */}
            <div className="flex items-center gap-2 sm:gap-3 min-w-0">
                <button
                    onClick={onToggleSidebar}
                    className="touch-target lg:hidden p-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-inset)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--border-default)] transition-colors"
                    aria-label={isSidebarOpen ? 'Close sidebar menu' : 'Open sidebar menu'}
                >
                    {isSidebarOpen ? <X className="w-4 h-4" /> : <PanelLeft className="w-4 h-4" />}
                </button>

                <div className="lg:hidden text-[11px] font-display font-semibold uppercase tracking-wide text-[var(--text-secondary)]">
                    {activeViewLabel}
                </div>

                {/* Desktop breadcrumb */}
                <div className="hidden lg:flex items-center gap-2 text-sm font-body">
                    <span className="font-display font-semibold text-[var(--accent-primary)]">
                        Workspace
                    </span>
                    <span className="text-[var(--text-muted)]">/</span>
                    <span className="text-[var(--text-secondary)]">
                        {sessionId ? 'Active Document' : 'No Document'}
                    </span>
                    {hasUnsavedChanges && (
                        <span className="ml-2 w-2 h-2 rounded-full bg-[var(--accent-secondary)] animate-pulse" />
                    )}
                </div>
            </div>

            {/* Actions */}
            <div className="flex items-center gap-2 sm:gap-3">
                <button
                    type="button"
                    onClick={onOpenCommandPalette}
                    className="touch-target hidden min-h-11 items-center gap-2 rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-inset)] px-3 text-[var(--text-secondary)] transition hover:border-[var(--accent-primary)] hover:text-[var(--text-primary)] md:flex"
                    aria-label="Open command palette"
                    title="Search workflows and tools"
                >
                    <Search className="h-4 w-4" aria-hidden="true" />
                    <span className="hidden xl:inline font-display text-[11px] font-semibold">Find a tool</span>
                    <kbd className="hidden items-center gap-1 rounded-md border border-[var(--border-subtle)] bg-[var(--bg-canvas)] px-1.5 py-0.5 font-mono text-[9px] text-[var(--text-muted)] lg:flex"><Command className="h-2.5 w-2.5" aria-hidden="true" />K</kbd>
                </button>
                <RuntimeHealthPanel />
                <RecoveryCenter />
                <ThemeToggle />

                {/* Export Button */}
                <button
                    onClick={() => exportPDF()}
                    disabled={disabledActions}
                    className={`
                        touch-target flex min-h-11 items-center gap-2 px-2 sm:px-4 py-2
                        font-display text-xs sm:text-sm font-semibold uppercase tracking-wide
                        rounded-lg border transition-all duration-200
                        ${disabledActions
                            ? 'bg-[var(--bg-inset)] text-[var(--text-muted)] border-[var(--border-subtle)] cursor-not-allowed'
                            : 'bg-[var(--bg-canvas)] text-[var(--text-primary)] border-[var(--border-default)] hover:border-[var(--accent-primary)] hover:text-[var(--accent-primary)] hover:shadow-sm'
                        }
                    `}
                >
                    {isUploading ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                        <Download className="w-4 h-4" />
                    )}
                    <span className="hidden sm:inline">Export</span>
                </button>

                {/* Save Button */}
                <button
                    onClick={() => saveChanges()}
                    disabled={disabledActions || !hasUnsavedChanges}
                    className={`
                        touch-target flex min-h-11 items-center gap-2 px-2 sm:px-4 py-2
                        font-display text-xs sm:text-sm font-semibold uppercase tracking-wide
                        rounded-lg transition-all duration-200
                        ${(hasUnsavedChanges && !disabledActions)
                            ? 'bg-[var(--accent-primary)] text-[var(--neutral-950)] hover:bg-[var(--accent-primary-dim)] shadow-lg shadow-[var(--accent-primary-glow)]'
                            : 'bg-[var(--bg-inset)] text-[var(--text-muted)] cursor-not-allowed'
                        }
                    `}
                >
                    {isUploading ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                        <Save className="w-4 h-4" />
                    )}
                    <span className="hidden sm:inline">Save</span>
                </button>
            </div>
        </header>
    );
};

export default Header;
