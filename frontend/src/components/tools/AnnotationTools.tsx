import { useState } from 'react';
import axios from 'axios';
import {
  FileText,
  Music,
  PenTool,
  MessageSquare,
  Loader2,
  CheckCircle,
  AlertCircle,
  Settings,
} from 'lucide-react';
import { useEditor } from '../../contexts/EditorContext';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

interface Message {
  type: 'success' | 'error';
  text: string;
}

interface PolygonPoint {
  x: number;
  y: number;
}

const AnnotationTools: React.FC = () => {
  const { sessionId, currentPage, color, setColor } = useEditor();
  const [loading, setLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [activeTab, setActiveTab] = useState<'file' | 'sound' | 'polygon' | 'style'>('file');

  // File attachment state
  const [filePath, setFilePath] = useState('');
  const [fileName, setFileName] = useState('');
  const [fileX, setFileX] = useState(100);
  const [fileY, setFileY] = useState(100);
  const [fileWidth, setFileWidth] = useState(32);
  const [fileHeight, setFileHeight] = useState(32);

  // Sound annotation state
  const [audioPath, setAudioPath] = useState('');
  const [audioMimeType, setAudioMimeType] = useState('audio/mp3');
  const [soundX, setSoundX] = useState(100);
  const [soundY, setSoundY] = useState(100);

  // Polygon state
  const [polygonPoints, setPolygonPoints] = useState<PolygonPoint[]>([]);
  const [tempPoint, setTempPoint] = useState({ x: 0, y: 0 });
  const [polygonColor, setPolygonColor] = useState([1, 0, 0]);
  const [polygonFill, setPolygonFill] = useState<[number, number, number] | null>(null);
  const [polygonWidth, setPolygonWidth] = useState(1);

  // Style settings
  const [strokeColor, setStrokeColor] = useState([0, 0, 1]);
  const [fillColor, setFillColor] = useState<[number, number, number] | null>(null);
  const [borderWidth, setBorderWidth] = useState(1);
  const [opacity, setOpacity] = useState(1);

  const showMessage = (type: 'success' | 'error', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 5000);
  };

  const hexToRgb = (hex: string): [number, number, number] => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
      ? [
          parseInt(result[1], 16) / 255,
          parseInt(result[2], 16) / 255,
          parseInt(result[3], 16) / 255,
        ]
      : [0, 0, 0];
  };

  const handleAddFileAttachment = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('file');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/annotations/file`,
        {
          page_num: currentPage,
          x: fileX,
          y: fileY,
          width: fileWidth,
          height: fileHeight,
          file_path: filePath,
          filename: fileName || undefined,
          color: [0, 0, 1],
        }
      );

      if (response.data.success) {
        showMessage('success', 'File attachment added');
      }
    } catch (error) {
      showMessage('error', 'Failed to add file attachment');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleAddSoundAnnotation = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    setLoading('sound');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/annotations/sound`,
        {
          page_num: currentPage,
          x: soundX,
          y: soundY,
          width: 32,
          height: 32,
          audio_path: audioPath,
          mime_type: audioMimeType,
          color: [0, 0, 1],
        }
      );

      if (response.data.success) {
        showMessage('success', 'Sound annotation added');
      }
    } catch (error) {
      showMessage('error', 'Failed to add sound annotation');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const handleAddPolygon = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sessionId) {
      showMessage('error', 'No document loaded');
      return;
    }

    if (polygonPoints.length < 3) {
      showMessage('error', 'Polygon requires at least 3 points');
      return;
    }

    setLoading('polygon');
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/annotations/polygon`,
        {
          page_num: currentPage,
          points: polygonPoints.map((p) => [p.x, p.y]),
          color: polygonColor,
          fill_color: polygonFill,
          width: polygonWidth,
          opacity: 1,
        }
      );

      if (response.data.success) {
        showMessage('success', 'Polygon annotation added');
        setPolygonPoints([]);
      }
    } catch (error) {
      showMessage('error', 'Failed to add polygon');
      console.error(error);
    } finally {
      setLoading(null);
    }
  };

  const addPolygonPoint = () => {
    setPolygonPoints([...polygonPoints, { x: tempPoint.x, y: tempPoint.y }]);
  };

  const removePolygonPoint = (index: number) => {
    setPolygonPoints(polygonPoints.filter((_, i) => i !== index));
  };

  const colorOptions = [
    { name: 'Red', hex: '#ff0000', rgb: [1, 0, 0] },
    { name: 'Green', hex: '#00ff00', rgb: [0, 1, 0] },
    { name: 'Blue', hex: '#0000ff', rgb: [0, 0, 1] },
    { name: 'Yellow', hex: '#ffff00', rgb: [1, 1, 0] },
    { name: 'Orange', hex: '#ff8000', rgb: [1, 0.5, 0] },
    { name: 'Purple', hex: '#8000ff', rgb: [0.5, 0, 1] },
    { name: 'Black', hex: '#000000', rgb: [0, 0, 0] },
  ];

  if (!sessionId) {
    return (
      <div className="p-8 max-w-7xl mx-auto animate-fade-in">
        <div className="text-center py-12 bg-[var(--card-bg)] rounded-xl border border-[var(--border-color)]">
          <FileText className="w-12 h-12 mx-auto mb-4 text-[var(--text-secondary)]" />
          <p className="text-[var(--text-secondary)]">Upload a PDF to use annotation tools</p>
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
            Advanced Annotations
          </h2>
          <span className="tag">Page {currentPage + 1}</span>
        </div>
        <p className="text-sm text-[var(--text-secondary)] font-body">
          File attachments, sound annotations, free-form polygons, and appearance customization
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

      {/* Tabs */}
      <div className="flex gap-2 mb-6 border-b border-[var(--border-color)]">
        <button
          onClick={() => setActiveTab('file')}
          className={`px-4 py-2 font-medium text-sm border-b-2 transition-colors ${
            activeTab === 'file'
              ? 'border-blue-600 text-blue-600'
              : 'border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
          }`}
        >
          <FileText className="w-4 h-4 inline mr-2" />
          File Attachment
        </button>
        <button
          onClick={() => setActiveTab('sound')}
          className={`px-4 py-2 font-medium text-sm border-b-2 transition-colors ${
            activeTab === 'sound'
              ? 'border-green-600 text-green-600'
              : 'border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
          }`}
        >
          <Music className="w-4 h-4 inline mr-2" />
          Sound
        </button>
        <button
          onClick={() => setActiveTab('polygon')}
          className={`px-4 py-2 font-medium text-sm border-b-2 transition-colors ${
            activeTab === 'polygon'
              ? 'border-orange-600 text-orange-600'
              : 'border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
          }`}
        >
          <PenTool className="w-4 h-4 inline mr-2" />
          Polygon
        </button>
        <button
          onClick={() => setActiveTab('style')}
          className={`px-4 py-2 font-medium text-sm border-b-2 transition-colors ${
            activeTab === 'style'
              ? 'border-purple-600 text-purple-600'
              : 'border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
          }`}
        >
          <Settings className="w-4 h-4 inline mr-2" />
          Style Settings
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* File Attachment Tab */}
        {activeTab === 'file' && (
          <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)] lg:col-span-2">
            <h3 className="font-semibold text-lg text-[var(--text-primary)] mb-4">
              File Attachment Annotation
            </h3>
            <form onSubmit={handleAddFileAttachment} className="space-y-4 max-w-lg">
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                  File Path (server-side)
                </label>
                <input
                  type="text"
                  value={filePath}
                  onChange={(e) => setFilePath(e.target.value)}
                  placeholder="/path/to/file.pdf"
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                  required
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                  Display Name (optional)
                </label>
                <input
                  type="text"
                  value={fileName}
                  onChange={(e) => setFileName(e.target.value)}
                  placeholder="Attachment name"
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                />
              </div>
              <div className="grid grid-cols-4 gap-2">
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">X</label>
                  <input
                    type="number"
                    value={fileX}
                    onChange={(e) => setFileX(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Y</label>
                  <input
                    type="number"
                    value={fileY}
                    onChange={(e) => setFileY(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Width</label>
                  <input
                    type="number"
                    value={fileWidth}
                    onChange={(e) => setFileWidth(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Height</label>
                  <input
                    type="number"
                    value={fileHeight}
                    onChange={(e) => setFileHeight(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
              </div>
              <button
                type="submit"
                disabled={!!loading}
                className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {loading === 'file' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" /> Adding...
                  </>
                ) : (
                  <>
                    <FileText className="w-4 h-4" /> Add File Attachment
                  </>
                )}
              </button>
            </form>
            <p className="text-xs text-[var(--text-secondary)] mt-3">
              Note: The file must exist on the server where the PDF editor is running.
            </p>
          </div>
        )}

        {/* Sound Annotation Tab */}
        {activeTab === 'sound' && (
          <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)] lg:col-span-2">
            <h3 className="font-semibold text-lg text-[var(--text-primary)] mb-4">
              Sound / Audio Annotation
            </h3>
            <form onSubmit={handleAddSoundAnnotation} className="space-y-4 max-w-lg">
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                  Audio File Path (server-side)
                </label>
                <input
                  type="text"
                  value={audioPath}
                  onChange={(e) => setAudioPath(e.target.value)}
                  placeholder="/path/to/audio.mp3"
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                  required
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">
                  MIME Type
                </label>
                <select
                  value={audioMimeType}
                  onChange={(e) => setAudioMimeType(e.target.value)}
                  className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                >
                  <option value="audio/mp3">MP3 (audio/mp3)</option>
                  <option value="audio/wav">WAV (audio/wav)</option>
                  <option value="audio/mpeg">MPEG (audio/mpeg)</option>
                  <option value="audio/mp4">MP4 (audio/mp4)</option>
                </select>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">X Position</label>
                  <input
                    type="number"
                    value={soundX}
                    onChange={(e) => setSoundX(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">Y Position</label>
                  <input
                    type="number"
                    value={soundY}
                    onChange={(e) => setSoundY(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)] text-sm"
                  />
                </div>
              </div>
              <button
                type="submit"
                disabled={!!loading}
                className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {loading === 'sound' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" /> Adding...
                  </>
                ) : (
                  <>
                    <Music className="w-4 h-4" /> Add Sound Annotation
                  </>
                )}
              </button>
            </form>
            <p className="text-xs text-[var(--text-secondary)] mt-3">
              Audio annotations are embedded in the PDF and can be played in compatible PDF viewers.
            </p>
          </div>
        )}

        {/* Polygon Tab */}
        {activeTab === 'polygon' && (
          <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)] lg:col-span-2">
            <h3 className="font-semibold text-lg text-[var(--text-primary)] mb-4">
              Polygon / Free-form Shape
            </h3>
            <form onSubmit={handleAddPolygon} className="space-y-4">
              <div className="flex flex-wrap gap-2 mb-4">
                {colorOptions.map((c) => (
                  <button
                    key={c.hex}
                    type="button"
                    onClick={() => setPolygonColor(c.rgb as [number, number, number])}
                    className={`w-8 h-8 rounded border-2 ${
                      polygonColor[0] === c.rgb[0] &&
                      polygonColor[1] === c.rgb[1] &&
                      polygonColor[2] === c.rgb[2]
                        ? 'border-gray-900 ring-2 ring-offset-2 ring-gray-400'
                        : 'border-gray-300'
                    }`}
                    style={{ backgroundColor: c.hex }}
                    title={c.name}
                  />
                ))}
              </div>

              <div className="grid grid-cols-3 gap-4 mb-4">
                <div>
                  <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Line Width</label>
                  <input
                    type="number"
                    min="1"
                    max="10"
                    value={polygonWidth}
                    onChange={(e) => setPolygonWidth(Number(e.target.value))}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-[var(--text-secondary)] mb-1">Fill</label>
                  <select
                    value={polygonFill ? 'custom' : 'none'}
                    onChange={(e) => setPolygonFill(e.target.value === 'none' ? null : polygonColor)}
                    className="w-full p-2 border border-[var(--border-color)] rounded-lg bg-[var(--input-bg)] text-[var(--text-primary)]"
                  >
                    <option value="none">No Fill</option>
                    <option value="custom">Same as Stroke</option>
                  </select>
                </div>
              </div>

              <div className="border border-[var(--border-color)] rounded-lg p-4 bg-[var(--input-bg)]">
                <h4 className="text-sm font-medium text-[var(--text-secondary)] mb-2">Polygon Points</h4>
                <div className="grid grid-cols-2 gap-2 mb-2">
                  <div>
                    <label className="block text-xs text-[var(--text-secondary)] mb-1">X</label>
                    <input
                      type="number"
                      value={tempPoint.x}
                      onChange={(e) => setTempPoint({ ...tempPoint, x: Number(e.target.value) })}
                      className="w-full p-2 border border-[var(--border-color)] rounded bg-[var(--card-bg)] text-[var(--text-primary)] text-sm"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-[var(--text-secondary)] mb-1">Y</label>
                    <input
                      type="number"
                      value={tempPoint.y}
                      onChange={(e) => setTempPoint({ ...tempPoint, y: Number(e.target.value) })}
                      className="w-full p-2 border border-[var(--border-color)] rounded bg-[var(--card-bg)] text-[var(--text-primary)] text-sm"
                    />
                  </div>
                </div>
                <button
                  type="button"
                  onClick={addPolygonPoint}
                  className="w-full py-2 px-4 bg-gray-600 hover:bg-gray-700 text-white rounded-lg font-medium transition-colors text-sm"
                >
                  Add Point
                </button>

                {polygonPoints.length > 0 && (
                  <div className="mt-4 space-y-1">
                    <h5 className="text-xs text-[var(--text-secondary)]">
                      {polygonPoints.length} points added
                    </h5>
                    {polygonPoints.map((point, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between p-2 bg-[var(--card-bg)] rounded text-sm"
                      >
                        <span className="text-[var(--text-primary)]">
                          Point {index + 1}: ({point.x}, {point.y})
                        </span>
                        <button
                          type="button"
                          onClick={() => removePolygonPoint(index)}
                          className="p-1 hover:bg-red-100 rounded text-red-500"
                        >
                          Remove
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <button
                type="submit"
                disabled={!!loading || polygonPoints.length < 3}
                className="w-full py-2 px-4 bg-orange-600 hover:bg-orange-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {loading === 'polygon' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" /> Creating...
                  </>
                ) : (
                  <>
                    <PenTool className="w-4 h-4" /> Create Polygon ({polygonPoints.length} points)
                  </>
                )}
              </button>
            </form>
          </div>
        )}

        {/* Style Settings Tab */}
        {activeTab === 'style' && (
          <div className="bg-[var(--card-bg)] p-6 rounded-xl shadow-sm border border-[var(--border-color)] lg:col-span-2">
            <h3 className="font-semibold text-lg text-[var(--text-primary)] mb-4">
              Annotation Style Settings
            </h3>
            <p className="text-[var(--text-secondary)] mb-4">
              Configure default appearance for new annotations. These settings will be applied to
              newly created annotations.
            </p>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <h4 className="text-sm font-medium text-[var(--text-primary)] mb-3">Stroke Color</h4>
                <div className="flex flex-wrap gap-2">
                  {colorOptions.map((c) => (
                    <button
                      key={c.hex}
                      type="button"
                      onClick={() => setStrokeColor(c.rgb as [number, number, number])}
                      className={`w-10 h-10 rounded-lg border-2 transition-all ${
                        strokeColor[0] === c.rgb[0] &&
                        strokeColor[1] === c.rgb[1] &&
                        strokeColor[2] === c.rgb[2]
                          ? 'border-gray-900 ring-2 ring-offset-2 ring-blue-400 scale-110'
                          : 'border-gray-300 hover:scale-105'
                      }`}
                      style={{ backgroundColor: c.hex }}
                      title={c.name}
                    />
                  ))}
                </div>
              </div>

              <div>
                <h4 className="text-sm font-medium text-[var(--text-primary)] mb-3">Border Width</h4>
                <div className="flex items-center gap-4">
                  <input
                    type="range"
                    min="1"
                    max="10"
                    value={borderWidth}
                    onChange={(e) => setBorderWidth(Number(e.target.value))}
                    className="flex-1"
                  />
                  <span className="text-lg font-medium text-[var(--text-primary)] w-8">
                    {borderWidth}
                  </span>
                </div>
              </div>

              <div>
                <h4 className="text-sm font-medium text-[var(--text-primary)] mb-3">Opacity</h4>
                <div className="flex items-center gap-4">
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.1"
                    value={opacity}
                    onChange={(e) => setOpacity(Number(e.target.value))}
                    className="flex-1"
                  />
                  <span className="text-lg font-medium text-[var(--text-primary)] w-12">
                    {Math.round(opacity * 100)}%
                  </span>
                </div>
              </div>

              <div>
                <h4 className="text-sm font-medium text-[var(--text-primary)] mb-3">Preview</h4>
                <div
                  className="h-24 rounded-lg border-2 flex items-center justify-center"
                  style={{
                    backgroundColor: `rgba(${fillColor?.[0] || 1} * 255, ${fillColor?.[1] || 1} * 255, ${fillColor?.[2] || 1} * 255, ${opacity})`,
                    borderColor: `rgb(${strokeColor[0] * 255}, ${strokeColor[1] * 255}, ${strokeColor[2] * 255})`,
                    borderWidth: `${borderWidth}px`,
                  }}
                >
                  <span className="text-sm font-medium" style={{ opacity }}>
                    Annotation Preview
                  </span>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default AnnotationTools;
