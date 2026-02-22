import React from 'react';
import { useEditor } from '../contexts/EditorContext';
import { ChevronLeft, ChevronRight } from 'lucide-react';

const PageNavigation: React.FC = () => {
  const { currentPage, setCurrentPage, pageCount, sessionId } = useEditor();

  if (!sessionId) return null;

  const handlePrev = () => {
    if (currentPage > 0) {
      setCurrentPage(currentPage - 1);
    }
  };

  const handleNext = () => {
    if (currentPage < pageCount - 1) {
      setCurrentPage(currentPage + 1);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent, action: 'prev' | 'next') => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (action === 'prev') handlePrev();
      else handleNext();
    }
  };

  return (
    <nav
      className="absolute bottom-16 sm:bottom-6 left-1/2 -translate-x-1/2 bg-white rounded-full shadow-lg border border-gray-200 px-2 sm:px-4 py-2 flex items-center gap-2 sm:gap-4 z-50 max-w-[calc(100%-1rem)]"
      aria-label="Page navigation"
    >
      <button
        onClick={handlePrev}
        onKeyDown={(e) => handleKeyDown(e, 'prev')}
        disabled={currentPage === 0}
        className={`p-1 rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-sky-500 ${currentPage === 0
            ? 'text-gray-300 cursor-not-allowed'
            : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
          }`}
        title="Previous Page"
        aria-label="Go to previous page"
      >
        <ChevronLeft className="w-5 h-5" />
      </button>

      <span className="text-xs sm:text-sm font-medium text-gray-600 font-mono whitespace-nowrap" aria-live="polite">
        Page {currentPage + 1} of {pageCount}
      </span>

      <button
        onClick={handleNext}
        onKeyDown={(e) => handleKeyDown(e, 'next')}
        disabled={currentPage === pageCount - 1}
        className={`p-1 rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-sky-500 ${currentPage === pageCount - 1
            ? 'text-gray-300 cursor-not-allowed'
            : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
          }`}
        title="Next Page"
        aria-label="Go to next page"
      >
        <ChevronRight className="w-5 h-5" />
      </button>
    </nav>
  );
};

export default PageNavigation;
