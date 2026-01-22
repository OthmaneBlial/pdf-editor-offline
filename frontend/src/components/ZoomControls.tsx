import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { ZoomIn, ZoomOut, Maximize, Maximize2, Square } from 'lucide-react';

const ZOOM_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

const ZoomControls: React.FC = () => {
    const { zoom, setZoom } = useEditor();
    const [showPresets, setShowPresets] = useState(false);

    const handleZoomIn = () => {
        setZoom(Math.min(zoom + 0.25, 5)); // Max zoom 5x
    };

    const handleZoomOut = () => {
        setZoom(Math.max(zoom - 0.25, 0.25)); // Min zoom 0.25x
    };

    const fitToWidth = () => {
        // Fit PDF to viewport width (approximate - assumes standard PDF width)
        const viewportWidth = window.innerWidth - 400; // Subtract sidebar
        const pdfWidth = 612; // Standard PDF width in points
        const newZoom = Math.min((viewportWidth / pdfWidth) * 0.9, 3);
        setZoom(Math.max(0.5, newZoom));
    };

    const fitToPage = () => {
        // Fit entire page to viewport
        const viewportHeight = window.innerHeight - 200;
        const viewportWidth = window.innerWidth - 400;
        const pdfHeight = 792; // Standard PDF height
        const pdfWidth = 612;
        const newZoom = Math.min(viewportHeight / pdfHeight, viewportWidth / pdfWidth) * 0.9;
        setZoom(Math.max(0.5, Math.min(newZoom, 3)));
    };

    const resetZoom = () => {
        setZoom(1);
    };

    return (
        <div className="absolute bottom-6 right-6 bg-[var(--card-bg)] rounded-xl shadow-xl border border-[var(--border-color)] p-2 flex items-center gap-1 z-50 backdrop-blur-xl">
            {/* Zoom Out */}
            <button
                onClick={handleZoomOut}
                className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
                title="Zoom Out (Ctrl+-)"
            >
                <ZoomOut className="w-4 h-4" />
            </button>

            {/* Zoom Level Button with Presets */}
            <div className="relative">
                <button
                    onClick={() => setShowPresets(!showPresets)}
                    className="px-2 py-1 text-xs font-semibold text-[var(--text-primary)] bg-[var(--bg-primary)] hover:bg-[var(--hover-bg)] rounded-lg border border-[var(--border-color)] min-w-[52px] transition-all"
                    title="Click for zoom presets"
                >
                    {Math.round(zoom * 100)}%
                </button>

                {showPresets && (
                    <div className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 bg-[var(--card-bg)] border border-[var(--border-color)] rounded-lg shadow-xl p-1 min-w-[80px]">
                        {ZOOM_PRESETS.map(preset => (
                            <button
                                key={preset}
                                onClick={() => { setZoom(preset); setShowPresets(false); }}
                                className={`w-full px-3 py-1.5 text-xs text-left rounded-md transition-colors ${Math.round(zoom * 100) === preset * 100
                                        ? 'bg-[var(--color-primary-600)] text-white'
                                        : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)]'
                                    }`}
                            >
                                {preset * 100}%
                            </button>
                        ))}
                    </div>
                )}
            </div>

            {/* Zoom In */}
            <button
                onClick={handleZoomIn}
                className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
                title="Zoom In (Ctrl++)"
            >
                <ZoomIn className="w-4 h-4" />
            </button>

            <div className="w-px h-5 bg-[var(--border-color)] mx-1"></div>

            {/* Fit to Width */}
            <button
                onClick={fitToWidth}
                className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
                title="Fit to Width"
            >
                <Maximize2 className="w-4 h-4" />
            </button>

            {/* Fit to Page */}
            <button
                onClick={fitToPage}
                className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
                title="Fit to Page"
            >
                <Square className="w-4 h-4" />
            </button>

            {/* Reset Zoom */}
            <button
                onClick={resetZoom}
                className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
                title="Reset to 100%"
            >
                <Maximize className="w-4 h-4" />
            </button>
        </div>
    );
};

export default ZoomControls;

