import { useRef } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { FileUp } from 'lucide-react';
import { addRecentFile } from '../services/recentFiles';

const FileUpload: React.FC = () => {
  const { setDocument } = useEditor();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) {
      setDocument(file);
      addRecentFile(file);
    }
  };

  const triggerFileInput = () => {
    fileInputRef.current?.click();
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    const file = e.dataTransfer.files?.[0];
    if (file && file.type === 'application/pdf') {
      setDocument(file);
      addRecentFile(file);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
  };

  return (
    <div className="w-full">
      <div
        onClick={triggerFileInput}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        id="file-upload"
        className="group relative w-full py-8 rounded-xl border-2 border-dashed border-[var(--sidebar-border)] hover:border-[var(--accent-primary)] bg-white/5 hover:bg-white/10 transition-all duration-300 cursor-pointer flex flex-col items-center justify-center gap-3 overflow-hidden focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)] focus:ring-offset-2"
        role="button"
        tabIndex={0}
        aria-label="Upload PDF file"
        onKeyPress={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            triggerFileInput();
          }
        }}
      >
        {/* Glow effect on hover */}
        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 bg-gradient-to-br from-[var(--accent-primary)]/5 via-transparent to-[var(--accent-tertiary)]/5 pointer-events-none" aria-hidden="true" />

        {/* Icon */}
        <div className="p-3 bg-[var(--accent-primary)]/10 rounded-xl group-hover:bg-[var(--accent-primary)]/20 group-hover:scale-110 transition-all duration-300 relative z-10" aria-hidden="true">
          <FileUp className="w-6 h-6 text-[var(--accent-primary)]" />
        </div>

        {/* Text */}
        <div className="text-center relative z-10">
          <p className="text-sm font-display font-semibold text-[var(--sidebar-text)] group-hover:text-[var(--accent-primary)] transition-colors">
            Upload PDF
          </p>
          <p className="text-xs text-[var(--sidebar-text-muted)] mt-1 font-body">
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
        accept="application/pdf"
        onChange={handleFileUpload}
        className="hidden"
        aria-label="PDF file input"
      />
    </div>
  );
};

export default FileUpload;
