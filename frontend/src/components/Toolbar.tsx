import React from 'react';
import { useEditor } from '../contexts/EditorContext';
import { MousePointer2, Pen, Square, Circle, Type, Palette, Move, Undo, Wand2 } from 'lucide-react';

const Toolbar: React.FC = () => {
  const {
    drawingMode,
    setDrawingMode,
    color,
    setColor,
    strokeWidth,
    setStrokeWidth,
    fontSize,
    setFontSize,
    fontFamily,
    setFontFamily,
    undo
  } = useEditor();

  const modes = [
    { id: 'select', label: 'Select', icon: MousePointer2, description: 'Select and move objects' },
    { id: 'pen', label: 'Pen', icon: Pen, description: 'Draw freehand annotations' },
    { id: 'rectangle', label: 'Rectangle', icon: Square, description: 'Add rectangle shape' },
    { id: 'circle', label: 'Circle', icon: Circle, description: 'Add circle shape' },
    { id: 'text', label: 'Text', icon: Type, description: 'Add text annotation' },
  ];

  const handleKeyDown = (modeId: string, e: React.KeyboardEvent) => {
    // Handle Enter and Space for keyboard activation
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      setDrawingMode(modeId);
    }
  };

  return (
    <div className="flex flex-col gap-4" role="group" aria-label="Drawing tools">
      {/* Actions section */}
      <div className="space-y-2">
        <label className="text-xs font-bold text-sky-600 uppercase tracking-wider flex items-center gap-1.5" id="actions-label">
          <Wand2 className="w-3.5 h-3.5" />
          Actions
        </label>
        <div className="flex gap-2" role="group" aria-labelledby="actions-label">
          <button
            onClick={undo}
            className="w-full p-3 bg-gradient-to-br from-sky-500 to-blue-600 border border-sky-400/50 rounded-xl text-white hover:from-sky-600 hover:to-blue-700 hover:shadow-lg hover:shadow-sky-500/40 hover:-translate-y-0.5 transition-all duration-300 flex items-center justify-center gap-2 group focus:outline-none focus:ring-2 focus:ring-sky-400 focus:ring-offset-2"
            title="Undo last action (Ctrl+Z)"
            aria-label="Undo last action"
          >
            <Undo className="w-4 h-4 group-hover:-rotate-12 transition-transform" aria-hidden="true" />
            <span className="text-sm font-bold">Undo</span>
          </button>
        </div>
      </div>

      {/* Drawing Modes section */}
      <div className="space-y-2">
        <label className="text-xs font-bold text-sky-600 uppercase tracking-wider flex items-center gap-1.5" id="modes-label">
          <Pen className="w-3.5 h-3.5" />
          Mode
        </label>
        <div
          className="grid grid-cols-5 gap-2"
          role="radiogroup"
          aria-labelledby="modes-label"
          aria-label="Drawing mode selection"
        >
          {modes.map((mode) => {
            const Icon = mode.icon;
            const isActive = drawingMode === mode.id;
            return (
              <button
                key={mode.id}
                onClick={() => setDrawingMode(mode.id)}
                onKeyDown={(e) => handleKeyDown(mode.id, e)}
                className={`p-3 rounded-xl transition-all duration-300 flex items-center justify-center group relative tool-mode-btn focus:outline-none focus:ring-2 focus:ring-sky-400 focus:ring-offset-2 ${
                  isActive
                    ? 'bg-gradient-to-br from-sky-500 to-blue-600 text-white shadow-lg shadow-sky-500/40 scale-105'
                    : 'bg-gradient-to-br from-slate-100 to-slate-200 dark:from-slate-800 dark:to-slate-700 text-slate-600 dark:text-slate-300 hover:from-sky-100 hover:to-blue-100 dark:hover:from-sky-900/30 dark:hover:to-blue-900/30 hover:text-sky-600 dark:hover:text-sky-300 hover:scale-110 hover:shadow-md'
                }`}
                title={mode.label}
                aria-label={mode.description}
                aria-pressed={isActive}
                role="radio"
                aria-checked={isActive}
              >
                <Icon className="w-5 h-5" aria-hidden="true" />
                {/* Tooltip */}
                <span className="absolute -top-10 left-1/2 -translate-x-1/2 px-3 py-1.5 bg-slate-900 text-white text-xs rounded-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-xl" role="tooltip">
                  {mode.label}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Styling Options - Beautiful gradient card */}
      <div className="p-4 bg-gradient-to-br from-sky-50 via-blue-50 to-indigo-50 dark:from-slate-900/50 dark:to-slate-800/50 rounded-2xl border border-sky-200/50 dark:border-slate-700 shadow-xl shadow-sky-200/50 dark:shadow-none" role="group" aria-label="Styling options">
        {/* Color picker */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label htmlFor="color-picker" className="text-xs font-bold text-sky-700 dark:text-sky-300 uppercase tracking-wider flex items-center gap-2">
              <Palette className="w-4 h-4" />
              Color
            </label>
            <span className="text-xs font-mono font-bold text-sky-600 dark:text-sky-400 bg-sky-100 dark:bg-sky-900/50 px-2 py-1 rounded-lg" aria-live="polite">
              {color}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <input
              id="color-picker"
              type="color"
              value={color}
              onChange={(e) => setColor(e.target.value)}
              className="w-full h-12 rounded-xl cursor-pointer border-2 border-sky-300 dark:border-slate-600 hover:border-sky-500 transition-all focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2 shadow-md"
              aria-label="Select annotation color"
            />
          </div>
        </div>

        {/* Stroke width */}
        <div className="space-y-3 mt-4">
          <div className="flex items-center justify-between">
            <label htmlFor="stroke-width" className="text-xs font-bold text-sky-700 dark:text-sky-300 uppercase tracking-wider flex items-center gap-2">
              <Move className="w-4 h-4" />
              Stroke
            </label>
            <span className="text-xs font-bold text-sky-600 dark:text-sky-400 bg-sky-100 dark:bg-sky-900/50 px-2 py-1 rounded-lg" aria-live="polite">
              {strokeWidth}px
            </span>
          </div>
          <input
            id="stroke-width"
            type="range"
            min="1"
            max="20"
            value={strokeWidth}
            onChange={(e) => setStrokeWidth(parseInt(e.target.value))}
            className="w-full h-2 bg-gradient-to-r from-sky-200 to-blue-200 dark:from-slate-700 dark:to-slate-600 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-gradient-to-br [&::-webkit-slider-thumb]:from-sky-500 [&::-webkit-slider-thumb]:to-blue-600 [&::-webkit-slider-thumb]:shadow-lg [&::-webkit-slider-thumb]:hover:scale-110 [&::-webkit-slider-thumb]:transition-transform"
            aria-label="Stroke width"
          />
        </div>

        {/* Typography section */}
        <div className="space-y-3 pt-4 mt-4 border-t border-sky-200/50 dark:border-slate-700">
          <label className="text-xs font-bold text-sky-700 dark:text-sky-300 uppercase tracking-wider flex items-center gap-2">
            <Type className="w-4 h-4" />
            Typography
          </label>
          <div className="grid grid-cols-2 gap-2">
            <div className="relative">
              <select
                id="font-family"
                value={fontFamily}
                onChange={(e) => setFontFamily(e.target.value)}
                className="w-full p-2.5 text-xs font-semibold bg-white dark:bg-slate-800 border border-sky-300 dark:border-slate-600 rounded-xl focus:outline-none focus:border-sky-500 focus:ring-2 focus:ring-sky-200 dark:focus:ring-sky-900 transition-all appearance-none cursor-pointer text-slate-700 dark:text-slate-200 shadow-sm"
                aria-label="Font family"
              >
                <option value="Arial">Arial</option>
                <option value="Times New Roman">Times New Roman</option>
                <option value="Courier New">Courier New</option>
                <option value="Georgia">Georgia</option>
                <option value="Verdana">Verdana</option>
              </select>
            </div>
            <input
              id="font-size"
              type="number"
              min="8"
              max="72"
              value={fontSize}
              onChange={(e) => setFontSize(parseInt(e.target.value))}
              className="w-full p-2.5 text-xs font-semibold bg-white dark:bg-slate-800 border border-sky-300 dark:border-slate-600 rounded-xl focus:outline-none focus:border-sky-500 focus:ring-2 focus:ring-sky-200 dark:focus:ring-sky-900 transition-all text-slate-700 dark:text-slate-200 shadow-sm"
              aria-label="Font size"
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Toolbar;
