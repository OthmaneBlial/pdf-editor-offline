import { useEditor } from '../contexts/EditorContext';
import { Download, Save, Loader2 } from 'lucide-react';
import ThemeToggle from './ThemeToggle';

const Header: React.FC = () => {
    const { exportPDF, saveChanges, hasUnsavedChanges, sessionId, isUploading } = useEditor();
    const disabledActions = !sessionId || isUploading;

    return (
        <header className="h-14 bg-[var(--bg-canvas)] border-b border-[var(--border-subtle)] flex items-center justify-between px-6 relative z-10">
            {/* Breadcrumb */}
            <div className="flex items-center gap-2 text-sm font-body">
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

            {/* Actions */}
            <div className="flex items-center gap-3">
                <ThemeToggle />

                {/* Export Button */}
                <button
                    onClick={() => exportPDF()}
                    disabled={disabledActions}
                    className={`
                        flex items-center gap-2 px-4 py-2 
                        font-display text-sm font-semibold uppercase tracking-wide
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
                    <span>Export</span>
                </button>

                {/* Save Button */}
                <button
                    onClick={() => saveChanges()}
                    disabled={disabledActions || !hasUnsavedChanges}
                    className={`
                        flex items-center gap-2 px-4 py-2
                        font-display text-sm font-semibold uppercase tracking-wide
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
                    <span>Save</span>
                </button>
            </div>
        </header>
    );
};

export default Header;
