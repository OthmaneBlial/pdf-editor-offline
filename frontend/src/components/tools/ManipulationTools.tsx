import { useState } from 'react';
import axios from 'axios';
import { FilePlus, Scissors, Layers, RotateCw, Hash, Download, Copy, Maximize2, Crop, FileInput, RefreshCw, Trash2, FileType, AlignCenter } from 'lucide-react';
import { API_DEFAULTS, FILENAMES } from '../../constants';

const API_BASE_URL = API_DEFAULTS.BASE_URL;

const PAGE_FORMATS = [
  { value: 'A4', label: 'A4 (595×842)' },
  { value: 'Letter', label: 'Letter (612×792)' },
  { value: 'Legal', label: 'Legal (612×1008)' },
  { value: 'A3', label: 'A3 (842×1191)' },
  { value: 'A5', label: 'A5 (420×595)' },
  { value: 'Tabloid', label: 'Tabloid (792×1224)' },
];

const NUMBERING_FORMATS = [
  { value: 'arabic', label: '1, 2, 3...' },
  { value: 'roman', label: 'I, II, III...' },
  { value: 'roman_lower', label: 'i, ii, iii...' },
  { value: 'letter', label: 'A, B, C...' },
  { value: 'letter_lower', label: 'a, b, c...' },
];

