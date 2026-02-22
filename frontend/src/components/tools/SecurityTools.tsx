import React, { useState } from 'react';
import axios from 'axios';
import { Lock, Unlock, PenTool, Stamp, RefreshCw } from 'lucide-react';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

const SecurityTools: React.FC = () => {
    const [loading, setLoading] = useState<string | null>(null);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

    const handleProtect = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('protect');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/protect`, formData, {
                responseType: 'blob'
            });
            downloadFile(response.data, 'protected.pdf');
            setMessage({ type: 'success', text: 'PDF protected successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to protect PDF.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleUnlock = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('unlock');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/unlock`, formData, {
                responseType: 'blob'
            });
            downloadFile(response.data, 'unlocked.pdf');
            setMessage({ type: 'success', text: 'PDF unlocked successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to unlock PDF.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleSign = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('sign');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/sign`, formData, {
                responseType: 'blob'
            });
            downloadFile(response.data, 'signed.pdf');
            setMessage({ type: 'success', text: 'PDF signed successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to sign PDF.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleWatermark = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('watermark');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/watermark`, formData, {
                responseType: 'blob'
            });
            downloadFile(response.data, 'watermarked.pdf');
            setMessage({ type: 'success', text: 'Watermark added successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to add watermark.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const downloadFile = (data: Blob, filename: string) => {
        const url = window.URL.createObjectURL(new Blob([data]));
        const link = document.createElement('a');
        link.href = url;
        link.setAttribute('download', filename);
        document.body.appendChild(link);
        link.click();
        link.remove();
    };

    return (
        <div className="p-3 sm:p-6 max-w-6xl mx-auto">
            <h2 className="text-2xl font-bold mb-6 text-gray-800">Security Tools</h2>

            {message && (
                <div className={`p-4 mb-6 rounded-lg ${message.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
                    {message.text}
                </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {/* Protect PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-red-100 rounded-lg text-red-600">
                            <Lock className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Protect PDF</h3>
                    </div>
                    <form onSubmit={handleProtect} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-red-50 file:text-red-700 hover:file:bg-red-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Password</label>
                            <input type="password" name="password" required className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-red-500 focus:border-red-500" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-red-600 hover:bg-red-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'protect' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Lock className="w-4 h-4" /> Protect PDF</>}
                        </button>
                    </form>
                </div>

                {/* Unlock PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-green-100 rounded-lg text-green-600">
                            <Unlock className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Unlock PDF</h3>
                    </div>
                    <form onSubmit={handleUnlock} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-green-50 file:text-green-700 hover:file:bg-green-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Password</label>
                            <input type="password" name="password" required className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-green-500 focus:border-green-500" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'unlock' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Unlock className="w-4 h-4" /> Unlock PDF</>}
                        </button>
                    </form>
                </div>

                {/* Sign PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-blue-100 rounded-lg text-blue-600">
                            <PenTool className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Sign PDF</h3>
                    </div>
                    <form onSubmit={handleSign} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Signature Image</label>
                            <input type="file" name="signature_file" accept="image/*" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100" />
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">Page</label>
                                <input type="number" name="page_num" defaultValue="1" min="1" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">X Position</label>
                                <input type="number" name="x" defaultValue="100" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">Y Position</label>
                                <input type="number" name="y" defaultValue="500" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">Width</label>
                                <input type="number" name="width" defaultValue="100" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'sign' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><PenTool className="w-4 h-4" /> Sign PDF</>}
                        </button>
                    </form>
                </div>

                {/* Watermark PDF */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-purple-100 rounded-lg text-purple-600">
                            <Stamp className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Watermark PDF</h3>
                    </div>
                    <form onSubmit={handleWatermark} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-purple-50 file:text-purple-700 hover:file:bg-purple-100" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Watermark Text</label>
                            <input type="text" name="text" required className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500" />
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">Opacity (0-1)</label>
                                <input type="number" name="opacity" defaultValue="0.3" step="0.1" min="0" max="1" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-gray-700 mb-1">Rotation</label>
                                <input type="number" name="rotation" defaultValue="45" className="w-full p-2 border border-gray-300 rounded-lg" />
                            </div>
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-purple-600 hover:bg-purple-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'watermark' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Stamp className="w-4 h-4" /> Add Watermark</>}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    );
};

export default SecurityTools;
