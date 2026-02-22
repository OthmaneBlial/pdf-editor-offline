import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { ZoomIn, ZoomOut, Maximize, Maximize2, Square } from 'lucide-react';

const ZOOM_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3] as const;

const ZoomControls: React.FC = () => {
  const { zoom, setZoom } = useEditor();
  const [showPresets, setShowPresets] = useState(false);

  const handleZoomIn = () => {
    setZoom(Math.min(zoom + 0.25, 5));
  };

  const handleZoomOut = () => {
    setZoom(Math.max(zoom - 0.25, 0.25));
  };

  const fitToWidth = () => {
    const desktopSidebarOffset = window.innerWidth >= 1024 ? 400 : 48;
    const viewportWidth = Math.max(280, window.innerWidth - desktopSidebarOffset);
    const pdfWidth = 612;
    const newZoom = Math.min((viewportWidth / pdfWidth) * 0.9, 3);
    setZoom(Math.max(0.5, newZoom));
  };

  const fitToPage = () => {
    const desktopSidebarOffset = window.innerWidth >= 1024 ? 400 : 48;
    const viewportHeightOffset = window.innerWidth >= 640 ? 200 : 160;
    const viewportHeight = Math.max(240, window.innerHeight - viewportHeightOffset);
    const viewportWidth = Math.max(280, window.innerWidth - desktopSidebarOffset);
    const pdfHeight = 792;
    const pdfWidth = 612;
    const newZoom = Math.min(viewportHeight / pdfHeight, viewportWidth / pdfWidth) * 0.9;
    setZoom(Math.max(0.5, Math.min(newZoom, 3)));
  };

  const resetZoom = () => {
    setZoom(1);
  };

  const zoomPercent = Math.round(zoom * 100);

  return (
    <div
      className="absolute bottom-3 left-1/2 -translate-x-1/2 lg:left-auto lg:right-6 lg:translate-x-0 lg:bottom-6 bg-[var(--card-bg)] rounded-xl shadow-xl border border-[var(--border-color)] p-2 flex items-center justify-between sm:justify-start gap-1 z-50 backdrop-blur-xl max-w-full overflow-x-auto max-[1024px]:scale-50 max-[1024px]:origin-bottom"
      role="group"
      aria-label="Zoom controls"
    >
      {/* Zoom Out */}
      <button
        onClick={handleZoomOut}
        className="shrink-0 p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
        title="Zoom Out (Ctrl+-)"
        aria-label="Zoom out"
      >
        <ZoomOut className="w-4 h-4" aria-hidden="true" />
      </button>

      {/* Zoom Level Button with Presets */}
      <div className="relative">
        <button
          onClick={() => setShowPresets(!showPresets)}
          className="shrink-0 px-2 py-1 text-xs font-semibold text-[var(--text-primary)] bg-[var(--bg-primary)] hover:bg-[var(--hover-bg)] rounded-lg border border-[var(--border-color)] min-w-[52px] transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
          aria-expanded={showPresets}
          aria-haspopup="listbox"
          aria-label={`Current zoom level: ${zoomPercent}%. Click for zoom presets.`}
        >
          {zoomPercent}%
        </button>

        {showPresets && (
          <ul
            className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 bg-[var(--card-bg)] border border-[var(--border-color)] rounded-lg shadow-xl p-1 min-w-[80px] z-50"
            role="listbox"
            aria-label="Zoom presets"
            aria-activedescendant={`zoom-${zoomPercent}`}
          >
            {ZOOM_PRESETS.map((preset) => {
              const isActive = Math.round(zoom * 100) === preset * 100;
              return (
                <li key={preset}>
                  <button
                    id={`zoom-${preset * 100}`}
                    onClick={() => { setZoom(preset); setShowPresets(false); }}
                    className={`w-full px-3 py-1.5 text-xs text-left rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)] ${isActive
                        ? 'bg-[var(--color-primary-600)] text-white'
                        : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)]'
                      }`}
                    role="option"
                    aria-selected={isActive}
                  >
                    {preset * 100}%
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* Zoom In */}
      <button
        onClick={handleZoomIn}
        className="shrink-0 p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
        title="Zoom In (Ctrl++)"
        aria-label="Zoom in"
      >
        <ZoomIn className="w-4 h-4" aria-hidden="true" />
      </button>

      <div className="w-px h-5 bg-[var(--border-color)] mx-1 shrink-0" aria-hidden="true" />

      {/* Fit to Width */}
      <button
        onClick={fitToWidth}
        className="shrink-0 p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
        title="Fit to Width"
        aria-label="Fit PDF to width"
      >
        <Maximize2 className="w-4 h-4" aria-hidden="true" />
      </button>

      {/* Fit to Page */}
      <button
        onClick={fitToPage}
        className="hidden sm:inline-flex shrink-0 p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
        title="Fit to Page"
        aria-label="Fit PDF to page"
      >
        <Square className="w-4 h-4" aria-hidden="true" />
      </button>

      {/* Reset Zoom */}
      <button
        onClick={resetZoom}
        className="hidden sm:inline-flex shrink-0 p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-500)]"
        title="Reset to 100%"
        aria-label="Reset zoom to 100%"
      >
        <Maximize className="w-4 h-4" aria-hidden="true" />
      </button>
    </div>
  );
};

export default ZoomControls;
