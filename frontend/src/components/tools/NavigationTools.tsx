import { useState, useEffect } from 'react';
import axios from 'axios';
import {
  List,
  Bookmark,
  Plus,
  Trash2,
  Link as LinkIcon,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  CheckCircle,
  AlertCircle,
  Loader2,
  Save,
  Download,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

interface TOCItem {
  level: number;
  title: string;
  page: number;
  has_link?: boolean;
  link_type?: string;
}

interface LinkItem {
  index: number;
  type: string;
  uri?: string;
  dest_page?: number;
  rect: number[];
}

interface Message {
  type: 'success' | 'error';
  text: string;
}

const NavigationTools: React.FC = () => {
  const { sessionId, currentPage, setCurrentPage, saveChanges, exportPDF, hasUnsavedChanges, pageCount } = useEditor();
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<Message | null>(null);

  // TOC state
  const [toc, setToc] = useState<TOCItem[]>([]);
  const [expandedItems, setExpandedItems] = useState<Set<number>>(new Set());

  // Bookmark state
  const [bookmarks, setBookmarks] = useState<TOCItem[]>([]);
  const [newBookmarkTitle, setNewBookmarkTitle] = useState('');
  const [newBookmarkLevel, setNewBookmarkLevel] = useState(1);

  // Link state
  const [links, setLinks] = useState<LinkItem[]>([]);

  const showMessage = (type: 'success' | 'error', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 5000);
  };

  const loadTOC = async () => {
    if (!sessionId) return;

    setLoading('toc');
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/toc`);
      if (response.data.success) {
        setToc(response.data.data.toc || []);
      }
    } catch (error) {
      showMessage('error', 'Failed to load TOC');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const loadBookmarks = async () => {
    if (!sessionId) return;

    setLoading('bookmarks');
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/toc`);
      if (response.data.success) {
        setBookmarks(response.data.data.toc || []);
      }
    } catch (error) {
      showMessage('error', 'Failed to load bookmarks');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const loadLinks = async () => {
    if (!sessionId) return;

    setLoading('links');
    try {
      const response = await axios.get(
        `${API_BASE_URL}/api/documents/${sessionId}/links/${currentPage}`
      );
      if (response.data.success) {
        setLinks(response.data.data.links || []);
      }
    } catch (error) {
      showMessage('error', 'Failed to load links');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleAddBookmark = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) return;

    setLoading('add-bookmark');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/bookmarks`,
        null,
        {
          params: {
            level: newBookmarkLevel,
            title: newBookmarkTitle,
            page_num: currentPage + 1,
          },
        }
      );

      if (response.data.success) {
        showMessage('success', 'Bookmark added');
        setNewBookmarkTitle('');
        loadBookmarks();
      }
    } catch (error) {
      showMessage('error', 'Failed to add bookmark');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleDeleteBookmark = async (index: number) => {
    if (!sessionId) return;

    setLoading('delete-bookmark');
    try {
      const response = await axios.delete(
        `${API_BASE_URL}/api/documents/${sessionId}/bookmarks/${index}`
      );

      if (response.data.success) {
        showMessage('success', 'Bookmark deleted');
        loadBookmarks();
      }
    } catch (error) {
      showMessage('error', 'Failed to delete bookmark');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleDeleteLink = async (linkIndex: number) => {
    if (!sessionId) return;

    setLoading('delete-link');
    try {
      const response = await axios.delete(
        `${API_BASE_URL}/api/documents/${sessionId}/links/${currentPage}/${linkIndex}`
      );

      if (response.data.success) {
        showMessage('success', 'Link deleted');
        loadLinks();
      }
    } catch (error) {
      showMessage('error', 'Failed to delete link');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const toggleExpanded = (index: number) => {
    const newExpanded = new Set(expandedItems);
    if (newExpanded.has(index)) {
      newExpanded.delete(index);
    } else {
      newExpanded.add(index);
    }
    setExpandedItems(newExpanded);
  };

  const navigateToPage = (page: number) => {
    setCurrentPage(page - 1); // Convert to 0-indexed
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const renderTOCItem = (item: TOCItem, index: number, indent: number = 0) => {
    const hasChildren = toc.slice(index + 1).some(
      (child) => child.level > item.level
    );
    const isExpanded = expandedItems.has(index);

    return (
      <div key={index}>
        <div
          className="flex items-center gap-2 py-1 px-2 hover:bg-[var(--input-bg)] rounded cursor-pointer"
          style={{ paddingLeft: `${indent * 16 + 8}px` }}
        >
          {hasChildren && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleExpanded(index);
              }}
              className="p-1 hover:bg-[var(--border-color)] rounded"
            >
              {isExpanded ? (
                <ChevronDown className="w-4 h-4 text-[var(--text-secondary)]" />
              ) : (
                <ChevronRight className="w-4 h-4 text-[var(--text-secondary)]" />
              )}
            </button>
          )}
          <button
            onClick={() => navigateToPage(item.page)}
            className="flex-1 text-left py-1 px-2 hover:bg-[var(--accent-color)]/10 rounded text-[var(--text-primary)] text-sm"
          >
            {item.title}
          </button>
          <span className="text-xs text-[var(--text-secondary)]">p.{item.page}</span>
        </div>
        {hasChildren && isExpanded && (
          <div>
            {toc.slice(index + 1).map((child, childIdx) => {
              const actualIdx = index + 1 + childIdx;
              if (child.level <= item.level) return null;
              if (childIdx > 0 && toc[actualIdx - 1].level > child.level) return null;
              return renderTOCItem(child, actualIdx, indent + 1);
            })}
          </div>
        )}
      </div>
    );
  };

  useEffect(() => {
    if (sessionId) {
      loadTOC();
      loadLinks();
    }
  }, [sessionId, currentPage]);

  if (!sessionId) {
    return (
      <div className="p-3 sm:p-6 lg:p-8 max-w-7xl mx-auto animate-fade-in">
        <div className="text-center py-12 bg-[var(--card-bg)] rounded-xl border border-[var(--border-color)]">
          <List className="w-12 h-12 mx-auto mb-4 text-[var(--text-secondary)]" />
          <p className="text-[var(--text-secondary)]">Upload a PDF to use navigation tools</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-3 sm:p-6 lg:p-8 max-w-7xl mx-auto animate-fade-in">
      {/* Page Header */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-3">
            <h2 className="font-display text-3xl font-bold text-[var(--text-primary)]">
              Navigation Tools
            </h2>
            <span className="tag">Page {currentPage + 1} / {pageCount}</span>
            {hasUnsavedChanges && (
              <span className="px-2 py-1 text-xs font-medium bg-amber-100 text-amber-700 rounded-full">
                Unsaved changes
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => saveChanges()}
              className="flex items-center gap-2 px-4 py-2 bg-sky-500 hover:bg-sky-600 text-white rounded-lg font-medium text-sm transition-colors"
            >
              <Save className="w-4 h-4" />
              Save
            </button>
            <button
              onClick={() => exportPDF()}
              className="flex items-center gap-2 px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg font-medium text-sm transition-colors"
            >
              <Download className="w-4 h-4" />
              Export PDF
            </button>
          </div>
        </div>
        <p className="text-sm text-[var(--text-secondary)] font-body">
          Manage table of contents, bookmarks, and hyperlinks
        </p>
      </div>

      {/* Status Message */}
      {message && (
        <div
          className={`p-4 mb-8 rounded-xl font-body text-sm flex items-center gap-3 animate-slide-up ${
            message.type === 'success'
              ? 'bg-[var(--status-success)]/10 text-[var(--status-success)] border border-[var(--status-success)]/20'
              : 'bg-[var(--status-error)]/10 text-[var(--status-error)] border border-[var(--status-error)]/20'
          }`}
        >
          {message.type === 'success' ? (
            <CheckCircle className="w-5 h-5 flex-shrink-0" />
          ) : (
            <AlertCircle className="w-5 h-5 flex-shrink-0" />
          )}
          <span className="font-medium">{message.text}</span>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Table of Contents */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
                <List className="w-6 h-6" />
              </div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Table of Contents</h3>
            </div>
            <button
              onClick={loadTOC}
              disabled={loading === 'toc'}
              className="p-2 hover:bg-[var(--input-bg)] rounded"
            >
              {loading === 'toc' ? (
                <Loader2 className="w-4 h-4 animate-spin text-[var(--text-secondary)]" />
              ) : (
                <Loader2 className="w-4 h-4 text-[var(--text-secondary)]" />
              )}
            </button>
          </div>

          <div className="max-h-80 overflow-y-auto">
            {toc.length > 0 ? (
              toc.map((item, index) => renderTOCItem(item, index))
            ) : (
              <p className="text-[var(--text-secondary)] text-sm py-4 text-center">
                No table of contents found
              </p>
            )}
          </div>

          <button className="w-full mt-4 py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors text-sm">
            Auto-Generate from Headers
          </button>
        </div>

        {/* Bookmarks */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-green-100 rounded-lg text-green-600">
              <Bookmark className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Bookmarks</h3>
          </div>

          <form onSubmit={handleAddBookmark} className="space-y-3 mb-4">
            <div>
              <input
                type="text"
                value={newBookmarkTitle}
                onChange={(e) => setNewBookmarkTitle(e.target.value)}
                placeholder="Bookmark title..."
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                required
              />
            </div>
            <div className="flex gap-2">
              <select
                value={newBookmarkLevel}
                onChange={(e) => setNewBookmarkLevel(Number(e.target.value))}
                className="flex-1 p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
              >
                <option value={1}>Level 1</option>
                <option value={2}>Level 2</option>
                <option value={3}>Level 3</option>
              </select>
              <button
                type="submit"
                disabled={loading === 'add-bookmark'}
                className="px-4 py-2 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50"
              >
                {loading === 'add-bookmark' ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Plus className="w-4 h-4" />
                )}
              </button>
            </div>
          </form>

          <div className="max-h-48 overflow-y-auto space-y-1">
            {bookmarks.length > 0 ? (
              bookmarks.map((item, index) => (
                <div
                  key={index}
                  className="flex items-center gap-2 p-2 bg-[var(--input-bg)] rounded group"
                  style={{ paddingLeft: `${item.level * 12 + 8}px` }}
                >
                  <button
                    onClick={() => navigateToPage(item.page)}
                    className="flex-1 text-left text-sm text-[var(--text-primary)] hover:text-[var(--accent-color)]"
                  >
                    {item.title}
                  </button>
                  <span className="text-xs text-[var(--text-secondary)]">{item.page}</span>
                  <button
                    onClick={() => handleDeleteBookmark(index)}
                    className="p-1 opacity-0 group-hover:opacity-100 hover:bg-red-100 rounded text-red-500"
                  >
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              ))
            ) : (
              <p className="text-[var(--text-secondary)] text-sm py-4 text-center">
                No bookmarks yet
              </p>
            )}
          </div>
        </div>

        {/* Links */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
                <LinkIcon className="w-6 h-6" />
              </div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Page Links</h3>
            </div>
            <button
              onClick={loadLinks}
              disabled={loading === 'links'}
              className="p-2 hover:bg-[var(--input-bg)] rounded"
            >
              {loading === 'links' ? (
                <Loader2 className="w-4 h-4 animate-spin text-[var(--text-secondary)]" />
              ) : (
                <Loader2 className="w-4 h-4 text-[var(--text-secondary)]" />
              )}
            </button>
          </div>

          <div className="max-h-96 overflow-y-auto space-y-2">
            {links.length > 0 ? (
              links.map((link) => (
                <div
                  key={link.index}
                  className="p-3 bg-[var(--input-bg)] rounded group"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="text-xs font-medium px-2 py-0.5 bg-purple-100 text-purple-700 rounded">
                          {link.type}
                        </span>
                      </div>
                      {link.uri && (
                        <a
                          href={link.uri}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-sm text-[var(--accent-color)] hover:underline flex items-center gap-1"
                        >
                          {link.uri.length > 40 ? link.uri.substring(0, 40) + '...' : link.uri}
                          <ExternalLink className="w-3 h-3 flex-shrink-0" />
                        </a>
                      )}
                      {link.dest_page !== undefined && (
                        <button
                          onClick={() => navigateToPage(link.dest_page! + 1)}
                          className="text-sm text-[var(--accent-color)] hover:underline"
                        >
                          Goes to page {link.dest_page + 1}
                        </button>
                      )}
                    </div>
                    <button
                      onClick={() => handleDeleteLink(link.index)}
                      className="p-1 opacity-0 group-hover:opacity-100 hover:bg-red-100 rounded text-red-500"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              ))
            ) : (
              <p className="text-[var(--text-secondary)] text-sm py-4 text-center">
                No links on this page
              </p>
            )}
          </div>

          <button className="w-full mt-4 py-2 px-4 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition-colors text-sm flex items-center justify-center gap-2">
            <Plus className="w-4 h-4" /> Add New Link
          </button>
        </div>
      </div>
    </div>
  );
};

export default NavigationTools;
