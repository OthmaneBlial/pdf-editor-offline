import React, { useState } from 'react';
import axios from 'axios';
import { FileText, RefreshCw, Layers, Settings, Download, Upload } from 'lucide-react';
import { API_BASE_URL } from '../../lib/apiClient';

const BatchProcessingTools: React.FC = () => {
    const [loading, setLoading] = useState<string | null>(null);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [activeTab, setActiveTab] = useState<'batch' | 'merge' | 'template'>('batch');
    const [selectedFiles, setSelectedFiles] = useState<FileList | null>(null);

    // Template form state
    const [watermarkText, setWatermarkText] = useState('');
    const [watermarkOpacity, setWatermarkOpacity] = useState(0.3);
    const [watermarkRotation, setWatermarkRotation] = useState(45);
    const [watermarkFontSize, setWatermarkFontSize] = useState(50);
    const [watermarkColor, setWatermarkColor] = useState('#000000');
    const [rotate, setRotate] = useState<number | null>(null);
    const [compress, setCompress] = useState<number | null>(null);
    const [protectPassword, setProtectPassword] = useState('');

    const conversionTypes = [
        { value: 'pdf-to-word', label: 'PDF to Word' },
        { value: 'pdf-to-ppt', label: 'PDF to PowerPoint' },
        { value: 'pdf-to-excel', label: 'PDF to Excel' },
        { value: 'pdf-to-markdown', label: 'PDF to Markdown' },
        { value: 'pdf-to-txt', label: 'PDF to TXT' },
        { value: 'pdf-to-jpg', label: 'PDF to JPG' },
        { value: 'pdf-to-svg', label: 'PDF to SVG' },
        { value: 'pdf-to-epub', label: 'PDF to EPUB' },
        { value: 'word-to-pdf', label: 'Word to PDF' },
        { value: 'ppt-to-pdf', label: 'PowerPoint to PDF' },
        { value: 'excel-to-pdf', label: 'Excel to PDF' },
        { value: 'markdown-to-pdf', label: 'Markdown to PDF' },
        { value: 'txt-to-pdf', label: 'TXT to PDF' },
        { value: 'csv-to-pdf', label: 'CSV to PDF' },
        { value: 'json-to-pdf', label: 'JSON to PDF' },
    ];

    const downloadFile = (data: Blob, filename: string) => {
        const url = window.URL.createObjectURL(data);
        const link = document.createElement('a');
        link.href = url;
        link.setAttribute('download', filename);
        document.body.appendChild(link);
        link.click();
        link.remove();
    };

    const handleBatchConvert = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('batch-convert');
        setMessage(null);

        const formData = new FormData(e.currentTarget);
        if (selectedFiles) {
            for (let i = 0; i < selectedFiles.length; i++) {
                formData.append('files', selectedFiles[i]);
            }
        }

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/batch-convert`, formData, {
                responseType: 'blob',
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'batch_converted.zip';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: `Batch conversion complete! Processed ${selectedFiles?.length || 0} files.` });
        } catch (error) {
            setMessage({ type: 'error', text: 'Batch conversion failed.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleAutoMerge = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('auto-merge');
        setMessage(null);

        const formData = new FormData();
        if (selectedFiles) {
            for (let i = 0; i < selectedFiles.length; i++) {
                formData.append('files', selectedFiles[i]);
            }
        }

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/auto-merge-folder`, formData, {
                responseType: 'blob',
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'merged.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: `Merged ${selectedFiles?.length || 0} PDFs successfully!` });
        } catch (error) {
            setMessage({ type: 'error', text: 'Merge failed.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleTemplateProcess = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('template-process');
        setMessage(null);

        const formData = new FormData();
        if (selectedFiles) {
            for (let i = 0; i < selectedFiles.length; i++) {
                formData.append('files', selectedFiles[i]);
            }
        }

        // Add template options
        if (watermarkText) {
            formData.append('watermark_text', watermarkText);
            formData.append('watermark_opacity', watermarkOpacity.toString());
            formData.append('watermark_rotation', watermarkRotation.toString());
            formData.append('watermark_font_size', watermarkFontSize.toString());
            formData.append('watermark_color', watermarkColor);
        }
        if (rotate !== null) {
            formData.append('rotate', rotate.toString());
        }
        if (compress !== null) {
            formData.append('compress', compress.toString());
        }
        if (protectPassword) {
            formData.append('protect_password', protectPassword);
        }

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/template-process`, formData, {
                responseType: 'blob',
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'template_processed.zip';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: `Template applied to ${selectedFiles?.length || 0} files!` });
        } catch (error) {
            setMessage({ type: 'error', text: 'Template processing failed.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        setSelectedFiles(e.target.files);
    };

    return (
        <div className="p-3 sm:p-6 lg:p-8 max-w-7xl mx-auto animate-fade-in">
            <div className="mb-8">
                <h2 className="text-3xl font-bold bg-gradient-to-r from-sky-700 to-cyan-700 bg-clip-text text-transparent mb-2">Batch Processing</h2>
                <p className="text-gray-500">Process multiple files at once</p>
            </div>

            {message && (
                <div className={`p-4 mb-6 rounded-xl shadow-lg ${message.type === 'success' ? 'bg-gradient-to-r from-green-50 to-emerald-50 text-green-700 border border-green-200' : 'bg-gradient-to-r from-red-50 to-pink-50 text-red-700 border border-red-200'} animate-scale-in`}>
                    {message.text}
                </div>
            )}

            {/* Tabs */}
            <div className="flex gap-3 sm:gap-6 mb-6 sm:mb-8 border-b border-sky-100 overflow-x-auto whitespace-nowrap pb-1">
                <button
                    onClick={() => setActiveTab('batch')}
                    className={`pb-3 sm:pb-4 px-3 sm:px-6 text-sm sm:text-base font-semibold transition-all duration-300 relative ${activeTab === 'batch' ? 'text-sky-700' : 'text-gray-500 hover:text-gray-700'}`}
                >
                    <div className="flex items-center gap-2">
                        <Layers className="w-4 h-4" />
                        Batch Convert
                    </div>
                    {activeTab === 'batch' && <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-sky-600 to-cyan-600 rounded-full" />}
                </button>
                <button
                    onClick={() => setActiveTab('merge')}
                    className={`pb-3 sm:pb-4 px-3 sm:px-6 text-sm sm:text-base font-semibold transition-all duration-300 relative ${activeTab === 'merge' ? 'text-sky-700' : 'text-gray-500 hover:text-gray-700'}`}
                >
                    <div className="flex items-center gap-2">
                        <FileText className="w-4 h-4" />
                        Auto-Merge
                    </div>
                    {activeTab === 'merge' && <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-sky-600 to-cyan-600 rounded-full" />}
                </button>
                <button
                    onClick={() => setActiveTab('template')}
                    className={`pb-3 sm:pb-4 px-3 sm:px-6 text-sm sm:text-base font-semibold transition-all duration-300 relative ${activeTab === 'template' ? 'text-sky-700' : 'text-gray-500 hover:text-gray-700'}`}
                >
                    <div className="flex items-center gap-2">
                        <Settings className="w-4 h-4" />
                        Template Process
                    </div>
                    {activeTab === 'template' && <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-sky-600 to-cyan-600 rounded-full" />}
                </button>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* File Upload Section */}
                <div className="lg:col-span-1">
                    <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-sky-100/50">
                        <h3 className="font-bold text-lg text-gray-800 mb-4 flex items-center gap-2">
                            <Upload className="w-5 h-5 text-sky-600" />
                            Select Files
                        </h3>
                        <div className="space-y-4">
                            <input
                                type="file"
                                multiple
                                accept={
                                    activeTab === 'batch'
                                        ? '.pdf,.docx,.doc,.pptx,.ppt,.xlsx,.xls,.md,.txt,.csv,.json,.html,.htm,.png,.jpg,.jpeg'
                                        : '.pdf'
                                }
                                onChange={handleFileSelect}
                                className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-sky-50 file:to-cyan-50 file:text-sky-700 hover:file:from-sky-100 hover:file:to-cyan-100 file:transition-all file:cursor-pointer"
                            />
                            {selectedFiles && selectedFiles.length > 0 && (
                                <div className="p-3 bg-sky-50 rounded-xl">
                                    <p className="text-sm text-sky-700 font-semibold">{selectedFiles.length} files selected</p>
                                    <p className="text-xs text-sky-600 mt-1">
                                        {Array.from(selectedFiles)
                                            .slice(0, 3)
                                            .map((f) => f.name)
                                            .join(', ')}
                                        {selectedFiles.length > 3 && '...'}
                                    </p>
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {/* Processing Options */}
                <div className="lg:col-span-2">
                    {activeTab === 'batch' && (
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-sky-100/50">
                            <h3 className="font-bold text-lg text-gray-800 mb-4">Batch Conversion</h3>
                            <form onSubmit={handleBatchConvert} className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-gray-700 mb-2">Conversion Type</label>
                                    <select
                                        name="conversion_type"
                                        required
                                        className="w-full px-4 py-2.5 rounded-xl border border-gray-300 focus:border-sky-500 focus:ring-2 focus:ring-sky-200 outline-none transition-all"
                                    >
                                        <option value="">Select conversion type...</option>
                                        {conversionTypes.map((type) => (
                                            <option key={type.value} value={type.value}>
                                                {type.label}
                                            </option>
                                        ))}
                                    </select>
                                </div>
                                <button
                                    type="submit"
                                    disabled={!!loading || !selectedFiles || selectedFiles.length === 0}
                                    className="w-full py-3 px-4 bg-gradient-to-r from-sky-600 to-cyan-600 hover:from-sky-700 hover:to-cyan-700 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-sky-500/30 hover:shadow-sky-500/50 hover:-translate-y-0.5"
                                >
                                    {loading === 'batch-convert' ? (
                                        <>
                                            <RefreshCw className="w-4 h-4 animate-spin" /> Processing...
                                        </>
                                    ) : (
                                        <>
                                            <Download className="w-4 h-4" /> Convert All Files
                                        </>
                                    )}
                                </button>
                            </form>
                        </div>
                    )}

                    {activeTab === 'merge' && (
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-sky-100/50">
                            <h3 className="font-bold text-lg text-gray-800 mb-4">Auto-Merge PDFs</h3>
                            <p className="text-sm text-gray-600 mb-4">
                                Merge all selected PDF files into a single PDF. Files will be merged in alphabetical order.
                            </p>
                            <form onSubmit={handleAutoMerge} className="space-y-4">
                                <button
                                    type="submit"
                                    disabled={!!loading || !selectedFiles || selectedFiles.length < 2}
                                    className="w-full py-3 px-4 bg-gradient-to-r from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-purple-500/30 hover:shadow-purple-500/50 hover:-translate-y-0.5"
                                >
                                    {loading === 'auto-merge' ? (
                                        <>
                                            <RefreshCw className="w-4 h-4 animate-spin" /> Merging...
                                        </>
                                    ) : (
                                        <>
                                            <FileText className="w-4 h-4" /> Merge PDFs
                                        </>
                                    )}
                                </button>
                                {selectedFiles && selectedFiles.length < 2 && (
                                    <p className="text-sm text-amber-600">At least 2 PDF files are required</p>
                                )}
                            </form>
                        </div>
                    )}

                    {activeTab === 'template' && (
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-sky-100/50">
                            <h3 className="font-bold text-lg text-gray-800 mb-4">Template Processing</h3>
                            <p className="text-sm text-gray-600 mb-4">
                                Apply the same settings to multiple PDFs.
                            </p>
                            <form onSubmit={handleTemplateProcess} className="space-y-4">
                                {/* Watermark Options */}
                                <div className="p-4 bg-gray-50 rounded-xl">
                                    <h4 className="font-semibold text-gray-700 mb-3">Watermark</h4>
                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                        <div className="col-span-2">
                                            <input
                                                type="text"
                                                placeholder="Watermark text"
                                                value={watermarkText}
                                                onChange={(e) => setWatermarkText(e.target.value)}
                                                className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                            />
                                        </div>
                                        <div>
                                            <label className="text-xs text-gray-600">Opacity</label>
                                            <input
                                                type="number"
                                                step="0.1"
                                                min="0"
                                                max="1"
                                                value={watermarkOpacity}
                                                onChange={(e) => setWatermarkOpacity(parseFloat(e.target.value))}
                                                className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                            />
                                        </div>
                                        <div>
                                            <label className="text-xs text-gray-600">Rotation</label>
                                            <input
                                                type="number"
                                                value={watermarkRotation}
                                                onChange={(e) => setWatermarkRotation(parseInt(e.target.value))}
                                                className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                            />
                                        </div>
                                        <div>
                                            <label className="text-xs text-gray-600">Font Size</label>
                                            <input
                                                type="number"
                                                value={watermarkFontSize}
                                                onChange={(e) => setWatermarkFontSize(parseInt(e.target.value))}
                                                className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                            />
                                        </div>
                                        <div>
                                            <label className="text-xs text-gray-600">Color</label>
                                            <input
                                                type="color"
                                                value={watermarkColor}
                                                onChange={(e) => setWatermarkColor(e.target.value)}
                                                className="w-full h-9 rounded-lg border border-gray-300 cursor-pointer"
                                            />
                                        </div>
                                    </div>
                                </div>

                                {/* Other Options */}
                                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                                    <div>
                                        <label className="block text-xs text-gray-600 mb-1">Rotate (°)</label>
                                        <input
                                            type="number"
                                            placeholder="90"
                                            value={rotate ?? ''}
                                            onChange={(e) => setRotate(e.target.value ? parseInt(e.target.value) : null)}
                                            className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-xs text-gray-600 mb-1">Compress (0-9)</label>
                                        <input
                                            type="number"
                                            min="0"
                                            max="9"
                                            placeholder="4"
                                            value={compress ?? ''}
                                            onChange={(e) => setCompress(e.target.value ? parseInt(e.target.value) : null)}
                                            className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-xs text-gray-600 mb-1">Password</label>
                                        <input
                                            type="password"
                                            placeholder="Protect PDF"
                                            value={protectPassword}
                                            onChange={(e) => setProtectPassword(e.target.value)}
                                            className="w-full px-3 py-2 rounded-lg border border-gray-300 focus:border-sky-500 outline-none text-sm"
                                        />
                                    </div>
                                </div>

                                <button
                                    type="submit"
                                    disabled={!!loading || !selectedFiles || selectedFiles.length === 0}
                                    className="w-full py-3 px-4 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-700 hover:to-purple-700 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-indigo-500/30 hover:shadow-indigo-500/50 hover:-translate-y-0.5"
                                >
                                    {loading === 'template-process' ? (
                                        <>
                                            <RefreshCw className="w-4 h-4 animate-spin" /> Processing...
                                        </>
                                    ) : (
                                        <>
                                            <Settings className="w-4 h-4" /> Apply Template
                                        </>
                                    )}
                                </button>
                            </form>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};

export default BatchProcessingTools;
