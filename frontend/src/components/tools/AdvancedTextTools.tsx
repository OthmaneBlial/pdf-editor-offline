import { useState } from 'react';
import axios from 'axios';
import {
  Type,
  Replace,
  Palette,
  Search,
  Code,
  Loader2,
  Save,
  Download,
  AlertTriangle,
  FlaskConical,
  ShieldCheck,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';
import { API_BASE_URL } from '../../lib/apiClient';
import WorkflowFeedback from '../WorkflowFeedback';

interface FontInfo {
  name: string;
  size: number;
  char_count: number;
  percentage: number;
  color?: number[];
}

interface SearchMatch {
  index: number;
  rect: number[];
  quad_points: number[][];
}

interface Message {
  type: 'success' | 'warning' | 'error';
  text: string;
}

interface ReplacementAssessment {
  status: 'eligible' | 'rejected';
  maturity: 'experimental';
  native_in_place_edit: false;
  implementation: 'redaction_plus_new_content_stream';
  rejection_reasons: string[];
  thresholds: {
    maximum_matches: number;
    maximum_replacement_width_ratio: number;
    maximum_target_render_change_ratio: number;
    maximum_unchanged_page_render_ratio: number;
  };
  geometry: {
    replacement_width_ratio: number | null;
  };
}

type RichInsertMode = 'html' | 'reflow' | 'textbox' | 'multifont';

const AdvancedTextTools: React.FC = () => {
  const { sessionId, currentPage, saveChanges, exportPDF, hasUnsavedChanges, reportToolResult } = useEditor();
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<Message | null>(null);

  // Text replacement state
  const [searchText, setSearchText] = useState('');
  const [replaceText, setReplaceText] = useState('');
  const [replacementAssessment, setReplacementAssessment] = useState<{
    fingerprint: string;
    report: ReplacementAssessment;
  } | null>(null);
  const [replacementAcknowledged, setReplacementAcknowledged] = useState(false);

  // Rich text state
  const [richText, setRichText] = useState('');
  const [richTextCss, setRichTextCss] = useState('');
  const [textBoxWidth, setTextBoxWidth] = useState(200);
  const [textBoxHeight, setTextBoxHeight] = useState(100);
  const [textBoxX, setTextBoxX] = useState(100);
  const [textBoxY, setTextBoxY] = useState(100);
  const [richInsertMode, setRichInsertMode] = useState<RichInsertMode>('html');

  // Font info state
  const [fonts, setFonts] = useState<FontInfo[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchMatches, setSearchMatches] = useState<SearchMatch[]>([]);

  const showMessage = (type: 'success' | 'warning' | 'error', text: string, refreshDocument = false) => {
    setMessage({ type, text });
    reportToolResult(type, text, refreshDocument);
    setTimeout(() => setMessage(null), 5000);
  };

  const handleTextReplace = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('replace');
    setMessage(null);
    const fingerprint = JSON.stringify([currentPage, searchText, replaceText]);

    try {
      if (replacementAssessment?.fingerprint !== fingerprint) {
        const preflight = await axios.post(
          `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/text/replace/preflight`,
          {
            page_num: currentPage,
            search_text: searchText,
            new_text: replaceText,
          },
        );
        const report = preflight.data.data as ReplacementAssessment;
        setReplacementAssessment({ fingerprint, report });
        setReplacementAcknowledged(false);
        if (report.status === 'eligible') {
          showMessage('warning', 'The bounded preflight passed. Review the implementation and thresholds, then explicitly acknowledge before applying.');
        } else {
          showMessage('error', `Unsupported content edit: ${report.rejection_reasons.map(reason => reason.replaceAll('_', ' ')).join('; ')}.`);
        }
        return;
      }
      if (replacementAssessment.report.status !== 'eligible' || !replacementAcknowledged) {
        showMessage('error', 'Review and acknowledge the experimental replacement contract first.');
        return;
      }
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/text/replace`,
        {
          page_num: currentPage,
          search_text: searchText,
          new_text: replaceText,
        }
      );

      if (response.data.success) {
        setReplacementAssessment(null);
        setReplacementAcknowledged(false);
        showMessage('success', 'Experimental replacement passed extraction, visual, semantic, and structural fidelity gates.', true);
      }
    } catch (error) {
      const detail = axios.isAxiosError(error) ? error.response?.data?.detail : undefined;
      const reasons = detail?.report?.rejection_reasons;
      showMessage('error', Array.isArray(reasons) ? `Replacement refused: ${reasons.join(', ').replaceAll('_', ' ')}.` : 'The experimental replacement was refused and the previous PDF was restored.');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleRichTextInsert = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('richtext');
    setMessage(null);

    try {
      const basePayload = {
        page_num: currentPage,
        x: textBoxX,
        y: textBoxY,
        width: textBoxWidth,
        height: textBoxHeight,
      };

      const endpointByMode: Record<RichInsertMode, string> = {
        html: 'rich',
        reflow: 'reflow',
        textbox: 'textbox',
        multifont: 'multifont',
      };

      const plainText = richText.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim() || 'Mixed font text';
      const payloadByMode = {
        html: {
          ...basePayload,
          html_content: richText,
          css: richTextCss || undefined,
        },
        reflow: {
          ...basePayload,
          html_content: richText,
        },
        textbox: {
          ...basePayload,
          text: plainText,
          border_color: [0, 0, 0],
          background_color: [1, 1, 1],
          font_size: 12,
          padding: 5,
        },
        multifont: {
          page_num: currentPage,
          x: textBoxX,
          y: textBoxY,
          fragments: [
            {
              text: plainText,
              font: 'Helvetica',
              size: 14,
              color: [0, 0, 0],
              bold: false,
              italic: false,
            },
            {
              text: '  styled',
              font: 'Helvetica',
              size: 16,
              color: [0.1, 0.35, 0.75],
              bold: true,
              italic: false,
            },
          ],
        },
      };

      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/text/${endpointByMode[richInsertMode]}`,
        payloadByMode[richInsertMode]
      );

      if (response.data.success) {
        showMessage('success', 'Text inserted successfully', true);
      }
    } catch (error) {
      showMessage('error', 'Failed to insert rich text');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleLoadFonts = async () => {
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('fonts');
    setMessage(null);

    try {
      const response = await axios.get(
        `${API_BASE_URL}/api/documents/${sessionId}/fonts/${currentPage}`
      );

      if (response.data.success) {
        setFonts(response.data.data.fonts || []);
        showMessage('success', `Loaded ${response.data.data.total_fonts} fonts`);
      }
    } catch (error) {
      showMessage('error', 'Failed to load fonts');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleSearchText = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('search');
    setMessage(null);

    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/text/search`,
        { text: searchQuery }
      );

      if (response.data.success) {
        const matches = response.data.data.matches || [];
        setSearchMatches(matches);
        showMessage(
          'success',
          `Found ${response.data.data.count ?? matches.length} occurrence(s)`
        );
      }
    } catch (error) {
      if (axios.isAxiosError(error)) {
        showMessage('error', error.response?.data?.detail || 'Failed to search text');
      } else {
        showMessage('error', 'Failed to search text');
      }
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const insertTemplate = (template: string) => {
    const templates: Record<string, { html: string; css: string }> = {
      header: {
        html: '<h1 style="color: #1a1a1a; font-size: 24px;">Header Text</h1>',
        css: '',
      },
      warning: {
        html: '<div style="background-color: #fff3cd; border-left: 4px solid #ffc107; padding: 10px;"><strong>Warning:</strong> This is a warning message.</div>',
        css: '',
      },
      info: {
        html: '<div style="background-color: #d1ecf1; border-left: 4px solid #17a2b8; padding: 10px;"><strong>Info:</strong> This is an info message.</div>',
        css: '',
      },
      callout: {
        html: '<div style="background-color: #f0f4ff; border: 1px solid #cce5ff; border-radius: 8px; padding: 15px;"><p style="margin: 0; color: #004085;"><strong>Note:</strong> This is a callout box.</p></div>',
        css: '',
      },
    };

    const selected = templates[template];
    if (selected) {
      setRichText(selected.html);
      setRichTextCss(selected.css);
    }
  };

  if (!sessionId) {
    return (
      <div className="p-3 sm:p-6 lg:p-8 max-w-7xl mx-auto animate-fade-in">
        <div className="text-center py-12 bg-[var(--card-bg)] rounded-xl border border-[var(--border-color)]">
          <Type className="w-12 h-12 mx-auto mb-4 text-[var(--text-secondary)]" />
          <p className="text-[var(--text-secondary)]">Upload a PDF to use advanced text tools</p>
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
              Advanced Text Tools
            </h2>
            <span className="tag">Page {currentPage + 1}</span>
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
          Experimental redaction-plus-redraw replacement, explicit text insertion, and font analysis
        </p>
      </div>

      {/* Status Message */}
      {message && (
        <WorkflowFeedback tone={message.type} title="Text operation result" className="mb-8">
          {message.text}
        </WorkflowFeedback>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Text Replacement */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
              <Replace className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Experimental Replace + Verify</h3>
              <p className="mt-0.5 font-mono text-[9px] font-black uppercase tracking-wider text-amber-700">Not native in-place editing</p>
            </div>
          </div>
          <div className="mb-4 rounded-xl border border-amber-300 bg-amber-50 p-3 text-xs leading-5 text-amber-950">
            <div className="flex gap-2"><AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" /><p>The original text is redacted and a new Base-14 text stream is drawn. Only one horizontal, isolated match that fits its source box can proceed.</p></div>
          </div>
          <form onSubmit={handleTextReplace} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Search Text
              </label>
              <input
                type="text"
                value={searchText}
                onChange={(e) => { setSearchText(e.target.value); setReplacementAssessment(null); setReplacementAcknowledged(false); }}
                placeholder="Text to find..."
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Replace With
              </label>
              <input
                type="text"
                value={replaceText}
                onChange={(e) => { setReplaceText(e.target.value); setReplacementAssessment(null); setReplacementAcknowledged(false); }}
                placeholder="New text..."
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              />
            </div>
            {replacementAssessment && (
              <div className={`rounded-xl border p-3 text-xs ${replacementAssessment.report.status === 'eligible' ? 'border-emerald-300 bg-emerald-50 text-emerald-950' : 'border-rose-300 bg-rose-50 text-rose-950'}`}>
                <div className="flex items-center gap-2 font-black">
                  {replacementAssessment.report.status === 'eligible' ? <ShieldCheck className="h-4 w-4" aria-hidden="true" /> : <AlertTriangle className="h-4 w-4" aria-hidden="true" />}
                  {replacementAssessment.report.status === 'eligible' ? 'Bounded input is eligible' : 'Unsupported structure or geometry'}
                </div>
                {replacementAssessment.report.status === 'eligible' ? (
                  <>
                    <p className="mt-2 leading-5">Post-edit gates require only this page and its extracted text to change, no structural-loss warning, and at most {(replacementAssessment.report.thresholds.maximum_target_render_change_ratio * 100).toFixed(0)}% changed target pixels.</p>
                    <label className="mt-3 flex min-h-11 items-center gap-3 rounded-lg border border-emerald-300 bg-white px-3 py-2 font-bold">
                      <input type="checkbox" checked={replacementAcknowledged} onChange={event => setReplacementAcknowledged(event.target.checked)} className="h-5 w-5" />
                      I understand this is experimental redaction plus redraw, not native text editing.
                    </label>
                  </>
                ) : <p className="mt-2 leading-5">{replacementAssessment.report.rejection_reasons.map(reason => reason.replaceAll('_', ' ')).join(' · ')}</p>}
              </div>
            )}
            <button
              type="submit"
              disabled={!!loading || (replacementAssessment?.report.status === 'eligible' && !replacementAcknowledged)}
              className="min-h-11 w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'replace' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Replacing...
                </>
              ) : (
                <>
                  {replacementAssessment?.report.status === 'eligible' ? <ShieldCheck className="h-4 w-4" /> : <FlaskConical className="h-4 w-4" />}
                  {replacementAssessment?.report.status === 'eligible' ? 'Apply + run fidelity gates' : 'Check experimental support'}
                </>
              )}
            </button>
          </form>
          <p className="text-xs text-[var(--text-secondary)] mt-3">
            Font substitution, rotated/skewed text, overlapping objects, tags, forms, signatures, layers, overflow, object transforms, and existing-paragraph reflow are refused.
          </p>
        </div>

        {/* Rich Text Insertion */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
              <Code className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Rich Text Insertion</h3>
          </div>

          {/* Template Buttons */}
          <div className="flex flex-wrap gap-2 mb-4">
            <button
              type="button"
              onClick={() => insertTemplate('header')}
              className="text-xs px-2 py-1 bg-gray-100 hover:bg-gray-200 rounded text-gray-700"
            >
              Header
            </button>
            <button
              type="button"
              onClick={() => insertTemplate('warning')}
              className="text-xs px-2 py-1 bg-yellow-100 hover:bg-yellow-200 rounded text-yellow-700"
            >
              Warning
            </button>
            <button
              type="button"
              onClick={() => insertTemplate('info')}
              className="text-xs px-2 py-1 bg-blue-100 hover:bg-blue-200 rounded text-blue-700"
            >
              Info
            </button>
            <button
              type="button"
              onClick={() => insertTemplate('callout')}
              className="text-xs px-2 py-1 bg-indigo-100 hover:bg-indigo-200 rounded text-indigo-700"
            >
              Callout
            </button>
          </div>

          <form onSubmit={handleRichTextInsert} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Insert Mode
              </label>
              <select
                value={richInsertMode}
                onChange={(e) => setRichInsertMode(e.target.value as RichInsertMode)}
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
              >
                <option value="html">HTML/CSS box</option>
                <option value="reflow">Story reflow</option>
                <option value="textbox">Bordered text box</option>
                <option value="multifont">Multi-font line</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                HTML Content
              </label>
              <textarea
                value={richText}
                onChange={(e) => setRichText(e.target.value)}
                placeholder="<p>HTML content here...</p>"
                rows={3}
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] font-mono text-sm"
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Custom CSS (optional)
              </label>
              <input
                type="text"
                value={richTextCss}
                onChange={(e) => setRichTextCss(e.target.value)}
                placeholder="body { font-family: Arial; }"
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] font-mono text-sm"
              />
            </div>
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">X</label>
                <input
                  type="number"
                  value={textBoxX}
                  onChange={(e) => setTextBoxX(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Y</label>
                <input
                  type="number"
                  value={textBoxY}
                  onChange={(e) => setTextBoxY(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Width</label>
                <input
                  type="number"
                  value={textBoxWidth}
                  onChange={(e) => setTextBoxWidth(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Height</label>
                <input
                  type="number"
                  value={textBoxHeight}
                  onChange={(e) => setTextBoxHeight(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
            </div>
            <button
              type="submit"
              disabled={!!loading}
              className="w-full py-2 px-4 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'richtext' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Inserting...
                </>
              ) : (
                <>
                  <Code className="w-4 h-4" /> Insert Rich Text
                </>
              )}
            </button>
          </form>
        </div>

        {/* Font Analysis */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-green-100 rounded-lg text-green-600">
              <Palette className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Font Analysis</h3>
          </div>
          <div className="space-y-4">
            <button
              onClick={handleLoadFonts}
              disabled={!!loading}
              className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'fonts' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Loading...
                </>
              ) : (
                <>
                  <Palette className="w-4 h-4" /> Load Page Fonts
                </>
              )}
            </button>

            {fonts.length > 0 && (
              <div className="mt-4 space-y-2 max-h-48 overflow-y-auto">
                {fonts.map((font, idx) => (
                  <div
                    key={idx}
                    className="p-2 bg-[var(--input-bg)] rounded text-sm flex justify-between items-center"
                  >
                    <div>
                      <span className="font-medium text-[var(--text-primary)]">{font.name}</span>
                      <span className="text-[var(--text-secondary)] ml-2">{font.size}pt</span>
                    </div>
                    <div className="text-right">
                      <span className="text-[var(--text-secondary)]">{font.char_count} chars</span>
                      <span className="text-[var(--accent-color)] ml-2">{font.percentage}%</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Text Search */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-orange-100 rounded-lg text-orange-600">
              <Search className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Text Search</h3>
          </div>
          <form onSubmit={handleSearchText} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Search Text
              </label>
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Text to search for..."
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              />
            </div>
            <button
              type="submit"
              disabled={!!loading}
              className="w-full py-2 px-4 bg-orange-600 hover:bg-orange-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'search' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Searching...
                </>
              ) : (
                <>
                  <Search className="w-4 h-4" /> Search
                </>
              )}
            </button>
          </form>
          {searchMatches.length > 0 && (
            <div className="mt-4 max-h-40 overflow-y-auto space-y-2">
              {searchMatches.map((match) => (
                <div
                  key={match.index}
                  className="p-2 rounded bg-[var(--input-bg)] text-xs text-[var(--text-primary)]"
                >
                  <p className="font-semibold">Match #{match.index + 1}</p>
                  <p className="text-[var(--text-secondary)]">
                    Rect: [{match.rect.map((v) => v.toFixed(1)).join(', ')}]
                  </p>
                </div>
              ))}
            </div>
          )}
          <p className="text-xs text-[var(--text-secondary)] mt-3">
            Find all occurrences of text with context preview
          </p>
        </div>
      </div>
    </div>
  );
};

export default AdvancedTextTools;
