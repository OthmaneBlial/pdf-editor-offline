import React, { useEffect, useState } from 'react';
import { Clock, FileText, X, Trash2 } from 'lucide-react';
import {
  getRecentFiles,
  removeRecentFile,
  clearRecentFiles,
  formatFileSize,
  formatDate,
  type RecentFile
} from '../services/recentFiles';

interface RecentFilesProps {
  onFileSelect?: (fileName: string) => void;
}

const RecentFiles: React.FC<RecentFilesProps> = ({ onFileSelect }) => {
  const [files, setFiles] = useState<RecentFile[]>([]);
  const [isOpen, setIsOpen] = useState(false);

  const loadFiles = () => {
    setFiles(getRecentFiles());
  };

  // Load on mount
  useEffect(() => {
    loadFiles();
  }, []);

  const handleRemove = (e: React.MouseEvent, fileName: string) => {
    e.stopPropagation();
    removeRecentFile(fileName);
    loadFiles();
  };

  const handleClear = (e: React.MouseEvent) => {
    e.stopPropagation();
    clearRecentFiles();
    loadFiles();
  };

  // Track when PDF is opened - event is dispatched from the service
  useEffect(() => {
    const handleFileOpened = () => {
      loadFiles();
    };

    window.addEventListener('pdf-opened', handleFileOpened);
    return () => {
      window.removeEventListener('pdf-opened', handleFileOpened);
    };
  }, []);

  if (files.length === 0) return null;

  return (
    <div className="relative mt-4">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-between p-3 bg-slate-800/50 hover:bg-slate-800 border border-slate-700 rounded-xl transition-all duration-200 group"
      >
        <div className="flex items-center gap-2 text-slate-400">
          <Clock className="w-4 h-4 group-hover:text-emerald-400 transition-colors" />
          <span className="text-sm font-medium">Recent Files ({files.length})</span>
        </div>
        <span className="text-[10px] text-slate-400 bg-slate-900 px-2 py-0.5 rounded-full border border-slate-700">
          {isOpen ? 'Hide' : 'Show'}
        </span>
      </button>

      {isOpen && (
        <div className="absolute top-full left-0 right-0 mt-2 bg-slate-800 border border-slate-700 rounded-xl shadow-xl overflow-hidden z-50">
          <div className="flex justify-between items-center px-4 py-2 border-b border-slate-700 bg-slate-900/50">
            <span className="text-xs font-semibold text-slate-400">History</span>
            <button
              onClick={handleClear}
              className="text-[10px] flex items-center gap-1 text-red-400 hover:text-red-300 transition-colors"
            >
              <Trash2 className="w-3 h-3" /> Clear All
            </button>
          </div>
          <div className="max-h-60 overflow-y-auto">
            {files.map((file) => (
              <div
                key={file.name}
                onClick={() => onFileSelect?.(file.name)}
                className="flex items-center justify-between p-3 hover:bg-slate-700 cursor-pointer border-b border-slate-700/50 last:border-0 transition-colors group"
              >
                <div className="flex items-center gap-3 overflow-hidden">
                  <div className="p-2 rounded-lg bg-slate-900 text-emerald-400">
                    <FileText className="w-4 h-4" />
                  </div>
                  <div className="flex flex-col min-w-0">
                    <span className="text-sm font-medium text-slate-300 truncate" title={file.name}>{file.name}</span>
                    <span className="text-[10px] text-slate-500">
                      {formatFileSize(file.size)} • {formatDate(file.lastOpened)}
                    </span>
                  </div>
                </div>
                <button
                  onClick={(e) => handleRemove(e, file.name)}
                  className="p-1.5 rounded-lg opacity-0 group-hover:opacity-100 hover:bg-slate-700 text-slate-400 hover:text-red-400 transition-all"
                  title={`Remove ${file.name}`}
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default RecentFiles;
