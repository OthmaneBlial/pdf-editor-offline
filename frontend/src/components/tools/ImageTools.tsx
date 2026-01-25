import { useState, useEffect } from 'react';
import axios from 'axios';
import {
  Image as ImageIcon,
  Download,
  Replace,
  FileUp,
  Zap,
  Loader2,
  CheckCircle,
  AlertCircle,
  ExternalLink,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

interface ImageMetadata {
  index: number;
  xref: number;
  width: number;
  height: number;
  bits_per_component: number;
  color_space: string;
  compression: string;
  format?: string;
  size_bytes?: number;
  aspect_ratio?: number;
  bbox?: number[];
  has_mask?: boolean;
  name?: string;
}

interface Message {
  type: 'success' | 'error';
  text: string;
}

const ImageTools: React.FC = () => {
  const { sessionId, currentPage } = useEditor();
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<Message | null>(null);

  // Image list state
  const [images, setImages] = useState<ImageMetadata[]>([]);
  const [allImages, setAllImages] = useState<Record<number, ImageMetadata[]>>({});

  // Image replace state
  const [replaceImagePath, setReplaceImagePath] = useState('');
  const [selectedImageIndex, setSelectedImageIndex] = useState<number | null>(null);

  // Image insert state
  const [insertImagePath, setInsertImagePath] = useState('');
  const [insertX, setInsertX] = useState(100);
  const [insertY, setInsertY] = useState(100);
  const [insertWidth, setInsertWidth] = useState(200);
  const [insertHeight, setInsertHeight] = useState(200);

  // Optimization state
  const [garbageLevel, setGarbageLevel] = useState(4);
  const [deflate, setDeflate] = useState(true);
  const [clean, setClean] = useState(true);

  const showMessage = (type: 'success' | 'error', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 5000);
  };

  const loadPageImages = async () => {
    if (!sessionId) return;

    setLoading('images');
    try {
      const response = await axios.get(
        `${API_BASE_URL}/api/documents/${sessionId}/images/${currentPage}`
      );
      if (response.data.success) {
        setImages(response.data.data.images || []);
      }
    } catch (error) {
      showMessage('error', 'Failed to load images');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const loadAllImages = async () => {
    if (!sessionId) return;

    setLoading('all-images');
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/images`);
      if (response.data.success) {
        setAllImages(response.data.data.images || {});
      }
    } catch (error) {
      showMessage('error', 'Failed to load all images');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleReplaceImage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId || selectedImageIndex === null) {
      showMessage('error', 'Select an image to replace');
      return;
    }

    setLoading('replace');
    try {
      const img = images[selectedImageIndex];
      const rect = img.bbox || [100, 100, 200, 200];

      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/images/replace`,
        {
          page_num: currentPage,
          old_rect: rect,
          new_image_path: replaceImagePath,
          maintain_aspect: true,
        }
      );

      if (response.data.success) {
        showMessage('success', 'Image replaced successfully');
        loadPageImages();
      }
    } catch (error) {
      showMessage('error', 'Failed to replace image');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleInsertImage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) return;

    setLoading('insert');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/images/insert`,
        {
          page_num: currentPage,
          x: insertX,
          y: insertY,
          width: insertWidth,
          height: insertHeight,
          image_path: insertImagePath,
          maintain_aspect: true,
        }
      );

      if (response.data.success) {
        showMessage('success', 'Image inserted successfully');
        loadPageImages();
      }
    } catch (error) {
      showMessage('error', 'Failed to insert image');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleOptimizeDocument = async () => {
    if (!sessionId) return;

    setLoading('optimize');
    try {
      const params = new URLSearchParams({
        garbage: garbageLevel.toString(),
        deflate: deflate.toString(),
        clean: clean.toString(),
      });

      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/optimize?${params.toString()}`,
        {},
        {
          responseType: 'blob',
        }
      );

      // Download the optimized file
      const url = window.URL.createObjectURL(response.data);
      const link = document.createElement('a');
      link.href = url;
      link.setAttribute('download', `optimized_document.pdf`);
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.URL.revokeObjectURL(url);

      showMessage('success', 'Document optimized and downloaded');
    } catch (error) {
      showMessage('error', 'Failed to optimize document');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleOptimizePage = async () => {
    if (!sessionId) return;

    setLoading('optimize-page');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/optimize`
      );

      if (response.data.success) {
        showMessage('success', 'Page optimized successfully');
      }
    } catch (error) {
      showMessage('error', 'Failed to optimize page');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const formatFileSize = (bytes: number | undefined): string => {
    if (!bytes) return 'N/A';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  useEffect(() => {
    if (sessionId) {
      loadPageImages();
    }
  }, [sessionId, currentPage]);

  if (!sessionId) {
    return (
      <div className="p-8 max-w-7xl mx-auto animate-fade-in">
        <div className="text-center py-12 bg-[var(--card-bg)] rounded-xl border border-[var(--border-color)]">
          <ImageIcon className="w-12 h-12 mx-auto mb-4 text-[var(--text-secondary)]" />
          <p className="text-[var(--text-secondary)]">Upload a PDF to use image tools</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 max-w-7xl mx-auto animate-fade-in">
      {/* Page Header */}
      <div className="mb-8">
        <div className="flex items-center gap-3 mb-2">
          <h2 className="font-display text-3xl font-bold text-[var(--text-primary)]">
            Image Tools
          </h2>
          <span className="tag">Page {currentPage + 1}</span>
        </div>
        <p className="text-sm text-[var(--text-secondary)] font-body">
          Extract, replace, and insert images; optimize document size
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
        {/* Page Images */}
        <div className="lg:col-span-2 bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
                <ImageIcon className="w-6 h-6" />
              </div>
              <h3 className="font-semibold text-lg text-[var(--text-primary)]">
                Images on Page {currentPage + 1}
              </h3>
            </div>
            <button
              onClick={loadPageImages}
              disabled={loading === 'images'}
              className="p-2 hover:bg-[var(--input-bg)] rounded"
            >
              {loading === 'images' ? (
                <Loader2 className="w-4 h-4 animate-spin text-[var(--text-secondary)]" />
              ) : (
                <Loader2 className="w-4 h-4 text-[var(--text-secondary)]" />
              )}
            </button>
          </div>

          {images.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {images.map((img, index) => (
                <div
                  key={index}
                  className={`p-4 border rounded-lg cursor-pointer transition-all ${
                    selectedImageIndex === index
                      ? 'border-[var(--accent-color)] bg-[var(--accent-color)]/5'
                      : 'border-[var(--border-color)] hover:border-[var(--accent-color)]/50'
                  }`}
                  onClick={() => setSelectedImageIndex(index)}
                >
                  <div className="flex items-start justify-between mb-2">
                    <span className="text-xs font-medium px-2 py-1 bg-purple-100 text-purple-700 rounded">
                      Image #{index + 1}
                    </span>
                    {img.format && (
                      <span className="text-xs text-[var(--text-secondary)] uppercase">
                        {img.format}
                      </span>
                    )}
                  </div>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span className="text-[var(--text-secondary)]">Size:</span>{' '}
                      <span className="text-[var(--text-primary)]">{img.width}×{img.height}</span>
                    </div>
                    <div>
                      <span className="text-[var(--text-secondary)]">Space:</span>{' '}
                      <span className="text-[var(--text-primary)]">{img.color_space}</span>
                    </div>
                    <div>
                      <span className="text-[var(--text-secondary)]">BPC:</span>{' '}
                      <span className="text-[var(--text-primary)]">{img.bits_per_component}</span>
                    </div>
                    <div>
                      <span className="text-[var(--text-secondary)]">Size:</span>{' '}
                      <span className="text-[var(--text-primary)]">{formatFileSize(img.size_bytes)}</span>
                    </div>
                  </div>
                  {img.compression !== 'None' && (
                    <div className="mt-2 text-xs">
                      <span className="text-[var(--text-secondary)]">Compression:</span>{' '}
                      <span className="text-[var(--text-primary)]">{img.compression}</span>
                    </div>
                  )}
                  {img.aspect_ratio && (
                    <div className="mt-1 text-xs">
                      <span className="text-[var(--text-secondary)]">Aspect Ratio:</span>{' '}
                      <span className="text-[var(--text-primary)]">{img.aspect_ratio.toFixed(2)}</span>
                    </div>
                  )}
                  {img.has_mask && (
                    <div className="mt-1 text-xs px-2 py-0.5 bg-yellow-100 text-yellow-700 rounded inline-block">
                      Has transparency mask
                    </div>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center py-8 text-[var(--text-secondary)]">
              <ImageIcon className="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>No images found on this page</p>
            </div>
          )}
        </div>

        {/* Replace Image */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-orange-100 rounded-lg text-orange-600">
              <Replace className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Replace Image</h3>
          </div>

          <form onSubmit={handleReplaceImage} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Selected Image
              </label>
              <select
                value={selectedImageIndex ?? ''}
                onChange={(e) => setSelectedImageIndex(e.target.value ? Number(e.target.value) : null)}
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              >
                <option value="">Select an image...</option>
                {images.map((img, index) => (
                  <option key={index} value={index}>
                    Image #{index + 1} ({img.width}×{img.height})
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                New Image Path (server-side)
              </label>
              <input
                type="text"
                value={replaceImagePath}
                onChange={(e) => setReplaceImagePath(e.target.value)}
                placeholder="/path/to/new-image.png"
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              />
            </div>
            <button
              type="submit"
              disabled={!!loading || selectedImageIndex === null}
              className="w-full py-2 px-4 bg-orange-600 hover:bg-orange-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'replace' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Replacing...
                </>
              ) : (
                <>
                  <Replace className="w-4 h-4" /> Replace Image
                </>
              )}
            </button>
          </form>
        </div>

        {/* Insert New Image */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-green-100 rounded-lg text-green-600">
              <FileUp className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Insert Image</h3>
          </div>

          <form onSubmit={handleInsertImage} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                Image Path (server-side)
              </label>
              <input
                type="text"
                value={insertImagePath}
                onChange={(e) => setInsertImagePath(e.target.value)}
                placeholder="/path/to/image.png"
                className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                required
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">X</label>
                <input
                  type="number"
                  value={insertX}
                  onChange={(e) => setInsertX(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Y</label>
                <input
                  type="number"
                  value={insertY}
                  onChange={(e) => setInsertY(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Width</label>
                <input
                  type="number"
                  value={insertWidth}
                  onChange={(e) => setInsertWidth(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Height</label>
                <input
                  type="number"
                  value={insertHeight}
                  onChange={(e) => setInsertHeight(Number(e.target.value))}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                />
              </div>
            </div>
            <button
              type="submit"
              disabled={!!loading}
              className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {loading === 'insert' ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" /> Inserting...
                </>
              ) : (
                <>
                  <FileUp className="w-4 h-4" /> Insert Image
                </>
              )}
            </button>
          </form>
        </div>

        {/* Document Optimization */}
        <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-yellow-100 rounded-lg text-yellow-600">
              <Zap className="w-6 h-6" />
            </div>
            <h3 className="font-semibold text-lg text-[var(--text-primary)]">Optimize</h3>
          </div>

          <div className="space-y-4">
            <div className="p-3 bg-[var(--input-bg)] rounded-lg">
              <h4 className="text-sm font-medium text-[var(--text-primary)] mb-2">Optimize Current Page</h4>
              <p className="text-xs text-[var(--text-secondary)] mb-3">
                Remove redundant content from this page
              </p>
              <button
                onClick={handleOptimizePage}
                disabled={loading === 'optimize-page'}
                className="w-full py-2 px-4 bg-yellow-600 hover:bg-yellow-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2 text-sm"
              >
                {loading === 'optimize-page' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" /> Optimizing...
                  </>
                ) : (
                  <>
                    <Zap className="w-4 h-4" /> Optimize Page
                  </>
                )}
              </button>
            </div>

            <div className="p-3 bg-[var(--input-bg)] rounded-lg">
              <h4 className="text-sm font-medium text-[var(--text-primary)] mb-2">Optimize Document</h4>
              <p className="text-xs text-[var(--text-secondary)] mb-3">
                Download optimized version with reduced file size
              </p>

              <div className="space-y-2 mb-3">
                <div className="flex items-center justify-between">
                  <label className="text-xs text-[var(--text-secondary)]">Garbage Collection</label>
                  <select
                    value={garbageLevel}
                    onChange={(e) => setGarbageLevel(Number(e.target.value))}
                    className="text-xs p-1 border border-[var(--border-color)] rounded bg-[var(--card-bg)] text-[var(--text-primary)]"
                  >
                    <option value={0}>None</option>
                    <option value={1}>Basic</option>
                    <option value={2}>Standard</option>
                    <option value={3}>Advanced</option>
                    <option value={4}>Maximum</option>
                  </select>
                </div>
                <div className="flex items-center justify-between">
                  <label className="text-xs text-[var(--text-secondary)]">Compress Streams</label>
                  <input
                    type="checkbox"
                    checked={deflate}
                    onChange={(e) => setDeflate(e.target.checked)}
                    className="rounded"
                  />
                </div>
                <div className="flex items-center justify-between">
                  <label className="text-xs text-[var(--text-secondary)]">Clean Content</label>
                  <input
                    type="checkbox"
                    checked={clean}
                    onChange={(e) => setClean(e.target.checked)}
                    className="rounded"
                  />
                </div>
              </div>

              <button
                onClick={handleOptimizeDocument}
                disabled={loading === 'optimize'}
                className="w-full py-2 px-4 bg-yellow-600 hover:bg-yellow-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2 text-sm"
              >
                {loading === 'optimize' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" /> Optimizing...
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4" /> Optimize & Download
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ImageTools;
