import React, { useState } from 'react';
import axios from 'axios';
import { Lock, Unlock, PenTool, Stamp, RefreshCw, ShieldCheck, Eraser, Trash2 } from 'lucide-react';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';

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
            await saveBlob(response.data, 'protected.pdf');
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
            await saveBlob(response.data, 'unlocked.pdf');
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
            await saveBlob(response.data, 'signed.pdf');
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
            await saveBlob(response.data, 'watermarked.pdf');
            setMessage({ type: 'success', text: 'Watermark added successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to add watermark.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleCleanMetadata = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('metadata');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/clean-metadata`, formData, {
                responseType: 'blob'
            });
            await saveBlob(response.data, 'metadata-cleaned.pdf');
            setMessage({ type: 'success', text: 'Metadata cleaned successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to clean metadata.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleCleanHiddenData = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('privacy');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/clean-hidden-data`, formData, {
                responseType: 'blob'
            });
            await saveBlob(response.data, 'privacy-cleaned.pdf');
            setMessage({ type: 'success', text: 'Hidden data cleaned successfully!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to clean hidden data.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const handleMaintenanceCleanup = async (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading('cleanup');
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            await axios.post(`${API_BASE_URL}/api/documents/maintenance/cleanup`, {
                temp_max_age_minutes: Number(formData.get('temp_max_age_minutes')),
                session_max_age_hours: Number(formData.get('session_max_age_hours')),
                include_active_sessions: formData.get('include_active_sessions') === 'true',
            });
            setMessage({ type: 'success', text: 'Maintenance cleanup completed!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Failed to run maintenance cleanup.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
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
                            <input type="password" name="password" minLength={8} required className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-red-500 focus:border-red-500" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Owner Password</label>
                            <input type="password" name="owner_password" minLength={8} placeholder="Optional" className="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-red-500 focus:border-red-500" />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Encryption</label>
                            <select name="encryption" defaultValue="aes-256" className="w-full p-2 border border-gray-300 rounded-lg">
                                <option value="aes-256">AES-256</option>
                                <option value="aes-128">AES-128</option>
                            </select>
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            {[
                                ['allow_print', 'Print', 'true'],
                                ['allow_copy', 'Copy Text', 'false'],
                                ['allow_edit', 'Edit Content', 'false'],
                                ['allow_annotate', 'Annotate', 'false'],
                                ['allow_form', 'Fill Forms', 'true'],
                                ['allow_accessibility', 'Accessibility', 'true'],
                                ['allow_assemble', 'Assemble Pages', 'false'],
                                ['allow_high_quality_print', 'High Quality Print', 'true'],
                            ].map(([name, label, defaultValue]) => (
                                <label key={name} className="text-xs font-medium text-gray-700">
                                    {label}
                                    <select name={name} defaultValue={defaultValue} className="mt-1 w-full p-2 border border-gray-300 rounded-lg text-sm">
                                        <option value="true">Allow</option>
                                        <option value="false">Block</option>
                                    </select>
                                </label>
                            ))}
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

                {/* Metadata Cleaner */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-amber-100 rounded-lg text-amber-700">
                            <Eraser className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Clean Metadata</h3>
                    </div>
                    <form onSubmit={handleCleanMetadata} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-amber-50 file:text-amber-700 hover:file:bg-amber-100" />
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-amber-600 hover:bg-amber-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'metadata' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Eraser className="w-4 h-4" /> Clean Metadata</>}
                        </button>
                    </form>
                </div>

                {/* Hidden Data Cleanup */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-slate-100 rounded-lg text-slate-700">
                            <ShieldCheck className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Clean Hidden Data</h3>
                    </div>
                    <form onSubmit={handleCleanHiddenData} className="space-y-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-1">Upload PDF</label>
                            <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-slate-50 file:text-slate-700 hover:file:bg-slate-100" />
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            {[
                                ['remove_metadata', 'Metadata', 'true'],
                                ['remove_embedded_files', 'Embedded Files', 'true'],
                                ['remove_hidden_text', 'Hidden Text', 'true'],
                                ['remove_javascript', 'JavaScript', 'true'],
                                ['remove_links', 'Links', 'false'],
                                ['remove_annotations', 'Annotations', 'false'],
                                ['reset_form_fields', 'Form Fields', 'false'],
                                ['clean_pages', 'Page Streams', 'true'],
                            ].map(([name, label, defaultValue]) => (
                                <label key={name} className="text-xs font-medium text-gray-700">
                                    {label}
                                    <select name={name} defaultValue={defaultValue} className="mt-1 w-full p-2 border border-gray-300 rounded-lg text-sm">
                                        <option value="true">Clean</option>
                                        <option value="false">Keep</option>
                                    </select>
                                </label>
                            ))}
                        </div>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-slate-700 hover:bg-slate-800 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'privacy' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><ShieldCheck className="w-4 h-4" /> Clean Hidden Data</>}
                        </button>
                    </form>
                </div>

                {/* Maintenance Cleanup */}
                <div className="bg-white p-6 rounded-xl shadow-sm border border-gray-200">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-zinc-100 rounded-lg text-zinc-700">
                            <Trash2 className="w-6 h-6" />
                        </div>
                        <h3 className="font-semibold text-lg">Maintenance Cleanup</h3>
                    </div>
                    <form onSubmit={handleMaintenanceCleanup} className="space-y-4">
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            <label className="text-xs font-medium text-gray-700">
                                Temp Age Minutes
                                <input type="number" name="temp_max_age_minutes" defaultValue="60" min="0" max="1440" className="mt-1 w-full p-2 border border-gray-300 rounded-lg text-sm" />
                            </label>
                            <label className="text-xs font-medium text-gray-700">
                                Session Age Hours
                                <input type="number" name="session_max_age_hours" defaultValue="24" min="1" max="168" className="mt-1 w-full p-2 border border-gray-300 rounded-lg text-sm" />
                            </label>
                        </div>
                        <label className="text-xs font-medium text-gray-700 block">
                            Active Sessions
                            <select name="include_active_sessions" defaultValue="false" className="mt-1 w-full p-2 border border-gray-300 rounded-lg text-sm">
                                <option value="false">Keep</option>
                                <option value="true">Remove Expired</option>
                            </select>
                        </label>
                        <button type="submit" disabled={!!loading} className="w-full py-2 px-4 bg-zinc-700 hover:bg-zinc-800 text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'cleanup' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Processing...</> : <><Trash2 className="w-4 h-4" /> Run Cleanup</>}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    );
};

export default SecurityTools;
