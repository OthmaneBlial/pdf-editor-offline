import { useState } from 'react';
import { EditorProvider } from './contexts/EditorContext';
import FileUpload from './components/FileUpload';
import PDFViewer from './components/PDFViewer';
import Toolbar from './components/Toolbar';
import ImageUpload from './components/ImageUpload';
import ShortcutsModal from './components/ShortcutsModal';
import ErrorBoundary from './components/ErrorBoundary';
import RecentFiles from './components/RecentFiles';
import PageThumbnails from './components/PageThumbnails';
import { FileText, Scissors, RefreshCw, Shield, Zap } from 'lucide-react';
import Header from './components/Header';
import PageNavigation from './components/PageNavigation';
import ZoomControls from './components/ZoomControls';
import ManipulationTools from './components/tools/ManipulationTools';
import ConversionTools from './components/tools/ConversionTools';
import SecurityTools from './components/tools/SecurityTools';
import AdvancedTools from './components/tools/AdvancedTools';
import './App.css';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';

type ViewMode = 'editor' | 'manipulation' | 'conversion' | 'security' | 'advanced';

// Component to handle keyboard shortcuts - must be inside EditorProvider
function KeyboardShortcutsHandler({ onShowHelp }: { onShowHelp: () => void }) {
  useKeyboardShortcuts({
    onShowHelp,
  });
  return null;
}

