import React from 'react';
import { useEditor } from '../contexts/EditorContext';
import { MousePointer2, Pen, Square, Circle, Type, Palette, Move, Undo } from 'lucide-react';

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
    <div className="flex flex-col gap-6" role="group" aria-label="Drawing tools">
      {/* Actions */}
      <div className="space-y-3">
        <label className="text-xs font-semibold text-sky-600 uppercase tracking-wider" id="actions-label">
          Actions
        </label>
        <div className="flex gap-2" role="group" aria-labelledby="actions-label">
          <button
            onClick={undo}
            className="w-full p-2.5 bg-gradient-to-r from-sky-50 to-cyan-50 border border-sky-200 rounded-xl text-sky-700 hover:from-sky-100 hover:to-cyan-100 hover:text-sky-800 hover:shadow-lg hover:shadow-sky-200/50 hover:-translate-y-0.5 transition-all duration-300 flex items-center justify-center gap-2 group focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2"
            title="Undo last action (Ctrl+Z)"
            aria-label="Undo last action"
          >
            <Undo className="w-4 h-4 group-hover:rotate-12 transition-transform" aria-hidden="true" />
            <span className="text-sm font-medium">Undo</span>
          </button>
        </div>
      </div>

      {/* Drawing Modes */}
      <div className="space-y-3">
        <label className="text-xs font-semibold text-sky-600 uppercase tracking-wider" id="modes-label">
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
                className={`p-2.5 rounded-xl transition-all duration-300 flex items-center justify-center group relative tool-mode-btn focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2 ${isActive
                  ? 'bg-gradient-to-br from-sky-600 to-cyan-600 text-white shadow-lg shadow-sky-500/40 scale-105 active'
                  : 'bg-gradient-to-br from-sky-50 to-cyan-50 text-sky-600 hover:from-sky-100 hover:to-cyan-100 hover:text-sky-700 hover:scale-110'
                  }`}
                title={mode.label}
                aria-label={mode.description}
                aria-pressed={isActive}
                role="radio"
                aria-checked={isActive}
              >
                <Icon className="w-5 h-5" aria-hidden="true" />
                {/* Tooltip */}
                <span className="absolute -top-10 left-1/2 -translate-x-1/2 px-3 py-1.5 bg-gray-900 text-white text-xs rounded-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-xl" role="tooltip">
                  {mode.label}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Styling Options */}
      <div className="space-y-4 p-5 bg-gradient-to-br from-sky-50/50 to-cyan-50/50 rounded-2xl border border-sky-100/50 shadow-lg shadow-sky-100/30" role="group" aria-label="Styling options">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label htmlFor="color-picker" className="text-xs font-semibold text-sky-700 uppercase tracking-wider flex items-center gap-2">
              <Palette className="w-3.5 h-3.5" aria-hidden="true" /> Color
            </label>
            <span className="text-xs font-mono text-sky-600 uppercase font-semibold" aria-live="polite">{color}</span>
          </div>
          <div className="flex items-center gap-2">
            <input
              id="color-picker"
              type="color"
              value={color}
              onChange={(e) => setColor(e.target.value)}
              className="w-full h-12 rounded-xl cursor-pointer border-2 border-sky-200 hover:border-sky-400 transition-all focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2"
              aria-label="Select annotation color"
            />
          </div>
        </div>

        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label htmlFor="stroke-width" className="text-xs font-semibold text-sky-700 uppercase tracking-wider flex items-center gap-2">
              <Move className="w-3.5 h-3.5" aria-hidden="true" /> Stroke
            </label>
            <span className="text-xs font-semibold text-sky-700 bg-sky-100 px-2 py-1 rounded-lg" aria-live="polite">
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
            className="w-full h-2 bg-gradient-to-r from-sky-200 to-cyan-200 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-sky-500 focus:ring-offset-2"
            aria-label="Stroke width"
          />
        </div>

        {/* Text Options */}
        <div className="space-y-3 pt-3 border-t border-sky-200/50">
          <label className="text-xs font-semibold text-sky-700 uppercase tracking-wider flex items-center gap-2">
            <Type className="w-3.5 h-3.5" aria-hidden="true" /> Typography
          </label>
          <div className="grid grid-cols-2 gap-2">
            <div className="relative">
              <select
                id="font-family"
                value={fontFamily}
                onChange={(e) => setFontFamily(e.target.value)}
                className="w-full p-2.5 text-xs font-medium bg-white border border-sky-200 rounded-xl focus:outline-none focus:border-sky-500 focus:ring-2 focus:ring-sky-200 transition-all appearance-none"
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
              className="w-full p-2.5 text-xs font-medium bg-white border border-sky-200 rounded-xl focus:outline-none focus:border-sky-500 focus:ring-2 focus:ring-sky-200 transition-all"
              aria-label="Font size"
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Toolbar;
