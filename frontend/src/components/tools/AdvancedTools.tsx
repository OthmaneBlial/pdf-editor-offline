import React, { useState } from 'react';
import axios from 'axios';
import { Minimize2, Wrench, Scan, Text, GitCompare, RefreshCw } from 'lucide-react';
import { API_BASE_URL } from '../../lib/apiClient';

const AdvancedTools: React.FC = () => {
    const [loading, setLoading] = useState<string | null>(null);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

    const handleCompress = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('compress');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/compress`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'compressed.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'PDF compressed successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to compress PDF.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleRepair = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('repair');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/repair`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'repaired.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'PDF repaired successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to repair PDF.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleScan = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('scan');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/scan-to-pdf`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'scanned.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'Scanned images converted to PDF!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to convert scanned images.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleOCR = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('ocr');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/ocr`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'ocr_result.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'OCR completed successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to perform OCR.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleCompare = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('compare');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/compare`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || 'comparison_diff.pdf';
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'Comparison completed successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to compare PDFs.' });
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
        <div className="p-3 sm:p-6 max-w-6xl mx-auto">
            <h2 className="text-2xl font-bold mb-6 text-gray-800">Advanced Tools</h2>

            {message && (
                <div className={`p-4 mb-6 rounded-lg ${message.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
                    {message.text}
                </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {/* Compress PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
                            <Minimize2 className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Compress PDF</h3>
                    </div>
                    <form onSubmit={handleCompress} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Compression Level</label>
                            <select name="level" className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500">
                                <option value="0">Default</option>
                                <option value="1">Prepress</option>
                                <option value="2">Printer</option>
                                <option value="3">Ebook</option>
                                <option value="4">Screen</option>
                            </select>
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'compress' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Minimize2 className="w-4 h-4" /> Compress PDF</>}
                        </button>
                    </form>
                </div>

                {/* Repair PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-green-100 rounded-lg text-green-600">
                            <Wrench className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Repair PDF</h3>
                    </div>
                    <form onSubmit={handleRepair} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-green-50 file:text-green-700 hover:file:bg-green-100" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'repair' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Wrench className="w-4 h-4" /> Repair PDF</>}
                        </button>
                    </form>
                </div>

                {/* Scan to PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
                            <Scan className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Scan to PDF</h3>
                    </div>
                    <form onSubmit={handleScan} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload Images</label>
                            <input type="file" name="files" multiple accept="image/*" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-purple-50 file:text-purple-700 hover:file:bg-purple-100" />
                        </div>
                        <div className="flex items-center gap-2">
                            <input type="checkbox" name="enhance" value="true" defaultChecked id="enhance" className="rounded text-purple-600 focus:ring-purple-500" />
                            <label htmlFor="enhance" className="text-sm text-gray-700">Enhance Images</label>
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'scan' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Scan className="w-4 h-4" /> Convert to PDF</>}
                        </button>
                    </form>
                </div>

                {/* PDF OCR */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-orange-100 rounded-lg text-orange-600">
                            <Text className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">PDF OCR</h3>
                    </div>
                    <form onSubmit={handleOCR} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-orange-50 file:text-orange-700 hover:file:bg-orange-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Language Code</label>
                            <input type="text" name="lang" defaultValue="eng" placeholder="e.g., eng, fra" className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-orange-500 focus:border-orange-500" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-orange-600 hover:bg-orange-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'ocr' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Text className="w-4 h-4" /> Perform OCR</>}
                        </button>
                    </form>
                </div>

                {/* Compare PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-gray-100 rounded-lg text-gray-600">
                            <GitCompare className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Compare PDF</h3>
                    </div>
                    <form onSubmit={handleCompare} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">First PDF</label>
                            <input type="file" name="file1" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-gray-50 file:text-gray-700 hover:file:bg-gray-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Second PDF</label>
                            <input type="file" name="file2" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-gray-50 file:text-gray-700 hover:file:bg-gray-100" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-gray-600 hover:bg-gray-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'compare' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><GitCompare className="w-4 h-4" /> Compare PDFs</>}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    );
};

export default AdvancedTools;
