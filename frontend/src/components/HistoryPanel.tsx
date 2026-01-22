import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { Undo2, Redo2, History, Clock, Trash2, ChevronDown, ChevronUp } from 'lucide-react';

interface HistoryItem {
    index: number;
    timestamp: Date;
    label: string;
}

const HistoryPanel: React.FC = () => {
    const { undo, redo, history, historyStep, canUndo, canRedo, clearHistory } = useEditor();
    const [isExpanded, setIsExpanded] = useState(false);

    // Generate history items with timestamps
    const historyItems: HistoryItem[] = history.map((_, index) => ({
        index,
        timestamp: new Date(),
        label: index === 0 ? 'Initial State' : `Edit ${index}`
    }));

    const currentIndex = historyStep;

    return (
        <div className="bg-[var(--card-bg)] border border-[var(--border-color)] rounded-xl shadow-lg overflow-hidden">
            {/* Header with Undo/Redo buttons */}
            <div className="p-3 border-b border-[var(--border-color)] bg-[var(--bg-secondary)]/50">
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                        <History className="w-4 h-4 text-[var(--color-primary-600)]" />
                        <span className="text-sm font-semibold text-[var(--text-primary)]">History</span>
                    </div>
                    <button
                        onClick={() => setIsExpanded(!isExpanded)}
                        className="p-1 text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
                    >
                        {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                    </button>
                </div>

                <div className="flex items-center gap-2">
                    <button
                        onClick={undo}
                        disabled={!canUndo}
                        className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition-all ${canUndo
                                ? 'bg-[var(--bg-primary)] text-[var(--text-primary)] hover:bg-[var(--hover-bg)] border border-[var(--border-color)]'
                                : 'bg-[var(--bg-primary)]/50 text-[var(--text-secondary)]/50 cursor-not-allowed border border-transparent'
                            }`}
                        title="Undo (Ctrl+Z)"
                    >
                        <Undo2 className="w-3.5 h-3.5" />
                        Undo
                    </button>

                    <button
                        onClick={redo}
                        disabled={!canRedo}
                        className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition-all ${canRedo
                                ? 'bg-[var(--bg-primary)] text-[var(--text-primary)] hover:bg-[var(--hover-bg)] border border-[var(--border-color)]'
                                : 'bg-[var(--bg-primary)]/50 text-[var(--text-secondary)]/50 cursor-not-allowed border border-transparent'
                            }`}
                        title="Redo (Ctrl+Y)"
                    >
                        <Redo2 className="w-3.5 h-3.5" />
                        Redo
                    </button>
                </div>

                {/* History count */}
                <div className="mt-2 flex items-center justify-between text-[10px] text-[var(--text-secondary)]">
                    <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        {historyItems.length} state{historyItems.length !== 1 ? 's' : ''} • Position {currentIndex + 1}
                    </span>
                    {historyItems.length > 1 && (
                        <button
                            onClick={clearHistory}
                            className="flex items-center gap-1 text-red-500 hover:text-red-600 transition-colors"
                        >
                            <Trash2 className="w-3 h-3" />
                            Clear
                        </button>
                    )}
                </div>
            </div>

            {/* Expandable history list */}
            {isExpanded && historyItems.length > 0 && (
                <div className="max-h-48 overflow-y-auto custom-scrollbar">
                    {historyItems.map((item, idx) => (
                        <div
                            key={idx}
                            className={`px-3 py-2 border-b border-[var(--border-color)]/50 last:border-0 flex items-center gap-2 text-xs ${idx === currentIndex
                                    ? 'bg-[var(--color-primary-50)] text-[var(--color-primary-700)]'
                                    : idx < currentIndex
                                        ? 'text-[var(--text-secondary)]'
                                        : 'text-[var(--text-secondary)]/50'
                                }`}
                        >
                            <div className={`w-2 h-2 rounded-full ${idx === currentIndex
                                    ? 'bg-[var(--color-primary-600)]'
                                    : idx < currentIndex
                                        ? 'bg-[var(--text-secondary)]'
                                        : 'bg-[var(--border-color)]'
                                }`} />
                            <span className="flex-1">{item.label}</span>
                            {idx === currentIndex && (
                                <span className="text-[10px] px-1.5 py-0.5 bg-[var(--color-primary-600)] text-white rounded">
                                    Current
                                </span>
                            )}
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
};

export default HistoryPanel;
