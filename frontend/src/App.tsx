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
import HistoryPanel from './components/HistoryPanel';
import FullscreenButton from './components/FullscreenButton';
import CollaborativeAnnotations from './components/CollaborativeAnnotations';
import { FileText, Scissors, RefreshCw, Shield, Zap, Sparkles, ChevronRight, Keyboard } from 'lucide-react';
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

// Navigation item component for cleaner code
function NavItem({
  active,
  onClick,
  icon: Icon,
  label,
  delay = 0
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  delay?: number;
}) {
  return (
    <button
      onClick={onClick}
      style={{ animationDelay: `${delay}ms` }}
      className={`
        group w-full flex items-center gap-3 px-4 py-3 rounded-lg font-display text-sm font-medium
        transition-all duration-200 animate-slide-in-left
        ${active
          ? 'bg-[var(--accent-primary)] text-[var(--neutral-950)] shadow-lg'
          : 'text-[var(--sidebar-text-muted)] hover:text-[var(--sidebar-text)] hover:bg-white/5'
        }
      `}
    >
      <Icon className={`w-4 h-4 transition-transform duration-200 ${active ? '' : 'group-hover:scale-110'}`} />
      <span className="flex-1 text-left">{label}</span>
      <ChevronRight className={`w-3 h-3 transition-all duration-200 ${active ? 'opacity-100' : 'opacity-0 group-hover:opacity-50'}`} />
    </button>
  );
}

// Inner component with access to EditorContext
function AppContent() {
  const [activeView, setActiveView] = useState<ViewMode>('editor');
  const [showShortcuts, setShowShortcuts] = useState(false);

  const renderContent = () => {
    switch (activeView) {
      case 'editor':
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

      {/* Dark Sidebar - Neo-Brutalist */}
      <aside className="w-72 bg-[var(--sidebar-bg)] flex flex-col relative z-10 border-r border-[var(--sidebar-border)]">
        {/* Logo Section */}
        <div className="p-6 border-b border-[var(--sidebar-border)]">
          <div className="flex items-center gap-3">
            <div className="relative">
              <img
                src="/logo.png"
                alt="Logo"
                className="w-10 h-10 object-contain drop-shadow-lg animate-float"
              />
              <div className="absolute -inset-1 bg-[var(--accent-primary)]/20 rounded-full blur-md -z-10" />
            </div>
            <div>
              <h1 className="font-display text-lg font-bold text-[var(--sidebar-text)] tracking-tight">
                PDF Smart Editor
              </h1>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="tag tag-new">PRO</span>
                <span className="text-[10px] text-[var(--sidebar-text-muted)] font-mono">v1.0.0</span>
              </div>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <div className="flex-1 overflow-y-auto p-4">
          <div className="space-y-1">
            <p className="px-4 py-2 text-[10px] font-display font-bold uppercase tracking-widest text-[var(--sidebar-text-muted)]">
              Navigation
            </p>
            <NavItem
              active={activeView === 'editor'}
              onClick={() => setActiveView('editor')}
              icon={FileText}
              label="Editor"
              delay={50}
            />
            <NavItem
              active={activeView === 'manipulation'}
              onClick={() => setActiveView('manipulation')}
              icon={Scissors}
              label="Manipulation"
              delay={100}
            />
            <NavItem
              active={activeView === 'conversion'}
              onClick={() => setActiveView('conversion')}
              icon={RefreshCw}
              label="Conversion"
              delay={150}
            />
            <NavItem
              active={activeView === 'security'}
              onClick={() => setActiveView('security')}
              icon={Shield}
              label="Security"
              delay={200}
            />
            <NavItem
              active={activeView === 'advanced'}
              onClick={() => setActiveView('advanced')}
              icon={Zap}
              label="Advanced"
              delay={250}
            />
          </div>

          {/* Editor Tools Section */}
          {activeView === 'editor' && (
            <div className="mt-6 space-y-4 animate-fade-in">
              <div className="h-px bg-gradient-to-r from-transparent via-[var(--sidebar-border)] to-transparent" />

              <div>
                <p className="px-4 py-2 text-[10px] font-display font-bold uppercase tracking-widest text-[var(--sidebar-text-muted)]">
                  Quick Tools
                </p>
                <div className="px-2">
                  <Toolbar />
                </div>
              </div>

              <div>
                <p className="px-4 py-2 text-[10px] font-display font-bold uppercase tracking-widest text-[var(--sidebar-text-muted)]">
                  Add Assets
                </p>
                <div className="px-2">
                  <ImageUpload />
                </div>
              </div>

              <div>
                <div className="flex items-center justify-between px-4 py-2">
                  <p className="text-[10px] font-display font-bold uppercase tracking-widest text-[var(--sidebar-text-muted)]">
                    History
                  </p>
                  <FullscreenButton />
                </div>
                <div className="px-2">
                  <HistoryPanel />
                </div>
              </div>

              <div>
                <p className="px-4 py-2 text-[10px] font-display font-bold uppercase tracking-widest text-[var(--sidebar-text-muted)]">
                  Comments
                </p>
                <div className="px-2">
                  <CollaborativeAnnotations />
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Bottom Section */}
        <div className="p-4 border-t border-[var(--sidebar-border)] space-y-3">
          {/* File Upload */}
          <FileUpload />
          <RecentFiles />

          {/* Status Bar */}
          <div className="flex items-center justify-between px-2 py-2 rounded-lg bg-white/5">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-[var(--status-success)] animate-pulse" />
              <span className="text-[10px] font-mono text-[var(--sidebar-text-muted)]">
                All systems operational
              </span>
            </div>
            <button
              onClick={() => setShowShortcuts(true)}
              className="p-1.5 rounded hover:bg-white/10 transition-colors group"
              title="Keyboard shortcuts"
            >
              <Keyboard className="w-3.5 h-3.5 text-[var(--sidebar-text-muted)] group-hover:text-[var(--accent-primary)]" />
            </button>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between text-[10px] text-[var(--sidebar-text-muted)]">
            <span className="font-mono">© 2025</span>
            <div className="flex items-center gap-1">
              <Sparkles className="w-3 h-3 text-[var(--accent-primary)]" />
              <span className="font-display font-semibold text-[var(--accent-primary)]">Premium</span>
            </div>
          </div>
        </div>
      </aside>

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
