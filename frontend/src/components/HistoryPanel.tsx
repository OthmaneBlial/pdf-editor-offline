import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { Undo2, Redo2, Clock, Trash2, ChevronDown, ChevronUp, Sparkles } from 'lucide-react';

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
    <div className="overflow-hidden" role="region" aria-labelledby="history-heading">
      {/* Header with beautiful gradient background */}
      <div className="relative">
        {/* Animated gradient background */}
        <div className="absolute inset-0 bg-gradient-to-br from-violet-600 via-purple-600 to-fuchsia-600" />
        <div className="absolute inset-0 bg-[url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjAiIGhlaWdodD0iMjAiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PGNpcmNsZSBjeD0iMSIgY3k9IjEiIHI9IjEiIGZpbGw9InJnYmEoMjU1LDI1NSwyNTUsMC4xKSIvPjwvc3ZnPg==')] opacity-30" />

        <div className="relative p-4">
          {/* Title section */}
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2" id="history-heading">
              <div className="p-2 bg-white/20 backdrop-blur-sm rounded-xl">
                <Sparkles className="w-4 h-4 text-white" aria-hidden="true" />
              </div>
              <span className="text-sm font-bold text-white">History</span>
            </div>
            <button
              onClick={() => setIsExpanded(!isExpanded)}
              className="p-2 bg-white/20 backdrop-blur-sm rounded-lg text-white hover:bg-white/30 transition-all focus:outline-none focus:ring-2 focus:ring-white/50"
              aria-expanded={isExpanded}
              aria-controls="history-list"
              aria-label={isExpanded ? 'Collapse history' : 'Expand history'}
            >
              {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
            </button>
          </div>

          {/* Undo/Redo buttons with glass effect */}
          <div className="flex items-center gap-2" role="group" aria-label="Undo and redo">
            <button
              onClick={undo}
              disabled={!canUndo}
              className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold transition-all focus:outline-none focus:ring-2 focus:ring-white/50 ${
                canUndo
                  ? 'bg-white text-purple-700 hover:bg-white/90 shadow-lg shadow-black/20'
                  : 'bg-white/20 text-white/50 cursor-not-allowed'
              }`}
              title="Undo (Ctrl+Z)"
              aria-label="Undo last action"
            >
              <Undo2 className="w-4 h-4" aria-hidden="true" />
              Undo
            </button>

            <button
              onClick={redo}
              disabled={!canRedo}
              className={`flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold transition-all focus:outline-none focus:ring-2 focus:ring-white/50 ${
                canRedo
                  ? 'bg-white text-purple-700 hover:bg-white/90 shadow-lg shadow-black/20'
                  : 'bg-white/20 text-white/50 cursor-not-allowed'
              }`}
              title="Redo (Ctrl+Y)"
              aria-label="Redo last action"
            >
              <Redo2 className="w-4 h-4" aria-hidden="true" />
              Redo
            </button>
          </div>

          {/* History count */}
          <div className="mt-3 flex items-center justify-between text-xs text-white/80">
            <span className="flex items-center gap-1.5">
              <Clock className="w-3.5 h-3.5" aria-hidden="true" />
              <span aria-live="polite">
                {historyItems.length} state{historyItems.length !== 1 ? 's' : ''} • Position {currentIndex + 1}
              </span>
            </span>
            {historyItems.length > 1 && (
              <button
                onClick={clearHistory}
                className="flex items-center gap-1 px-2 py-1 bg-red-500/80 hover:bg-red-500 text-white rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-red-400"
                aria-label="Clear all history"
              >
                <Trash2 className="w-3 h-3" aria-hidden="true" />
                Clear
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Expandable history list with gradient timeline */}
      {isExpanded && historyItems.length > 0 && (
        <div className="bg-gradient-to-b from-slate-50 to-slate-100 dark:from-slate-900 dark:to-slate-950">
          <ul
            id="history-list"
            className="max-h-56 overflow-y-auto custom-scrollbar py-2"
            role="listbox"
            aria-label="History items"
            aria-activedescendant={`history-item-${currentIndex}`}
          >
            {historyItems.map((item, idx) => (
              <li
                key={idx}
                id={`history-item-${idx}`}
                className={`mx-2 my-1 px-3 py-2.5 rounded-xl flex items-center gap-3 text-sm cursor-pointer transition-all ${
                  idx === currentIndex
                    ? 'bg-gradient-to-r from-purple-600 to-fuchsia-600 text-white shadow-lg shadow-purple-500/30 scale-[1.02]'
                    : idx < currentIndex
                      ? 'bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:shadow-md hover:scale-[1.01]'
                      : 'bg-white/50 dark:bg-slate-800/50 text-slate-400 dark:text-slate-500'
                }`}
                role="option"
                aria-selected={idx === currentIndex}
              >
                <div className={`w-8 h-8 rounded-full flex items-center justify-center font-bold text-xs ${
                  idx === currentIndex
                    ? 'bg-white/20 text-white'
                    : idx < currentIndex
                      ? 'bg-gradient-to-br from-purple-100 to-fuchsia-100 dark:from-purple-900/30 dark:to-fuchsia-900/30 text-purple-700 dark:text-purple-300'
                      : 'bg-slate-200 dark:bg-slate-700 text-slate-500 dark:text-slate-500'
                }`} aria-hidden="true">
                  {idx + 1}
                </div>
                <span className="flex-1 font-medium">{item.label}</span>
                {idx === currentIndex && (
                  <span className="text-[10px] px-2 py-0.5 bg-white/20 text-white rounded-full font-medium">
                    Current
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};

export default HistoryPanel;
