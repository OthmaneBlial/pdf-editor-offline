/* eslint-disable react-refresh/only-export-components */
/* eslint-disable react-hooks/immutability */
import React, { createContext, useContext, useState, useRef, useCallback, useMemo, type ReactNode, useEffect } from 'react';
import * as fabric from 'fabric';
import axios from 'axios';
import type { EditorContextType, EditorState, CanvasState, HistoryState, TOCItem, FontInfo, ToolToast } from './types';
import { MAX_HISTORY_SIZE } from './types';
import { getApiData, isApiSuccess } from '../utils/apiResponse';
import { API_BASE_URL } from '../lib/apiClient';

const EditorContext = createContext<EditorContextType | undefined>(undefined);

export const useEditor = () => {
  const context = useContext(EditorContext);
  if (!context) {
    throw new Error('useEditor must be used within an EditorProvider');
  }
  return context;
};

interface EditorProviderProps {
  children: ReactNode;
}

export const EditorProvider: React.FC<EditorProviderProps> = ({ children }) => {
  // Editor state (document-related, changes less frequently)
  const [document, setDocument] = useState<File | null>(null);
  const [sessionId, setSessionId] = useState<string>('');
  const [pageCount, setPageCount] = useState<number>(1);
  const [isUploading, setIsUploading] = useState<boolean>(false);

  // Canvas state (rendering-related)
  const [canvas, setCanvas] = useState<fabric.Canvas | null>(null);
  const [currentPage, setCurrentPage] = useState<number>(0);
  const [zoom, setZoom] = useState<number>(1);
  const [canvasObjects, setCanvasObjects] = useState<fabric.Object[]>([]);

  // Tool settings (changes frequently)
  const [drawingMode, setDrawingMode] = useState<string>('select');
  const [color, setColor] = useState<string>('#000000');
  const [strokeWidth, setStrokeWidth] = useState<number>(2);
  const [fontSize, setFontSize] = useState<number>(20);
  const [fontFamily, setFontFamily] = useState<string>('Arial');

  // History state
  const [history, setHistory] = useState<string[]>([]);
  const [historyStep, setHistoryStep] = useState<number>(-1);
  const isUndoing = useRef(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState<boolean>(false);

  // Advanced Editing state
  const [toc, setToc] = useState<TOCItem[]>([]);
  const [fonts, setFonts] = useState<FontInfo[]>([]);
  const [bookmarks, setBookmarks] = useState<TOCItem[]>([]);
  const [showTOC, setShowTOC] = useState<boolean>(false);
  const [documentMutationVersion, setDocumentMutationVersion] = useState<number>(0);
  const [toolToast, setToolToast] = useState<ToolToast | null>(null);
  const toastTimeoutRef = useRef<number | null>(null);

  // Limit history size to prevent memory issues
  useEffect(() => {
    if (history.length > MAX_HISTORY_SIZE) {
      setHistory(prev => prev.slice(-MAX_HISTORY_SIZE));
      setHistoryStep(MAX_HISTORY_SIZE - 1);
    }
  }, [history.length]);

  // Initialize history when canvas is ready
  useEffect(() => {
    if (canvas) {
      const saveHistory = () => {
        if (isUndoing.current) return;

        const json = JSON.stringify(canvas.toJSON());

        if (historyStep < history.length - 1) {
          const newHistory = history.slice(0, historyStep + 1);
          setHistory([...newHistory, json]);
          setHistoryStep(newHistory.length);
        } else {
          setHistory(prev => [...prev, json]);
          setHistoryStep(prev => prev + 1);
        }
        setHasUnsavedChanges(true);
      };

      if (history.length === 0) {
        const json = JSON.stringify(canvas.toJSON());
        setHistory([json]);
        setHistoryStep(0);
      }

      canvas.on('object:added', saveHistory);
      canvas.on('object:modified', saveHistory);
      canvas.on('object:removed', saveHistory);

      return () => {
        canvas.off('object:added', saveHistory);
        canvas.off('object:modified', saveHistory);
        canvas.off('object:removed', saveHistory);
      };
    }
  }, [canvas, historyStep, history]);

  // Memoized undo function
  const undo = useCallback(() => {
    if (canvas && historyStep > 0) {
      isUndoing.current = true;
      const previousState = history[historyStep - 1];
      setHistoryStep(prev => prev - 1);

      canvas.loadFromJSON(JSON.parse(previousState)).then(() => {
        canvas.renderAll();
        isUndoing.current = false;
        setHasUnsavedChanges(true);
      });
    }
  }, [canvas, history, historyStep]);

  // Memoized redo function
  const redo = useCallback(() => {
    if (canvas && historyStep < history.length - 1) {
      isUndoing.current = true;
      const nextState = history[historyStep + 1];
      setHistoryStep(prev => prev + 1);

      canvas.loadFromJSON(JSON.parse(nextState)).then(() => {
        canvas.renderAll();
        isUndoing.current = false;
        setHasUnsavedChanges(true);
      });
    }
  }, [canvas, history, historyStep]);

  // Memoized save changes function
  const saveChanges = useCallback(async (force = false) => {
    if (!sessionId || !canvas) return;
    if (!force && !hasUnsavedChanges) return;

    try {
      const objects = canvas.getObjects().map(obj => obj.toObject());
      const canvasData = {
        objects: objects,
        zoom: canvas.getZoom(),
        background_image: ''
      };

      const originalBg = canvas.backgroundImage;
      // Properly handle background image - fabric.js requires direct mutation
      if (originalBg) {
        canvas.backgroundImage = undefined;
      }
      canvas.requestRenderAll();
      const overlayImage = canvas.toDataURL({ format: 'png', multiplier: 1 });
      if (originalBg) {
        canvas.backgroundImage = originalBg;
        canvas.requestRenderAll();
      }

      await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/canvas`, {
        ...canvasData,
        overlay_image: overlayImage
      });
      setHasUnsavedChanges(false);
    } catch (error) {
      // Use proper error handling instead of alert
      if (error instanceof Error) {
        // Could use a toast notification system here
        console.error('Failed to save changes:', error.message);
      }
    }
  }, [sessionId, canvas, currentPage, hasUnsavedChanges]);

  // Memoized export function
  const exportPDF = useCallback(async () => {
    if (!sessionId) return;

    try {
      await saveChanges(true);

      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/download`, {
        responseType: 'blob'
      });

      const filename = response.headers['content-disposition']?.split('filename=')[1]?.replace(/"/g, '') || `document_${sessionId}.pdf`;
      const url = window.URL.createObjectURL(response.data);
      const link = window.document.createElement('a');
      link.href = url;
      link.setAttribute('download', filename);
      window.document.body.appendChild(link);
      link.click();
      link.remove();
      window.URL.revokeObjectURL(url);
    } catch (error) {
      if (error instanceof Error) {
        console.error('Failed to export PDF:', error.message);
      }
    }
  }, [sessionId, saveChanges]);

  // Memoized reorder pages function
  const reorderPages = useCallback((fromIndex: number, toIndex: number) => {
    if (fromIndex === toIndex || !sessionId) return;

    const pages = Array.from({ length: pageCount }, (_, i) => i);
    const [movedPage] = pages.splice(fromIndex, 1);
    pages.splice(toIndex, 0, movedPage);

    // TODO: Call API to persist reorder when backend supports it
    if (currentPage === fromIndex) {
      setCurrentPage(toIndex);
    } else if (fromIndex < currentPage && currentPage <= toIndex) {
      setCurrentPage(currentPage - 1);
    } else if (toIndex <= currentPage && currentPage < fromIndex) {
      setCurrentPage(currentPage + 1);
    }
  }, [sessionId, pageCount, currentPage]);

  // Memoized clear history function
  const clearHistory = useCallback(() => {
    if (canvas) {
      const json = JSON.stringify(canvas.toJSON());
      setHistory([json]);
      setHistoryStep(0);
    }
  }, [canvas]);

  const clearToolToast = useCallback(() => {
    if (toastTimeoutRef.current) {
      window.clearTimeout(toastTimeoutRef.current);
      toastTimeoutRef.current = null;
    }
    setToolToast(null);
  }, []);

  const reportToolResult = useCallback((
    type: 'success' | 'error',
    text: string,
    refreshDocument: boolean = false
  ) => {
    if (refreshDocument) {
      setDocumentMutationVersion(prev => prev + 1);
    }
    setToolToast({ type, text });
    if (toastTimeoutRef.current) {
      window.clearTimeout(toastTimeoutRef.current);
    }
    toastTimeoutRef.current = window.setTimeout(() => {
      setToolToast(null);
      toastTimeoutRef.current = null;
    }, 5000);
  }, []);

  // Advanced Editing actions
  const loadTOC = useCallback(async () => {
    if (!sessionId) return;
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/toc`);
      const data = getApiData<{ toc?: TOCItem[] }>(response.data);
      if (!data) return;
      setToc(Array.isArray(data.toc) ? data.toc : []);
    } catch (error) {
      console.error('Failed to load TOC:', error);
    }
  }, [sessionId]);

  const addBookmark = useCallback(async (item: Omit<TOCItem, 'page'>) => {
    if (!sessionId) return;
    try {
      const response = await axios.post(
        `${API_BASE_URL}/api/documents/${sessionId}/bookmarks`,
        null,
        { params: { level: item.level, title: item.title, page_num: currentPage + 1 } }
      );
      if (isApiSuccess(response.data)) {
        loadTOC();
      }
    } catch (error) {
      console.error('Failed to add bookmark:', error);
      throw error;
    }
  }, [sessionId, currentPage, loadTOC]);

  const deleteBookmark = useCallback(async (index: number) => {
    if (!sessionId) return;
    try {
      const response = await axios.delete(`${API_BASE_URL}/api/documents/${sessionId}/bookmarks/${index}`);
      if (isApiSuccess(response.data)) {
        loadTOC();
      }
    } catch (error) {
      console.error('Failed to delete bookmark:', error);
      throw error;
    }
  }, [sessionId, loadTOC]);

  const loadFonts = useCallback(async () => {
    if (!sessionId) return;
    try {
      const response = await axios.get(`${API_BASE_URL}/api/documents/${sessionId}/fonts/${currentPage}`);
      const data = getApiData<{ fonts?: FontInfo[] }>(response.data);
      if (!data) return;
      setFonts(Array.isArray(data.fonts) ? data.fonts : []);
    } catch (error) {
      console.error('Failed to load fonts:', error);
    }
  }, [sessionId, currentPage]);

  // Load TOC when sessionId changes
  useEffect(() => {
    if (sessionId) {
      loadTOC();
    }
  }, [sessionId, loadTOC]);

  useEffect(() => {
    return () => {
      if (toastTimeoutRef.current) {
        window.clearTimeout(toastTimeoutRef.current);
      }
    };
  }, []);

  // Memoized context value to prevent unnecessary re-renders
  const value = useMemo<EditorContextType>(() => ({
    // State
    document,
    currentPage,
    canvas,
    drawingMode,
    color,
    strokeWidth,
    fontSize,
    fontFamily,
    canvasObjects,
    sessionId,
    hasUnsavedChanges,
    pageCount,
    zoom,
    isUploading,
    // Setters
    setDocument,
    setCurrentPage,
    setCanvas,
    setDrawingMode,
    setColor,
    setStrokeWidth,
    setFontSize,
    setFontFamily,
    setCanvasObjects,
    setSessionId,
    setPageCount,
    setZoom,
    setIsUploading,
    // Actions
    undo,
    redo,
    saveChanges,
    exportPDF,
    reorderPages,
    clearHistory,
    // Advanced Editing
    toc,
    setToc,
    fonts,
    setFonts,
    bookmarks,
    setBookmarks,
    showTOC,
    setShowTOC,
    loadTOC,
    addBookmark,
    deleteBookmark,
    loadFonts,
    documentMutationVersion,
    toolToast,
    reportToolResult,
    clearToolToast,
    // History state
    history,
    historyStep,
    canUndo: historyStep > 0,
    canRedo: historyStep < history.length - 1,
  }), [
    document,
    currentPage,
    canvas,
    drawingMode,
    color,
    strokeWidth,
    fontSize,
    fontFamily,
    canvasObjects,
    sessionId,
    hasUnsavedChanges,
    pageCount,
    zoom,
    isUploading,
    history,
    historyStep,
    undo,
    redo,
    saveChanges,
    exportPDF,
    reorderPages,
    clearHistory,
    toc,
    fonts,
    bookmarks,
    showTOC,
    loadTOC,
    addBookmark,
    deleteBookmark,
    loadFonts,
    documentMutationVersion,
    toolToast,
    reportToolResult,
    clearToolToast,
  ]);

  return (
    <EditorContext.Provider value={value}>
      {children}
    </EditorContext.Provider>
  );
};

// Re-export types for convenience
export type { EditorContextType, EditorState, CanvasState, HistoryState };
