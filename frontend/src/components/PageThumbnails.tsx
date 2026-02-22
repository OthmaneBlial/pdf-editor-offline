import React, { useState, useCallback } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { Grid, GripVertical, LayoutGrid } from 'lucide-react';

interface PageThumbnailsProps {
    isOpen?: boolean;
}

const PageThumbnails: React.FC<PageThumbnailsProps> = ({ isOpen = false }) => {
    const { pageCount, currentPage, setCurrentPage, reorderPages } = useEditor();
    const [isExpanded, setIsExpanded] = useState(isOpen);
    const [draggedPage, setDraggedPage] = useState<number | null>(null);
    const [dragOverPage, setDragOverPage] = useState<number | null>(null);

    const handleDragStart = useCallback((e: React.DragEvent, pageIndex: number) => {
        setDraggedPage(pageIndex);
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', pageIndex.toString());
        // Add drag image styling
        const target = e.target as HTMLElement;
        target.style.opacity = '0.5';
    }, []);

    const handleDragEnd = useCallback((e: React.DragEvent) => {
        const target = e.target as HTMLElement;
        target.style.opacity = '1';
        setDraggedPage(null);
        setDragOverPage(null);
    }, []);

    const handleDragOver = useCallback((e: React.DragEvent, pageIndex: number) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
        if (draggedPage !== null && draggedPage !== pageIndex) {
            setDragOverPage(pageIndex);
        }
    }, [draggedPage]);

    const handleDragLeave = useCallback(() => {
        setDragOverPage(null);
    }, []);

    const handleDrop = useCallback((e: React.DragEvent, targetIndex: number) => {
        e.preventDefault();
        if (draggedPage !== null && draggedPage !== targetIndex && reorderPages) {
            reorderPages(draggedPage, targetIndex);
        }
        setDraggedPage(null);
        setDragOverPage(null);
    }, [draggedPage, reorderPages]);

    if (pageCount === 0) return null;

    return (
        <div className={`hidden lg:block fixed left-0 top-1/2 -translate-y-1/2 z-20 transition-all duration-300 transform ${isExpanded ? 'translate-x-0' : '-translate-x-full pl-2'}`}>
            <div className="bg-[var(--sidebar-bg)] backdrop-blur-xl border border-[var(--border-color)] shadow-xl rounded-r-2xl overflow-hidden flex">
                {/* Content */}
                <div className={`w-64 h-[calc(100dvh-12rem)] flex flex-col transition-all duration-300 ${isExpanded ? 'opacity-100' : 'opacity-0 w-0'}`}>
                    <div className="p-4 border-b border-[var(--border-color)] bg-[var(--bg-secondary)]/50 flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <LayoutGrid className="w-4 h-4 text-[var(--color-primary-600)]" />
                            <h3 className="font-semibold text-[var(--text-primary)]">Pages ({pageCount})</h3>
                        </div>
                        <button onClick={() => setIsExpanded(false)} className="text-xs px-2 py-1 rounded-lg bg-[var(--hover-bg)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
                            Close
                        </button>
                    </div>
                    <p className="text-[10px] text-center text-[var(--text-secondary)] py-2 border-b border-[var(--border-color)]/50 bg-[var(--bg-primary)]/50">
                        Drag pages to reorder
                    </p>
                    <div className="flex-1 overflow-y-auto p-4 space-y-3 custom-scrollbar">
                        {Array.from({ length: pageCount }).map((_, index) => (
                            <div
                                key={index}
                                draggable
                                onDragStart={(e) => handleDragStart(e, index)}
                                onDragEnd={handleDragEnd}
                                onDragOver={(e) => handleDragOver(e, index)}
                                onDragLeave={handleDragLeave}
                                onDrop={(e) => handleDrop(e, index)}
                                onClick={() => setCurrentPage(index)}
                                className={`group relative aspect-[3/4] bg-[var(--card-bg)] rounded-lg border-2 transition-all cursor-pointer
                                    ${currentPage === index
                                        ? 'border-[var(--color-primary-500)] shadow-lg ring-2 ring-[var(--color-primary-200)]'
                                        : 'border-[var(--border-color)] hover:border-[var(--color-primary-300)] shadow-sm'}
                                    ${draggedPage === index ? 'opacity-50 scale-95' : 'hover:scale-105'}
                                    ${dragOverPage === index ? 'border-dashed border-[var(--color-primary-500)] bg-[var(--color-primary-50)]' : ''}`}
                            >
                                {/* Page number badge */}
                                <div className="absolute top-2 left-2 w-6 h-6 bg-[var(--color-primary-600)] text-white text-xs font-bold rounded-full flex items-center justify-center shadow-md">
                                    {index + 1}
                                </div>

                                {/* Placeholder for page content */}
                                <div className="absolute inset-0 flex items-center justify-center text-[var(--color-primary-100)] opacity-20">
                                    <Grid className="w-12 h-12" />
                                </div>

                                {/* Drag handle */}
                                <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity cursor-grab active:cursor-grabbing text-[var(--text-secondary)] p-1 rounded bg-[var(--bg-primary)]/80">
                                    <GripVertical className="w-4 h-4" />
                                </div>

                                {/* Drop indicator */}
                                {dragOverPage === index && draggedPage !== null && (
                                    <div className="absolute inset-0 flex items-center justify-center bg-[var(--color-primary-500)]/10 rounded-lg">
                                        <span className="text-xs font-semibold text-[var(--color-primary-600)]">Drop here</span>
                                    </div>
                                )}
                            </div>
                        ))}
                    </div>
                </div>

                {/* Toggle Button */}
                <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="w-8 bg-[var(--bg-secondary)] hover:bg-[var(--hover-bg)] border-l border-[var(--border-color)] flex items-center justify-center cursor-pointer transition-colors"
                    title="Toggle Page Thumbnails"
                >
                    <div className="rotate-90 text-[10px] font-medium text-[var(--text-secondary)] whitespace-nowrap tracking-wider uppercase">
                        {isExpanded ? 'Hide' : 'Pages'}
                    </div>
                </button>
            </div>
        </div>
    );
};

export default PageThumbnails;