const ManipulationTools: React.FC = () => {
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const handleMerge = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('merge');
    setMessage(null);
    const formData = new FormData(e.currentTarget);

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/merge`, formData, {
        responseType: 'blob'
      });
      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'merged.pdf';
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'PDFs merged successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to merge PDFs.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleSplit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('split');
    setMessage(null);
    const formData = new FormData(e.currentTarget);

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/split`, formData, {
        responseType: 'blob'
      });
      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'split.zip';
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'PDF split successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to split PDF.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleOrganize = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('organize');
    setMessage(null);
    const formData = new FormData(e.currentTarget);
    const pageOrderStr = formData.get('page_order') as string;
    if (pageOrderStr) {
      const orderList = pageOrderStr.split(',').map(num => parseInt(num.trim()));
      formData.set('page_order', JSON.stringify(orderList));
    }

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/organize`, formData, {
        responseType: 'blob'
      });
      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'organized.pdf';
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'PDF organized successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to organize PDF.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleRotate = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('rotate');
    setMessage(null);
    const formData = new FormData(e.currentTarget);

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/rotate`, formData, {
        responseType: 'blob'
      });
      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'rotated.pdf';
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'PDF rotated successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to rotate PDF.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handlePageNumbers = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('page-numbers');
    setMessage(null);
    const formData = new FormData(e.currentTarget);

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/page-numbers`, formData, {
        responseType: 'blob'
      });
      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'numbered.pdf';
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'Page numbers added successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to add page numbers.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  // New Page Manipulation Handlers
  const handleExtract = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('extract');
    setMessage(null);
    const formData = new FormData(e.currentTarget);
    const pagesStr = formData.get('pages') as string;
    const pages = pagesStr.split(',').map(p => parseInt(p.trim()) - 1); // Convert to 0-indexed

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/split`, formData, {
        responseType: 'blob'
      });
      const filename = FILENAMES.EXTRACTED_PAGES;
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: `Extracted ${pages.length} page(s) successfully!` });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to extract pages.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleInsert = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('insert');
    setMessage(null);
    const formData = new FormData(e.currentTarget);
    const files = (e.currentTarget.elements.namedItem('files') as HTMLInputElement).files;

    if (!files || files.length < 2) {
      setMessage({ type: 'error', text: 'Please select at least 2 PDF files.' });
      return;
    }

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/merge`, formData, {
        responseType: 'blob'
      });
      const filename = FILENAMES.MERGED_WITH_INSERT;
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: 'Pages inserted successfully!' });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to insert pages.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleDuplicate = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading('duplicate');
    setMessage(null);
    const formData = new FormData(e.currentTarget);
    const pageNum = formData.get('page_num') as string;
    const count = parseInt(formData.get('count') as string) || 1;

    // Create page order with duplicates
    const pageOrder = [];
    for (let i = 1; i <= 10; i++) { // Assume max 10 pages
      pageOrder.push(i);
      if (i === parseInt(pageNum)) {
        for (let j = 0; j < count; j++) {
          pageOrder.push(i);
        }
      }
    }
    formData.set('page_order', JSON.stringify(pageOrder.slice(0, pageOrder.length)));

    try {
      const response = await axios.post(`${API_BASE_URL}/api/tools/organize`, formData, {
        responseType: 'blob'
      });
      const filename = FILENAMES.DUPLICATED;
      downloadFile(response.data, filename);
      setMessage({ type: 'success', text: `Page ${pageNum} duplicated ${count} time(s)!` });
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to duplicate page.' });
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const downloadFile = (data: Blob, filename: string) => {
    const url = window.URL.createObjectURL(data);
    const link = document.createElement('a');
    link.href = url;
    link.setAttribute('download', filename);
    document.body.appendChild(link);
    link.click();
    link.remove();
  };

  return (
    <div className="p-8 max-w-7xl mx-auto animate-fade-in">
      {/* Page Header */}
      <div className="mb-8">
        <div className="flex items-center gap-3 mb-2">
          <h2 className="font-display text-3xl font-bold text-[var(--text-primary)]">
            Manipulation Tools
          </h2>
          <span className="tag">14 Tools</span>
        </div>
        <p className="text-sm text-[var(--text-secondary)] font-body">
          Merge, split, organize, and manipulate PDF pages with precision
        </p>
      </div>

      {/* Status Message */}
      {message && (
        <div className={`p-4 mb-8 rounded-xl font-body text-sm flex items-center gap-3 animate-slide-up ${message.type === 'success'
            ? 'bg-[var(--status-success)]/10 text-[var(--status-success)] border border-[var(--status-success)]/20'
            : 'bg-[var(--status-error)]/10 text-[var(--status-error)] border border-[var(--status-error)]/20'
          }`}>
          <span className="font-medium">{message.text}</span>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {/* Merge PDF */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
              <FilePlus className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Merge PDF</h3>
          </div>
          <form onSubmit={handleMerge} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDFs</label>
              <input type="file" name="files" multiple accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100" />
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'merge' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Layers className="w-4 h-4" /> Merge Files</>}
            </button>
          </form>
        </div>

        {/* Split PDF */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-orange-100 rounded-lg text-orange-600">
              <Scissors className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Split PDF</h3>
          </div>
          <form onSubmit={handleSplit} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-orange-50 file:text-orange-700 hover:file:bg-orange-100" />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Page Ranges</label>
              <input type="text" name="page_ranges" placeholder="e.g., 1-3,5,7-9" required className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]" />
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-orange-600 hover:bg-orange-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'split' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Download className="w-4 h-4" /> Split PDF</>}
            </button>
          </form>
        </div>

        {/* Extract Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-cyan-100 rounded-lg text-cyan-600">
              <FileInput className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Extract Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <form onSubmit={handleExtract} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-cyan-50 file:text-cyan-700 hover:file:bg-cyan-100" />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Pages to Extract</label>
              <input type="text" name="page_ranges" placeholder="e.g., 1,3,5-7" required className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]" />
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-cyan-600 hover:bg-cyan-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'extract' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><FileInput className="w-4 h-4" /> Extract Pages</>}
            </button>
          </form>
        </div>

        {/* Insert Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-indigo-100 rounded-lg text-indigo-600">
              <FilePlus className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Insert Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <form onSubmit={handleInsert} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Select PDFs (in order)</label>
              <input type="file" name="files" multiple accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-indigo-50 file:text-indigo-700 hover:file:bg-indigo-100" />
              <p className="text-xs text-[var(--text-secondary)] mt-1">First file is base, others are inserted</p>
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'insert' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><FilePlus className="w-4 h-4" /> Insert Pages</>}
            </button>
          </form>
        </div>

        {/* Duplicate Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-pink-100 rounded-lg text-pink-600">
              <Copy className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Duplicate Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <form onSubmit={handleDuplicate} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-pink-50 file:text-pink-700 hover:file:bg-pink-100" />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Page #</label>
                <input type="number" name="page_num" min="1" placeholder="1" required className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]" />
              </div>
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Copies</label>
                <input type="number" name="count" min="1" max="10" defaultValue="1" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]" />
              </div>
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-pink-600 hover:bg-pink-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'duplicate' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Copy className="w-4 h-4" /> Duplicate</>}
            </button>
          </form>
        </div>

        {/* Resize Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-teal-100 rounded-lg text-teal-600">
              <Maximize2 className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Resize Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Target Format</label>
              <select className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]">
                {PAGE_FORMATS.map(f => (
                  <option key={f.value} value={f.value}>{f.label}</option>
                ))}
              </select>
            </div>
            <p className="text-xs text-[var(--text-secondary)]">
              Open a PDF in the editor and use page tools to resize individual pages.
            </p>
            <button disabled className="w-full py-2 px-4 bg-teal-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <Maximize2 className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>

        {/* Crop Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-amber-100 rounded-lg text-amber-600">
              <Crop className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Crop Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Left (pt)</label>
                <input type="number" placeholder="0" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Right (pt)</label>
                <input type="number" placeholder="0" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Top (pt)</label>
                <input type="number" placeholder="0" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Bottom (pt)</label>
                <input type="number" placeholder="0" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
            </div>
            <p className="text-xs text-[var(--text-secondary)]">72 points = 1 inch. Use in editor for precise cropping.</p>
            <button disabled className="w-full py-2 px-4 bg-amber-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <Crop className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>

        {/* Organize PDF */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
              <Layers className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Organize PDF</h3>
          </div>
          <form onSubmit={handleOrganize} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-purple-50 file:text-purple-700 hover:file:bg-purple-100" />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">New Page Order</label>
              <input type="text" name="page_order" placeholder="e.g., 3,1,2" required className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]" />
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'organize' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Layers className="w-4 h-4" /> Reorder Pages</>}
            </button>
          </form>
        </div>

        {/* Rotate PDF */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-green-100 rounded-lg text-green-600">
              <RotateCw className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Rotate PDF</h3>
          </div>
          <form onSubmit={handleRotate} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-green-50 file:text-green-700 hover:file:bg-green-100" />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Rotation Angle</label>
              <select name="rotation" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]">
                <option value="90">90 Degrees</option>
                <option value="180">180 Degrees</option>
                <option value="270">270 Degrees</option>
              </select>
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'rotate' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><RotateCw className="w-4 h-4" /> Rotate PDF</>}
            </button>
          </form>
        </div>

        {/* Page Numbers */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-gray-100 rounded-lg text-gray-600">
              <Hash className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Page Numbers</h3>
          </div>
          <form onSubmit={handlePageNumbers} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Upload PDF</label>
              <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-[var(--text-secondary)] file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-gray-50 file:text-gray-700 hover:file:bg-gray-100" />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Position</label>
              <select name="position" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]">
                <option value="bottom-center">Bottom Center</option>
                <option value="bottom-right">Bottom Right</option>
                <option value="bottom-left">Bottom Left</option>
                <option value="top-center">Top Center</option>
                <option value="top-right">Top Right</option>
                <option value="top-left">Top Left</option>
              </select>
            </div>
            <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-gray-600 hover:bg-gray-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
              {loading === 'page-numbers' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Hash className="w-4 h-4" /> Add Numbers</>}
            </button>
          </form>
        </div>

        {/* === ADVANCED MANIPULATION === */}

        {/* Flatten Annotations */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-red-100 rounded-lg text-red-600">
              <Layers className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Flatten Annotations</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <p className="text-xs text-[var(--text-secondary)]">
              Merge all annotations (highlights, notes, stamps) permanently into the page content.
              This makes them non-editable but ensures they appear in all PDF viewers.
            </p>
            <button disabled className="w-full py-2 px-4 bg-red-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <Layers className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>

        {/* Remove Blank Pages */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-rose-100 rounded-lg text-rose-600">
              <Trash2 className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Remove Blank Pages</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <p className="text-xs text-[var(--text-secondary)]">
              Automatically detect and remove pages with no text, images, or drawings.
              Perfect for cleaning up scanned documents.
            </p>
            <button disabled className="w-full py-2 px-4 bg-rose-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <Trash2 className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>

        {/* Custom Numbering */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-violet-100 rounded-lg text-violet-600">
              <FileType className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Custom Numbering</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Format</label>
              <select className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]">
                {NUMBERING_FORMATS.map(f => (
                  <option key={f.value} value={f.value}>{f.label}</option>
                ))}
              </select>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Prefix</label>
                <input type="text" placeholder="Page " className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Suffix</label>
                <input type="text" placeholder=" of 10" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
              </div>
            </div>
            <button disabled className="w-full py-2 px-4 bg-violet-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <FileType className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>

        {/* Header/Footer */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-sky-100 rounded-lg text-sky-600">
              <AlignCenter className="w-6 h-6" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">Header/Footer</h3>
              <span className="text-xs text-green-600 font-medium">NEW</span>
            </div>
          </div>
          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Header Text</label>
              <input type="text" placeholder="Company Name" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Footer Text</label>
              <input type="text" placeholder="Page {page} of {total}" className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm" />
            </div>
            <p className="text-[10px] text-[var(--text-secondary)]">Use {"{page}"} and {"{total}"} for page numbers</p>
            <button disabled className="w-full py-2 px-4 bg-sky-600/50 text-white rounded-lg font-medium cursor-not-allowed flex items-center justify-center gap-2">
              <AlignCenter className="w-4 h-4" /> Use in Editor
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ManipulationTools;

