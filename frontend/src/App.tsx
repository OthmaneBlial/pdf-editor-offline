import { useState } from 'react';
import { EditorProvider, useEditor } from './contexts/EditorContext';
import PDFViewer from './components/PDFViewer';
import ShortcutsModal from './components/ShortcutsModal';
import ErrorBoundary from './components/ErrorBoundary';
import PageThumbnails from './components/PageThumbnails';
import Header from './components/Header';
import PageNavigation from './components/PageNavigation';
import ZoomControls from './components/ZoomControls';
import Sidebar from './components/Sidebar';
import ManipulationTools from './components/tools/ManipulationTools';
import ConversionTools from './components/tools/ConversionTools';
import SecurityTools from './components/tools/SecurityTools';
import AdvancedTools from './components/tools/AdvancedTools';
import BatchProcessingTools from './components/tools/BatchProcessingTools';
// Phase 4: Advanced Editing tools
import AdvancedTextTools from './components/tools/AdvancedTextTools';
import NavigationTools from './components/tools/NavigationTools';
import AnnotationTools from './components/tools/AnnotationTools';
import ImageTools from './components/tools/ImageTools';
import './App.css';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';

export type ViewMode = 'editor' | 'manipulation' | 'conversion' | 'security' | 'advanced' | 'batch' | 'text' | 'navigation' | 'annotations' | 'images';

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
  const { sessionId } = useEditor();

  const renderContent = () => {
    switch (activeView) {
      case 'editor':
        // Show empty state when no document is loaded
        if (!sessionId) {
          return (
            <div className="flex-1 flex items-center justify-center p-6 animate-fade-in">
              <div className="text-center max-w-md">
                <div className="w-20 h-20 bg-gradient-to-br from-sky-500 to-cyan-500 rounded-2xl flex items-center justify-center shadow-lg shadow-sky-500/20 mx-auto mb-6">
                  <svg className="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                </div>
                <h2 className="text-2xl font-bold text-white mb-2">No Document Loaded</h2>
                <p className="text-slate-400 mb-6">Upload a PDF document to start editing. You can use the file upload button in the sidebar.</p>
                <div className="flex items-center justify-center gap-2 text-sm text-slate-500">
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>Supported formats: PDF</span>
                </div>
              </div>
            </div>
          );
        }
        return (
          <div className="flex-1 relative overflow-auto p-6 animate-fade-in">
            <PageThumbnails />
            <div className="min-h-full bg-[var(--bg-canvas)] rounded-2xl shadow-xl border border-[var(--border-subtle)] relative overflow-hidden">
              {/* Decorative corner accent */}
              <div className="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-[var(--accent-primary)]/10 to-transparent pointer-events-none" />
              <div className="absolute bottom-0 left-0 w-24 h-24 bg-gradient-to-tr from-[var(--accent-tertiary)]/10 to-transparent pointer-events-none" />
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
      case 'batch':
        return <BatchProcessingTools />;
      case 'text':
        return <AdvancedTextTools />;
      case 'navigation':
        return <NavigationTools />;
      case 'annotations':
        return <AnnotationTools />;
      case 'images':
        return <ImageTools />;
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen w-full bg-[var(--bg-app)] overflow-hidden relative">
      <KeyboardShortcutsHandler onShowHelp={() => setShowShortcuts(true)} />
      <ShortcutsModal isOpen={showShortcuts} onClose={() => setShowShortcuts(false)} />

      {/* Gradient mesh background */}
      <div
        className="fixed inset-0 pointer-events-none opacity-60"
        style={{ backgroundImage: 'var(--gradient-mesh)' }}
      />

      {/* Tab-based Sidebar */}
      <Sidebar
        activeView={activeView}
        onViewChange={setActiveView}
        onShowShortcuts={() => setShowShortcuts(true)}
      />

      {/* Main Content */}
      <main className="flex-1 relative flex flex-col overflow-hidden">
        <Header />
        <div className="flex-1 overflow-auto">
          {renderContent()}
        </div>
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
