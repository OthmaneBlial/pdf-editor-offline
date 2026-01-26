import { useState } from 'react';
import {
  FileText, Scissors, RefreshCw, Shield, Zap, Wrench, History,
  MessageSquare, Keyboard, ChevronRight, Layers, Sparkles,
  Type, Bookmark, PenTool, ImageIcon
} from 'lucide-react';
import FileUpload from './FileUpload';
import RecentFiles from './RecentFiles';
import Toolbar from './Toolbar';
import HistoryPanel from './HistoryPanel';
import CollaborativeAnnotations from './CollaborativeAnnotations';
import ImageUpload from './ImageUpload';
import FullscreenButton from './FullscreenButton';
import type { ViewMode } from '../App';

// Navigation items
interface NavItem {
  id: ViewMode;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

// Basic tools
const basicNavItems: NavItem[] = [
  { id: 'editor', label: 'Editor', icon: FileText },
  { id: 'manipulation', label: 'Manipulation', icon: Scissors },
  { id: 'conversion', label: 'Conversion', icon: RefreshCw },
  { id: 'security', label: 'Security', icon: Shield },
  { id: 'advanced', label: 'Advanced', icon: Zap },
  { id: 'batch', label: 'Batch Process', icon: Layers },
];

// Advanced Editing tools
const advancedEditingNavItems: NavItem[] = [
  { id: 'text', label: 'Text Tools', icon: Type },
  { id: 'navigation', label: 'Navigation', icon: Bookmark },
  { id: 'annotations', label: 'Annotations', icon: PenTool },
  { id: 'images', label: 'Images', icon: ImageIcon },
];

interface SidebarProps {
  activeView: ViewMode;
  onViewChange: (view: ViewMode) => void;
  onShowShortcuts: () => void;
}

const Sidebar: React.FC<SidebarProps> = ({ activeView, onViewChange, onShowShortcuts }) => {
  // Track which section is expanded (only one at a time)
  const [expandedSection, setExpandedSection] = useState<'basic' | 'advanced' | 'tools' | 'history' | 'comments' | null>('basic');

  const toggleSection = (section: typeof expandedSection) => {
    setExpandedSection(expandedSection === section ? null : section);
  };

  // Helper to render navigation items
  const renderNavItems = (items: NavItem[]) => {
    return items.map((item) => {
      const Icon = item.icon;
      const isActive = activeView === item.id;
      return (
        <button
          key={item.id}
          onClick={() => onViewChange(item.id)}
          className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
            isActive
              ? 'bg-gradient-to-r from-sky-500 to-cyan-500 text-white shadow-lg'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800'
          }`}
        >
          <Icon className="w-4 h-4" />
          <span>{item.label}</span>
          {isActive && <ChevronRight className="w-3 h-3 ml-auto" />}
        </button>
      );
    });
  };

  return (
    <aside className="w-72 bg-slate-900 flex flex-col relative z-10">
      {/* Logo Section */}
      <div className="p-5 border-b border-slate-700/50">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-gradient-to-br from-sky-500 to-cyan-500 rounded-xl flex items-center justify-center shadow-lg shadow-sky-500/20">
            <FileText className="w-5 h-5 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-bold text-white tracking-tight">PDF Editor</h1>
            <p className="text-[10px] text-slate-400 font-medium">v2.0.0</p>
          </div>
        </div>
      </div>

      {/* Basic Tools Section */}
      <div className="border-b border-slate-700/50">
        <button
          onClick={() => toggleSection('basic')}
          className={`w-full flex items-center justify-between px-4 py-3 text-sm font-medium transition-colors ${
            expandedSection === 'basic'
              ? 'text-white bg-slate-800'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          }`}
        >
          <div className="flex items-center gap-2">
            <FileText className="w-4 h-4" />
            <span>Basic Tools</span>
          </div>
          <ChevronRight className={`w-4 h-4 transition-transform ${expandedSection === 'basic' ? 'rotate-90' : ''}`} />
        </button>

        {expandedSection === 'basic' && (
          <div className="px-2 pb-3 space-y-1 animate-fade-in">
            {renderNavItems(basicNavItems)}
          </div>
        )}
      </div>

      {/* Advanced Editing Section */}
      <div className="border-b border-slate-700/50">
        <button
          onClick={() => toggleSection('advanced')}
          className={`w-full flex items-center justify-between px-4 py-3 text-sm font-medium transition-colors ${
            expandedSection === 'advanced'
              ? 'text-white bg-slate-800'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          }`}
        >
          <div className="flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-purple-400" />
            <span>Advanced Editing</span>
          </div>
          <ChevronRight className={`w-4 h-4 transition-transform ${expandedSection === 'advanced' ? 'rotate-90' : ''}`} />
        </button>

        {expandedSection === 'advanced' && (
          <div className="px-2 pb-3 space-y-1 animate-fade-in">
            {renderNavItems(advancedEditingNavItems)}
          </div>
        )}
      </div>

      {/* Tools Section */}
      <div className="border-b border-slate-700/50">
        <button
          onClick={() => toggleSection('tools')}
          className={`w-full flex items-center justify-between px-4 py-3 text-sm font-medium transition-colors ${
            expandedSection === 'tools'
              ? 'text-white bg-slate-800'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          }`}
        >
          <div className="flex items-center gap-2">
            <Wrench className="w-4 h-4" />
            <span>Tools</span>
          </div>
          <ChevronRight className={`w-4 h-4 transition-transform ${expandedSection === 'tools' ? 'rotate-90' : ''}`} />
        </button>

        {expandedSection === 'tools' && (
          <div className="px-3 pb-4 animate-fade-in">
            <div className="mb-4">
              <Toolbar />
            </div>
            <div>
              <ImageUpload />
            </div>
          </div>
        )}
      </div>

      {/* History Section */}
      <div className="border-b border-slate-700/50">
        <button
          onClick={() => toggleSection('history')}
          className={`w-full flex items-center justify-between px-4 py-3 text-sm font-medium transition-colors ${
            expandedSection === 'history'
              ? 'text-white bg-slate-800'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          }`}
        >
          <div className="flex items-center gap-2">
            <History className="w-4 h-4" />
            <span>History</span>
          </div>
          <ChevronRight className={`w-4 h-4 transition-transform ${expandedSection === 'history' ? 'rotate-90' : ''}`} />
        </button>

        {expandedSection === 'history' && (
          <div className="px-3 pb-4 animate-fade-in">
            <HistoryPanel />
          </div>
        )}
      </div>

      {/* Comments Section */}
      <div className="flex-1 min-h-0">
        <button
          onClick={() => toggleSection('comments')}
          className={`w-full flex items-center justify-between px-4 py-3 text-sm font-medium transition-colors ${
            expandedSection === 'comments'
              ? 'text-white bg-slate-800'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          }`}
        >
          <div className="flex items-center gap-2">
            <MessageSquare className="w-4 h-4" />
            <span>Comments</span>
          </div>
          <ChevronRight className={`w-4 h-4 transition-transform ${expandedSection === 'comments' ? 'rotate-90' : ''}`} />
        </button>

        {expandedSection === 'comments' && (
          <div className="px-3 pb-4 h-full overflow-y-auto animate-fade-in">
            <CollaborativeAnnotations />
          </div>
        )}
      </div>

      {/* Bottom Section */}
      <div className="p-4 border-t border-slate-700/50 space-y-3">
        <FileUpload />
        <RecentFiles />

        {/* Status & Actions */}
        <div className="flex items-center justify-between px-2 py-2 rounded-lg bg-slate-800/50">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span className="text-[10px] font-medium text-slate-400">Ready</span>
          </div>
          <div className="flex items-center gap-1">
            <FullscreenButton />
            <button
              onClick={onShowShortcuts}
              className="p-1.5 rounded-lg hover:bg-slate-700 text-slate-400 hover:text-white transition-all"
              title="Keyboard shortcuts"
            >
              <Keyboard className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
