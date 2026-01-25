export interface RecentFile {
    name: string;
    size: number;
    lastOpened: string;
}

const STORAGE_KEY = 'pdf-editor-recent-files';
const MAX_FILES = 10;

export const getRecentFiles = (): RecentFile[] => {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (!stored) return [];
        const parsed = JSON.parse(stored);
        // Ensure we always return an array
        return Array.isArray(parsed) ? parsed : [];
    } catch (e) {
        return [];
    }
};

export const addRecentFile = (file: File) => {
    const current = getRecentFiles() ?? [];
    const newFile: RecentFile = {
        name: file.name,
        size: file.size,
        lastOpened: new Date().toISOString(),
    };

    const updated = [
        newFile,
        ...current.filter(f => f && f.name !== file.name)
    ].slice(0, MAX_FILES);

    localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));

    // Dispatch event for RecentFiles component to listen to
    window.dispatchEvent(new CustomEvent('pdf-opened', {
        detail: { fileName: file.name, fileSize: file.size }
    }));
};

export const removeRecentFile = (fileName: string) => {
    const current = getRecentFiles() ?? [];
    const updated = current.filter(f => f && f.name !== fileName);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
};

export const clearRecentFiles = () => {
    localStorage.removeItem(STORAGE_KEY);
};

export const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
};

export const formatDate = (isoString: string): string => {
    const date = new Date(isoString);
    const now = new Date();
    const diffTime = Math.abs(now.getTime() - date.getTime());
    const diffDays = Math.floor(diffTime / (1000 * 60 * 60 * 24));

    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays} days ago`;
    return date.toLocaleDateString();
};
