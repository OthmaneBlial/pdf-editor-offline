import { useRef } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { FileUp } from 'lucide-react';
import { addRecentFile } from '../services/recentFiles';
import { isDesktopRuntime, openPdfWithDesktopDialog } from '../lib/desktop';
import { isPdfFile } from '../lib/pdfFiles';

interface FileUploadProps {
  compact?: boolean;
}

const FileUpload: React.FC<FileUploadProps> = ({ compact = false }) => {
  const { setDocument } = useEditor();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const openFile = (file: File) => {
    if (!isPdfFile(file)) {
      return;
    }
    setDocument(file);
    void addRecentFile(file);
  };

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const input = event.currentTarget;
    const file = input.files?.[0];
    // A file input does not emit another change event for the same path unless
    // its value is cleared after every selection.
    input.value = '';
    if (file) {
      openFile(file);
    }
  };

  const triggerFileInput = async () => {
    if (isDesktopRuntime()) {
      const file = await openPdfWithDesktopDialog();
      if (file) {
        openFile(file);
      }
      return;
    }

    fileInputRef.current?.click();
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const file = e.dataTransfer.files?.[0];
    if (file) {
      openFile(file);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div className="w-full">
      <div
        onClick={triggerFileInput}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        id="file-upload"
        className={`group relative w-full rounded-xl border-2 border-dashed border-[var(--sidebar-border)] hover:border-[var(--accent-primary)] bg-white/5 hover:bg-white/10 transition-all duration-300 cursor-pointer flex items-center justify-center overflow-hidden focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)] focus:ring-offset-2 ${compact ? 'touch-target min-h-12 flex-row gap-2 px-3 py-2' : 'flex-col gap-3 py-8'}`}
        role="button"
        tabIndex={0}
        aria-label="Upload PDF file"
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            void triggerFileInput();
          }
        }}
      >
        {/* Glow effect on hover */}
        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 bg-gradient-to-br from-[var(--accent-primary)]/5 via-transparent to-[var(--accent-tertiary)]/5 pointer-events-none" aria-hidden="true" />

        {/* Icon */}
        <div className={`${compact ? 'p-2' : 'p-3'} bg-[var(--accent-primary)]/10 rounded-xl group-hover:bg-[var(--accent-primary)]/20 group-hover:scale-110 transition-all duration-300 relative z-10`} aria-hidden="true">
          <FileUp className={`${compact ? 'h-4 w-4' : 'h-6 w-6'} text-[var(--accent-primary)]`} />
        </div>

        {/* Text */}
        <div className="text-center relative z-10">
          <p className="text-sm font-display font-semibold text-[var(--sidebar-text)] group-hover:text-[var(--accent-primary)] transition-colors">
            Upload PDF
          </p>
          <p className={`${compact ? 'sr-only' : 'mt-1 text-xs'} text-[var(--sidebar-text-muted)] font-body`}>
            Click or drag file here
          </p>
        </div>

        {/* Corner accents */}
        <div className="absolute top-3 left-3 w-3 h-3 border-t-2 border-l-2 border-[var(--accent-primary)] rounded-tl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" aria-hidden="true" />
        <div className="absolute top-3 right-3 w-3 h-3 border-t-2 border-r-2 border-[var(--accent-primary)] rounded-tr opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" aria-hidden="true" />
        <div className="absolute bottom-3 left-3 w-3 h-3 border-b-2 border-l-2 border-[var(--accent-primary)] rounded-bl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" aria-hidden="true" />
        <div className="absolute bottom-3 right-3 w-3 h-3 border-b-2 border-r-2 border-[var(--accent-primary)] rounded-br opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" aria-hidden="true" />
      </div>

      <input
        ref={fileInputRef}
        type="file"
        accept="application/pdf,.pdf"
        onChange={handleFileUpload}
        className="hidden"
        aria-label="PDF file input"
      />
    </div>
  );
};

export default FileUpload;
