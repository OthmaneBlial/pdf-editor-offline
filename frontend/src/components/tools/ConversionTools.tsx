import React, { useState } from 'react';
import axios from 'axios';
import { FileText, Image, FileSpreadsheet, Presentation, Globe, RefreshCw, Code, BookOpen, FileType } from 'lucide-react';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

const ConversionTools: React.FC = () => {
    const [loading, setLoading] = useState<string | null>(null); // Track specific endpoint loading
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [activeTab, setActiveTab] = useState<'to_pdf' | 'from_pdf'>('from_pdf');

    const handleConversion = async (e: React.FormEvent<HTMLFormElement>, endpoint: string, outputName: string) => {
        e.preventDefault();
        setLoading(endpoint);
        setMessage(null);
        const formData = new FormData(e.currentTarget);

        try {
            const response = await axios.post(`${API_BASE_URL}/api/tools/${endpoint}`, formData, {
                responseType: 'blob'
            });
            const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || outputName;
            downloadFile(response.data, filename);
            setMessage({ type: 'success', text: 'Conversion successful!' });
        } catch (error) {
            setMessage({ type: 'error', text: 'Conversion failed.' });
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
        <div className="p-3 sm:p-6 lg:p-8 max-w-7xl mx-auto animate-fade-in">
            <div className="mb-8">
                <h2 className="text-3xl font-bold bg-gradient-to-r from-sky-700 to-cyan-700 bg-clip-text text-transparent mb-2">Conversion Tools</h2>
                <p className="text-gray-500">Transform your documents with ease</p>
            </div>

            {message && (
                <div className={`p-4 mb-6 rounded-xl shadow-lg ${message.type === 'success' ? 'bg-gradient-to-r from-green-50 to-emerald-50 text-green-700 border border-green-200' : 'bg-gradient-to-r from-red-50 to-pink-50 text-red-700 border border-red-200'} animate-scale-in`}>
                    {message.text}
                </div>
            )}

            {/* Tabs */}
            <div className="flex gap-3 sm:gap-6 mb-6 sm:mb-8 border-b border-sky-100 overflow-x-auto whitespace-nowrap pb-1">
                <button
                    onClick={() => setActiveTab('from_pdf')}
                    className={`pb-3 sm:pb-4 px-3 sm:px-6 text-sm sm:text-base font-semibold transition-all duration-300 relative ${activeTab === 'from_pdf' ? 'text-sky-700' : 'text-gray-500 hover:text-gray-700'}`}
                >
                    Convert From PDF
                    {activeTab === 'from_pdf' && <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-sky-600 to-cyan-600 rounded-full" />}
                </button>
                <button
                    onClick={() => setActiveTab('to_pdf')}
                    className={`pb-3 sm:pb-4 px-3 sm:px-6 text-sm sm:text-base font-semibold transition-all duration-300 relative ${activeTab === 'to_pdf' ? 'text-sky-700' : 'text-gray-500 hover:text-gray-700'}`}
                >
                    Convert To PDF
                    {activeTab === 'to_pdf' && <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-sky-600 to-cyan-600 rounded-full" />}
                </button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {activeTab === 'to_pdf' ? (
                    <>
                        {/* Word to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-purple-100/50 hover:border-purple-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-blue-100 to-blue-200 rounded-xl text-blue-600 shadow-md">
                                    <FileText className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">Word to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'word-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".docx,.doc" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-blue-50 file:to-blue-100 file:text-blue-700 hover:file:from-blue-100 hover:file:to-blue-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-blue-600 to-blue-700 hover:from-blue-700 hover:to-blue-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-blue-500/30 hover:shadow-blue-500/50 hover:-translate-y-0.5">
                                    {loading === 'word-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* Excel to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-green-100/50 hover:border-green-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-green-100 to-green-200 rounded-xl text-green-600 shadow-md">
                                    <FileSpreadsheet className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">Excel to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'excel-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".xlsx,.xls" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-green-50 file:to-green-100 file:text-green-700 hover:file:from-green-100 hover:file:to-green-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-green-600 to-green-700 hover:from-green-700 hover:to-green-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-green-500/30 hover:shadow-green-500/50 hover:-translate-y-0.5">
                                    {loading === 'excel-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* PPT to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-orange-100/50 hover:border-orange-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-orange-100 to-orange-200 rounded-xl text-orange-600 shadow-md">
                                    <Presentation className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PPT to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'ppt-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".pptx,.ppt" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-orange-50 file:to-orange-100 file:text-orange-700 hover:file:from-orange-100 hover:file:to-orange-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-orange-600 to-orange-700 hover:from-orange-700 hover:to-orange-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-orange-500/30 hover:shadow-orange-500/50 hover:-translate-y-0.5">
                                    {loading === 'ppt-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* Image to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-purple-100/50 hover:border-purple-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-purple-100 to-purple-200 rounded-xl text-purple-600 shadow-md">
                                    <Image className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">Image to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'img-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept="image/*" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-purple-50 file:to-purple-100 file:text-purple-700 hover:file:from-purple-100 hover:file:to-purple-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-purple-600 to-purple-700 hover:from-purple-700 hover:to-purple-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-purple-500/30 hover:shadow-purple-500/50 hover:-translate-y-0.5">
                                    {loading === 'img-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* HTML to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-gray-200 hover:border-gray-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-gray-100 to-gray-200 rounded-xl text-gray-600 shadow-md">
                                    <Globe className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">HTML to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'html-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".html,.htm" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-gray-50 file:to-gray-100 file:text-gray-700 hover:file:from-gray-100 hover:file:to-gray-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-gray-600 to-gray-700 hover:from-gray-700 hover:to-gray-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-gray-500/30 hover:shadow-gray-500/50 hover:-translate-y-0.5">
                                    {loading === 'html-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* Markdown to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-slate-200 hover:border-slate-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-slate-100 to-slate-200 rounded-xl text-slate-600 shadow-md">
                                    <Code className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">Markdown to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'markdown-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".md,.markdown" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-slate-50 file:to-slate-100 file:text-slate-700 hover:file:from-slate-100 hover:file:to-slate-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-slate-600 to-slate-700 hover:from-slate-700 hover:to-slate-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-slate-500/30 hover:shadow-slate-500/50 hover:-translate-y-0.5">
                                    {loading === 'markdown-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* TXT to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-stone-200 hover:border-stone-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-stone-100 to-stone-200 rounded-xl text-stone-600 shadow-md">
                                    <FileType className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">TXT to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'txt-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".txt" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-stone-50 file:to-stone-100 file:text-stone-700 hover:file:from-stone-100 hover:file:to-stone-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-stone-600 to-stone-700 hover:from-stone-700 hover:to-stone-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-stone-500/30 hover:shadow-stone-500/50 hover:-translate-y-0.5">
                                    {loading === 'txt-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* CSV to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-teal-200 hover:border-teal-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-teal-100 to-teal-200 rounded-xl text-teal-600 shadow-md">
                                    <FileSpreadsheet className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">CSV to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'csv-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".csv" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-teal-50 file:to-teal-100 file:text-teal-700 hover:file:from-teal-100 hover:file:to-teal-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-teal-600 to-teal-700 hover:from-teal-700 hover:to-teal-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-teal-500/30 hover:shadow-teal-500/50 hover:-translate-y-0.5">
                                    {loading === 'csv-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>

                        {/* JSON to PDF */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-amber-200 hover:border-amber-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-amber-100 to-amber-200 rounded-xl text-amber-600 shadow-md">
                                    <Code className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">JSON to PDF</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'json-to-pdf', 'converted.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".json" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-amber-50 file:to-amber-100 file:text-amber-700 hover:file:from-amber-100 hover:file:to-amber-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-amber-600 to-amber-700 hover:from-amber-700 hover:to-amber-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-amber-500/30 hover:shadow-amber-500/50 hover:-translate-y-0.5">
                                    {loading === 'json-to-pdf' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF'}
                                </button>
                            </form>
                        </div>
                    </>
                ) : (
                    <>
                        {/* PDF to Word */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-blue-100/50 hover:border-blue-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-blue-100 to-blue-200 rounded-xl text-blue-600 shadow-md">
                                    <FileText className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to Word</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-word', 'converted.docx')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-blue-50 file:to-blue-100 file:text-blue-700 hover:file:from-blue-100 hover:file:to-blue-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-blue-600 to-blue-700 hover:from-blue-700 hover:to-blue-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-blue-500/30 hover:shadow-blue-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-word' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to Word'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to Excel */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-green-100/50 hover:border-green-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-green-100 to-green-200 rounded-xl text-green-600 shadow-md">
                                    <FileSpreadsheet className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to Excel</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-excel', 'converted.xlsx')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-green-50 file:to-green-100 file:text-green-700 hover:file:from-green-100 hover:file:to-green-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-green-600 to-green-700 hover:from-green-700 hover:to-green-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-green-500/30 hover:shadow-green-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-excel' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to Excel'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to PPT */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-orange-100/50 hover:border-orange-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-orange-100 to-orange-200 rounded-xl text-orange-600 shadow-md">
                                    <Presentation className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to PPT</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-ppt', 'converted.pptx')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-orange-50 file:to-orange-100 file:text-orange-700 hover:file:from-orange-100 hover:file:to-orange-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-orange-600 to-orange-700 hover:from-orange-700 hover:to-orange-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-orange-500/30 hover:shadow-orange-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-ppt' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PPT'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to JPG */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-purple-100/50 hover:border-purple-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-purple-100 to-purple-200 rounded-xl text-purple-600 shadow-md">
                                    <Image className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to JPG</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-jpg', 'converted.zip')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-purple-50 file:to-purple-100 file:text-purple-700 hover:file:from-purple-100 hover:file:to-purple-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-purple-600 to-purple-700 hover:from-purple-700 hover:to-purple-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-purple-500/30 hover:shadow-purple-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-jpg' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to Images'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to PDF/A */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-red-100/50 hover:border-red-300">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-red-100 to-red-200 rounded-xl text-red-600 shadow-md">
                                    <FileText className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to PDF/A</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-pdfa', 'converted_pdfa.pdf')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-red-50 file:to-red-100 file:text-red-700 hover:file:from-red-100 hover:file:to-red-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-red-600 to-red-700 hover:from-red-700 hover:to-red-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-red-500/30 hover:shadow-red-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-pdfa' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to PDF/A'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to Markdown */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-slate-200 hover:border-slate-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-slate-100 to-slate-200 rounded-xl text-slate-600 shadow-md">
                                    <Code className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to Markdown</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-markdown', 'converted.md')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-slate-50 file:to-slate-100 file:text-slate-700 hover:file:from-slate-100 hover:file:to-slate-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-slate-600 to-slate-700 hover:from-slate-700 hover:to-slate-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-slate-500/30 hover:shadow-slate-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-markdown' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to Markdown'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to TXT */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-stone-200 hover:border-stone-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-stone-100 to-stone-200 rounded-xl text-stone-600 shadow-md">
                                    <FileType className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to TXT</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-txt', 'converted.txt')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-stone-50 file:to-stone-100 file:text-stone-700 hover:file:from-stone-100 hover:file:to-stone-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-stone-600 to-stone-700 hover:from-stone-700 hover:to-stone-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-stone-500/30 hover:shadow-stone-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-txt' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to TXT'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to EPUB */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-indigo-200 hover:border-indigo-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-indigo-100 to-indigo-200 rounded-xl text-indigo-600 shadow-md">
                                    <BookOpen className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to EPUB</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-epub', 'converted.epub')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-indigo-50 file:to-indigo-100 file:text-indigo-700 hover:file:from-indigo-100 hover:file:to-indigo-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-indigo-600 to-indigo-700 hover:from-indigo-700 hover:to-indigo-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-indigo-500/30 hover:shadow-indigo-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-epub' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to EPUB'}
                                </button>
                            </form>
                        </div>

                        {/* PDF to SVG */}
                        <div className="card-interactive bg-white p-6 rounded-2xl shadow-lg border border-violet-200 hover:border-violet-400">
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-3 bg-gradient-to-br from-violet-100 to-violet-200 rounded-xl text-violet-600 shadow-md">
                                    <Image className="w-6 h-6" />
                                </div>
                                <h3 className="font-bold text-lg text-gray-800">PDF to SVG</h3>
                            </div>
                            <form onSubmit={(e) => handleConversion(e, 'pdf-to-svg', 'converted_svg.zip')} className="space-y-4">
                                <input type="file" name="file" accept=".pdf" required className="w-full text-sm text-gray-600 file:mr-4 file:py-2.5 file:px-5 file:rounded-xl file:border-0 file:text-sm file:font-semibold file:bg-gradient-to-r file:from-violet-50 file:to-violet-100 file:text-violet-700 hover:file:from-violet-100 hover:file:to-violet-200 file:transition-all file:cursor-pointer" />
                                <button type="submit" disabled={!!loading} className="w-full py-3 px-4 bg-gradient-to-r from-violet-600 to-violet-700 hover:from-violet-700 hover:to-violet-800 text-white rounded-xl font-semibold transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2 shadow-lg shadow-violet-500/30 hover:shadow-violet-500/50 hover:-translate-y-0.5">
                                    {loading === 'pdf-to-svg' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Converting...</> : 'Convert to SVG'}
                                </button>
                            </form>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
};

export default ConversionTools;
