import React, { useEffect, useState } from 'react';
import axios from 'axios';
import { Download, FileDiff, GitCompare, Minimize2, RefreshCw, Scan, ShieldCheck, Text, Wrench } from 'lucide-react';
import { API_BASE_URL } from '../../lib/apiClient';
import { saveBlob } from '../../lib/downloads';
import WorkflowFeedback from '../WorkflowFeedback';

interface ChangeReviewArtifact {
    name: string;
    media_type: string;
    url: string;
    content_bearing: boolean;
}

interface VisualReviewPage {
    page: number;
    changed_ratio_outside_expected: number;
    within_tolerance: boolean;
    artifacts: {
        before: string | null;
        after: string | null;
        overlay: string | null;
    };
}

interface ChangeReviewReport {
    verdict: string;
    audit_sha256: string;
    safe_to_publish: boolean;
    warnings: string[];
    visual: {
        pages: VisualReviewPage[];
        unexpected_pages: number;
    };
    semantic: {
        changed_text_pages: number;
        characters_added: number;
        characters_removed: number;
        changed_metadata_keys: number;
    };
    objects: {
        pages_changed: number;
    };
    annotation_history: {
        added: number;
        removed: number;
        modified: number;
    };
}

interface ChangeReviewData {
    review_id: string;
    report: ChangeReviewReport;
    artifacts: ChangeReviewArtifact[];
    expires_in_hours: number;
}