// Inner component with access to EditorContext
function AppContent() {
  const [activeView, setActiveView] = useState<ViewMode>('editor');
  const [showShortcuts, setShowShortcuts] = useState(false);

  const renderContent = () => {
    switch (activeView) {
      case 'editor':
        return (
          <div className="flex-1 relative overflow-auto p-8 animate-fade-in text-[var(--text-primary)]">
            <PageThumbnails />
            <div className="min-h-full bg-[var(--card-bg)] rounded-2xl shadow-2xl shadow-cyan-200/10 border border-[var(--border-color)] relative overflow-hidden transition-colors duration-300">
              {/* Decorative gradient overlay */}
              <div className="absolute inset-0 bg-gradient-to-br from-cyan-500/5 via-transparent to-blue-500/5 pointer-events-none" />
              <PDFViewer />
              <PageNavigation />
              <ZoomControls />
            </div>
          </div>
        );
      case 'manipulation':
        return <ManipulationTools />;
      case 'conversion':
        return <ConversionTools />;
      case 'security':
        return <SecurityTools />;
      case 'advanced':
        return <AdvancedTools />;
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen w-full bg-[var(--bg-primary)] overflow-hidden font-sans text-text relative transition-colors duration-300">
      <KeyboardShortcutsHandler onShowHelp={() => setShowShortcuts(true)} />
      <ShortcutsModal isOpen={showShortcuts} onClose={() => setShowShortcuts(false)} />

      {/* Animated background mesh */}
      <div className="absolute inset-0 opacity-30 pointer-events-none" style={{
        backgroundImage: `var(--gradient-mesh)`
      }} />

      {/* Sidebar */}
      <aside className="w-80 bg-[var(--sidebar-bg)] backdrop-blur-xl border-r border-[var(--border-color)] flex flex-col shadow-xl z-10 relative transition-colors duration-300">
        {/* Logo Section */}
        <div className="p-6 border-b border-[var(--border-color)] flex items-center gap-3 relative overflow-hidden">
          <div className="absolute inset-0 bg-gradient-to-r from-cyan-500/5 to-blue-500/5" />
          <div className="flex items-center justify-center relative">
            <img src="/logo.png" alt="Logo" className="w-10 h-10 object-contain drop-shadow-md transform hover:rotate-6 transition-transform" />
          </div>
          <h1 className="text-xl font-bold bg-gradient-to-r from-cyan-600 via-sky-600 to-blue-700 bg-clip-text text-transparent animate-slide-in-right">
            PDF Smart Editor
          </h1>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-6">
          {/* Navigation Menu */}
          <div className="space-y-2 animate-slide-in-left">
            <h2 className="text-xs font-semibold text-[var(--color-primary-600)] uppercase tracking-wider px-1">Navigation</h2>
            <button
              onClick={() => setActiveView('editor')}
              className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-xl transition-all duration-300 ${activeView === 'editor'
                ? 'bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white shadow-lg shadow-cyan-500/30 scale-105'
                : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-700)]'
                }`}
            >
              <FileText className="w-4 h-4" />
              <span className="text-sm font-medium">Editor</span>
            </button>
            <button
              onClick={() => setActiveView('manipulation')}
              className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-xl transition-all duration-300 ${activeView === 'manipulation'
                ? 'bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white shadow-lg shadow-cyan-500/30 scale-105'
                : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-700)]'
                }`}
            >
              <Scissors className="w-4 h-4" />
              <span className="text-sm font-medium">Manipulation</span>
            </button>
            <button
              onClick={() => setActiveView('conversion')}
              className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-xl transition-all duration-300 ${activeView === 'conversion'
                ? 'bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white shadow-lg shadow-cyan-500/30 scale-105'
                : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-700)]'
                }`}
            >
              <RefreshCw className="w-4 h-4" />
              <span className="text-sm font-medium">Conversion</span>
            </button>
            <button
              onClick={() => setActiveView('security')}
              className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-xl transition-all duration-300 ${activeView === 'security'
                ? 'bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white shadow-lg shadow-cyan-500/30 scale-105'
                : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-700)]'
                }`}
            >
              <Shield className="w-4 h-4" />
              <span className="text-sm font-medium">Security</span>
            </button>
            <button
              onClick={() => setActiveView('advanced')}
              className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-xl transition-all duration-300 ${activeView === 'advanced'
                ? 'bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white shadow-lg shadow-cyan-500/30 scale-105'
                : 'text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-700)]'
                }`}
            >
              <Zap className="w-4 h-4" />
              <span className="text-sm font-medium">Advanced</span>
            </button>
          </div>

          {activeView === 'editor' && (
            <div className="space-y-6 animate-fade-in border-t border-[var(--border-color)] pt-4">
              <div className="space-y-3">
                <h2 className="text-xs font-semibold text-[var(--color-primary-600)] uppercase tracking-wider px-1">Tools</h2>
                <Toolbar />
              </div>

              <div className="space-y-3">
                <h2 className="text-xs font-semibold text-[var(--color-primary-600)] uppercase tracking-wider px-1">Assets</h2>
                <ImageUpload />
              </div>
            </div>
          )}

          <div className="px-4 py-4 rounded-xl bg-gradient-to-br from-[var(--color-primary-50)] to-blue-50/50 border border-[var(--border-color)]">
            <div className="flex items-center gap-2 mb-2">
              <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
              <span className="text-xs font-semibold text-[var(--color-primary-700)]">System Status</span>
            </div>
            <p className="text-xs text-[var(--text-secondary)]">
              All systems operational.
              <br />
              Local processing active.
            </p>
          </div>
        </div>

        <div className="p-4 border-t border-[var(--border-color)] bg-[var(--sidebar-bg)] backdrop-blur-xl">
          <FileUpload />
          <RecentFiles />
          <div className="mt-4 flex items-center justify-between text-xs text-[var(--text-secondary)]">
            <span>© 2025 PDF Editor</span>
            <span className="hover:text-[var(--color-primary-600)] cursor-pointer transition-colors">v1.0.0</span>
          </div>
          <p className="text-xs text-center text-[var(--color-primary-600)] font-medium mt-2">
            Premium Edition ✨
          </p>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 relative flex flex-col">
        <Header />
        {renderContent()}
      </main>
    </div>
  );
}

// Wrapper to provide Editor context and Error Boundary
function App() {
  return (
    <ErrorBoundary>
      <EditorProvider>
        <AppContent />
      </EditorProvider>
    </ErrorBoundary>
  );
}


export default App;
