import { useState, useEffect } from 'react';
import { EditorProvider } from './contexts/EditorContext';
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

// Phase 4 tools that need tabbed interface
const PHASE_4_TOOLS: ViewMode[] = ['text', 'navigation', 'annotations', 'images'];

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
  // For Phase 4 tools, track which tab is active: 'editor' or 'tool'
  const [activeTab, setActiveTab] = useState<'editor' | 'tool'>('editor');
  // Force refresh PDF viewer when switching from tool tab to editor tab
  const [refreshKey, setRefreshKey] = useState<number>(0);
  const isPhase4Tool = PHASE_4_TOOLS.includes(activeView);

  // Reset to tool tab when switching to a Phase 4 tool
  useEffect(() => {
    if (isPhase4Tool) {
      setActiveTab('tool');
    }
  }, [activeView, isPhase4Tool]);

  // When switching to editor tab from tool tab, refresh the PDF
  const handleTabSwitch = (tab: 'editor' | 'tool') => {
    if (tab === 'editor' && activeTab === 'tool') {
      // Coming from tool tab to editor tab - force refresh
      setRefreshKey(prev => prev + 1);
    }
    setActiveTab(tab);
  };

  const renderContent = () => {
    // For basic tools, replace the entire content
    if (!isPhase4Tool) {
      switch (activeView) {
        case 'editor':
          return (
            <div className="flex-1 relative overflow-auto p-6 animate-fade-in">
              <PageThumbnails />
              <div className="min-h-full bg-[var(--bg-canvas)] rounded-2xl shadow-xl border border-[var(--border-subtle)] relative overflow-hidden">
                {/* Decorative corner accent */}
                <div className="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-[var(--accent-primary)]/10 to-transparent pointer-events-none" />
                <div className="absolute bottom-0 left-0 w-24 h-24 bg-gradient-to-tr from-[var(--accent-tertiary)]/10 to-transparent pointer-events-none" />
                <PDFViewer forceRefresh={refreshKey} />
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
        default:
          return null;
      }
    }

    // For Phase 4 tools, render tabs with Editor and Tool views
    const toolComponent = (() => {
      switch (activeView) {
        case 'text': return <AdvancedTextTools />;
        case 'navigation': return <NavigationTools />;
        case 'annotations': return <AnnotationTools />;
        case 'images': return <ImageTools />;
        default: return null;
      }
    })();

    return (
      <div className="flex flex-col h-full animate-fade-in">
        {/* Tabs Header */}
        <div className="flex items-center gap-2 px-4 py-2 bg-slate-900 border-b border-slate-700">
          <button
            onClick={() => handleTabSwitch('editor')}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all ${
              activeTab === 'editor'
                ? 'bg-gradient-to-r from-sky-500 to-cyan-500 text-white shadow-lg'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            Editor View
          </button>
          <button
            onClick={() => handleTabSwitch('tool')}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all ${
              activeTab === 'tool'
                ? 'bg-gradient-to-r from-violet-500 to-purple-500 text-white shadow-lg'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L3 12l5.714-2.143L9 3z" />
            </svg>
            {activeView === 'text' && 'Text Tools'}
            {activeView === 'navigation' && 'Navigation'}
            {activeView === 'annotations' && 'Annotations'}
            {activeView === 'images' && 'Images'}
          </button>
        </div>

        {/* Tab Content */}
        <div className="flex-1 overflow-auto">
          {activeTab === 'editor' ? (
            <div className="p-6">
              <PageThumbnails />
              <div className="min-h-full bg-[var(--bg-canvas)] rounded-2xl shadow-xl border border-[var(--border-subtle)] relative overflow-hidden">
                {/* Decorative corner accent */}
                <div className="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-[var(--accent-primary)]/10 to-transparent pointer-events-none" />
                <div className="absolute bottom-0 left-0 w-24 h-24 bg-gradient-to-tr from-[var(--accent-tertiary)]/10 to-transparent pointer-events-none" />
                <PDFViewer forceRefresh={refreshKey} />
                <PageNavigation />
                <ZoomControls />
              </div>
            </div>
          ) : (
            toolComponent
          )}
        </div>
      </div>
    );
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