const AdvancedTools: React.FC = () => {
    const [loading, setLoading] = useState<string | null>(null);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [review, setReview] = useState<ChangeReviewData | null>(null);
    const [previewUrls, setPreviewUrls] = useState<Record<string, string>>({});

    useEffect(() => {
        let active = true;
        const createdUrls: string[] = [];
        setPreviewUrls({});
        if (!review) return () => undefined;
        const imageArtifacts = review.artifacts.filter((item) => item.media_type === 'image/png');
        void Promise.all(imageArtifacts.map(async (artifact) => {
            const response = await axios.get(`${API_BASE_URL.replace(/\/$/, '')}${artifact.url}`, { responseType: 'blob' });
            const url = URL.createObjectURL(response.data);
            createdUrls.push(url);
            return [artifact.name, url] as const;
        })).then((entries) => {
            if (active) setPreviewUrls(Object.fromEntries(entries));
        }).catch((error) => {
            console.error(error);
            if (active) setMessage({ type: 'error', text: 'The review completed, but its private previews could not be loaded.' });
        });
        return () => {
            active = false;
            createdUrls.forEach((url) => URL.revokeObjectURL(url));
        };
    }, [review]);

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
            await saveBlob(response.data, filename);
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
            await saveBlob(response.data, filename);
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
            await saveBlob(response.data, filename);
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
            await saveBlob(response.data, filename);
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
        setReview(null);
        const formData = new FormData(e.currentTarget);
        const before = formData.get('before');
        const after = formData.get('after');
        const requireSafeOutput = formData.get('safe_mode') === 'on';
        formData.delete('safe_mode');

        try {
            const response = await axios.post<{ data: ChangeReviewData }>(`${API_BASE_URL}/api/tools/change-review`, formData);
            const data = response.data.data;
            setReview(data);
            if (requireSafeOutput && data.report.safe_to_publish && before instanceof File && after instanceof File) {
                const safeData = new FormData();
                safeData.append('before', before);
                safeData.append('candidate', after);
                const safeResponse = await axios.post(`${API_BASE_URL}/api/tools/safe-edit`, safeData, {
                    responseType: 'blob'
                });
                await saveBlob(safeResponse.data, 'safe-edited.pdf');
                setMessage({ type: 'success', text: 'Review passed. The verified candidate was downloaded as a separate safe copy.' });
            } else if (requireSafeOutput) {
                setMessage({ type: 'error', text: 'Safe edit refused the candidate. Review the detected structural-loss warnings below.' });
            } else {
                setMessage({ type: 'success', text: 'Local visual and semantic review is ready.' });
            }
        } catch (error) {
            setMessage({ type: 'error', text: 'The local change review failed safely.' });
            console.error(error);
        } finally {
            setLoading(null);
        }
    };

    const reviewPage = review?.report.visual.pages.find((page) => !page.within_tolerance)
        ?? review?.report.visual.pages[0];
    const artifactUrl = (name: string | null | undefined) => {
        if (!name) return null;
        return previewUrls[name] ?? null;
    };
    const downloadReviewArtifact = async (artifact: ChangeReviewArtifact) => {
        try {
            const response = await axios.get(`${API_BASE_URL.replace(/\/$/, '')}${artifact.url}`, { responseType: 'blob' });
            await saveBlob(response.data, artifact.name);
        } catch (error) {
            console.error(error);
            setMessage({ type: 'error', text: `Could not download ${artifact.name}.` });
        }
    };
    const textDiffArtifact = review?.artifacts.find((item) => item.name.endsWith('-text.diff'));
    const metadataArtifact = review?.artifacts.find((item) => item.name === 'metadata-diff.json');
    const annotationsArtifact = review?.artifacts.find((item) => item.name === 'annotation-history.json');

    return (
        <div className="p-3 sm:p-6 max-w-6xl mx-auto">
            <h2 className="text-2xl font-bold mb-6 text-gray-800">Advanced Tools</h2>

            {message && (
                <WorkflowFeedback tone={message.type} title="Advanced operation result" className="mb-6">
                    {message.text}
                </WorkflowFeedback>
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

                {/* Change review */}
                <div className="bg-slate-950 p-6 rounded-2xl shadow-sm border border-slate-700 text-white md:col-span-2 lg:col-span-2">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 bg-cyan-300 rounded-lg text-slate-950">
                            <GitCompare className="w-6 h-6" />
                        </div>
                        <div>
                            <h3 className="font-semibold text-lg">Visual + semantic review</h3>
                            <p className="text-xs text-slate-400">Render, objects, text, metadata, and annotations</p>
                        </div>
                    </div>
                    <form onSubmit={handleCompare} className="space-y-4">
                        <div className="grid gap-3 sm:grid-cols-2">
                            <div>
                                <label className="block text-sm font-medium text-slate-200 mb-1">Before PDF</label>
                                <input type="file" name="before" accept=".pdf" required className="w-full min-h-11 text-sm text-slate-300 file:mr-3 file:py-2 file:px-3 file:rounded-lg file:border-0 file:text-sm file:font-semibold file:bg-slate-800 file:text-cyan-200 hover:file:bg-slate-700" />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-slate-200 mb-1">After / candidate PDF</label>
                                <input type="file" name="after" accept=".pdf" required className="w-full min-h-11 text-sm text-slate-300 file:mr-3 file:py-2 file:px-3 file:rounded-lg file:border-0 file:text-sm file:font-semibold file:bg-slate-800 file:text-cyan-200 hover:file:bg-slate-700" />
                            </div>
                        </div>
                        <label className="flex min-h-11 items-center gap-3 rounded-xl border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-200">
                            <input type="checkbox" name="safe_mode" className="h-5 w-5 rounded border-slate-500 text-cyan-400 focus:ring-cyan-300" />
                            <span><strong className="block text-white">Safe edit mode</strong><span className="text-xs text-slate-400">Download the candidate only when no structural loss is detected.</span></span>
                        </label>
                        <button type="submit" disabled={!!loading} className="w-full min-h-11 py-2 px-4 bg-cyan-300 hover:bg-cyan-200 text-slate-950 rounded-xl font-bold transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                            {loading === 'compare' ? <><RefreshCw className="w-4 h-4 animate-spin" /> Reviewing locally...</> : <><FileDiff className="w-4 h-4" /> Review both PDFs</>}
                        </button>
                    </form>
                </div>
            </div>

            {review && reviewPage && (
                <section className="mt-8 overflow-hidden rounded-3xl border border-slate-700 bg-slate-950 text-white" aria-labelledby="change-review-heading">
                    <div className="flex flex-col gap-4 border-b border-slate-800 p-5 sm:flex-row sm:items-center sm:justify-between">
                        <div>
                            <p className="text-xs font-black uppercase tracking-[0.22em] text-cyan-300">Content-free audit</p>
                            <h3 id="change-review-heading" className="mt-1 text-2xl font-black">{review.report.verdict.replaceAll('_', ' ')}</h3>
                            <p className="mt-1 break-all font-mono text-[11px] text-slate-500">Audit SHA-256 {review.report.audit_sha256}</p>
                        </div>
                        <div className={`inline-flex min-h-11 items-center gap-2 self-start rounded-full px-4 py-2 text-sm font-bold ${review.report.safe_to_publish ? 'bg-lime-300 text-slate-950' : 'bg-rose-400 text-slate-950'}`}>
                            <ShieldCheck className="h-4 w-4" />
                            {review.report.safe_to_publish ? 'Safe to publish' : 'Structural loss detected'}
                        </div>
                    </div>

                    <div className="grid gap-px bg-slate-800 sm:grid-cols-2 lg:grid-cols-4">
                        {[
                            ['Text pages', review.report.semantic.changed_text_pages],
                            ['Changed objects', review.report.objects.pages_changed],
                            ['Metadata keys', review.report.semantic.changed_metadata_keys],
                            ['Annotation events', review.report.annotation_history.added + review.report.annotation_history.removed + review.report.annotation_history.modified],
                        ].map(([label, value]) => (
                            <div key={label} className="bg-slate-950 p-4"><strong className="block text-2xl text-cyan-200">{value}</strong><span className="text-xs text-slate-400">{label}</span></div>
                        ))}
                    </div>

                    <div className="grid gap-4 p-5 lg:grid-cols-3">
                        {([
                            ['Before', artifactUrl(reviewPage.artifacts.before)],
                            ['After', artifactUrl(reviewPage.artifacts.after)],
                            ['Change overlay', artifactUrl(reviewPage.artifacts.overlay)],
                        ] as Array<[string, string | null]>).map(([label, url]) => url && (
                            <figure key={label} className="overflow-hidden rounded-2xl border border-slate-700 bg-white">
                                <img src={url} alt={`${label} rendering for page ${reviewPage.page}`} className="aspect-[.71] w-full object-contain" />
                                <figcaption className="border-t border-slate-200 px-3 py-2 text-xs font-bold text-slate-700">{label} · page {reviewPage.page}</figcaption>
                            </figure>
                        ))}
                    </div>

                    {(review.report.warnings.length > 0 || textDiffArtifact || metadataArtifact || annotationsArtifact) && (
                        <div className="grid gap-5 border-t border-slate-800 p-5 lg:grid-cols-2">
                            <div>
                                <h4 className="text-sm font-bold text-slate-200">Structural-loss warnings</h4>
                                {review.report.warnings.length ? <ul className="mt-2 space-y-2 text-sm text-rose-200">{review.report.warnings.map((warning) => <li key={warning}>• {warning.replaceAll('_', ' ')}</li>)}</ul> : <p className="mt-2 text-sm text-slate-400">No lossy transformation detected.</p>}
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-slate-200">Private local artifacts</h4>
                                <p className="mt-1 text-xs text-slate-500">These files can contain document content and expire in {review.expires_in_hours} hours.</p>
                                <div className="mt-3 flex flex-wrap gap-2">
                                    {[textDiffArtifact, metadataArtifact, annotationsArtifact].filter((item): item is ChangeReviewArtifact => Boolean(item)).map((artifact) => (
                                        <button key={artifact.name} type="button" onClick={() => void downloadReviewArtifact(artifact)} className="inline-flex min-h-11 items-center gap-2 rounded-xl border border-slate-700 px-3 py-2 text-xs font-bold text-cyan-200 hover:border-cyan-300">
                                            <Download className="h-4 w-4" /> {artifact.name}
                                        </button>
                                    ))}
                                </div>
                            </div>
                        </div>
                    )}
                </section>
            )}
        </div>
    );
};

export default AdvancedTools;
