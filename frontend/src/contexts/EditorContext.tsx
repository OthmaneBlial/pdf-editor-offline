import React, { createContext, useContext, useState, type ReactNode, useEffect } from 'react';
import * as fabric from 'fabric';
import axios from 'axios';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000';

interface EditorState {
  document: File | null;
  currentPage: number;
  canvas: fabric.Canvas | null;
  drawingMode: string;
  color: string;
  strokeWidth: number;
  fontSize: number;
  fontFamily: string;
  canvasObjects: fabric.Object[];
  sessionId: string;
  hasUnsavedChanges: boolean;
  pageCount: number;
  zoom: number;
  isUploading: boolean;
}

interface EditorContextType extends EditorState {
  setDocument: (doc: File | null) => void;
  setCurrentPage: (page: number) => void;
  setCanvas: (canvas: fabric.Canvas | null) => void;
  setDrawingMode: (mode: string) => void;
  setColor: (color: string) => void;
  setStrokeWidth: (width: number) => void;
  setFontSize: (size: number) => void;
  setFontFamily: (font: string) => void;
  setCanvasObjects: (objects: fabric.Object[]) => void;
  setSessionId: (id: string) => void;
  setPageCount: (count: number) => void;
  setZoom: (zoom: number) => void;
  setIsUploading: (uploading: boolean) => void;
  undo: () => void;
  redo: () => void;
  saveChanges: (force?: boolean) => Promise<void>;
  exportPDF: () => Promise<void>;
  reorderPages: (fromIndex: number, toIndex: number) => void;
  // History state for UI
  history: string[];
  historyStep: number;
  canUndo: boolean;
  canRedo: boolean;
  clearHistory: () => void;
}

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
  const [document, setDocument] = useState<File | null>(null);
  const [currentPage, setCurrentPage] = useState<number>(0); // 0-indexed for backend
  const [canvas, setCanvas] = useState<fabric.Canvas | null>(null);
  const [drawingMode, setDrawingMode] = useState<string>('select');
  const [color, setColor] = useState<string>('#000000');
  const [strokeWidth, setStrokeWidth] = useState<number>(2);
  const [fontSize, setFontSize] = useState<number>(20);
  const [fontFamily, setFontFamily] = useState<string>('Arial');
  const [canvasObjects, setCanvasObjects] = useState<fabric.Object[]>([]);
  const [sessionId, setSessionId] = useState<string>('');
  const [pageCount, setPageCount] = useState<number>(1);
  const [zoom, setZoom] = useState<number>(1);
  const [isUploading, setIsUploading] = useState<boolean>(false);

  // History for undo
  const [history, setHistory] = useState<string[]>([]);
  const [historyStep, setHistoryStep] = useState<number>(-1);
  const isUndoing = React.useRef(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState<boolean>(false);

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
  }, [canvas, history, historyStep]);

  const undo = () => {
    if (canvas && historyStep > 0) {
      isUndoing.current = true;
      const previousState = history[historyStep - 1];
      setHistoryStep(prev => prev - 1);

      canvas.loadFromJSON(JSON.parse(previousState)).then(() => {
        canvas.renderAll();
        isUndoing.current = false;
        setHasUnsavedChanges(true); // Undo is a change
      });
    }
  };

  const redo = () => {
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
  };

  const saveChanges = async (force = false) => {
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
      // @ts-ignore - Fabric types mismatch for setting null/undefined on backgroundImage in strict mode
      canvas.backgroundImage = undefined;
      canvas.requestRenderAll();
      const overlayImage = canvas.toDataURL({ format: 'png', multiplier: 1 });
      canvas.backgroundImage = originalBg;
      if (originalBg) {
        canvas.requestRenderAll();
      }

      await axios.post(`${API_BASE_URL}/api/documents/${sessionId}/pages/${currentPage}/canvas`, {
        ...canvasData,
        overlay_image: overlayImage
      });
      setHasUnsavedChanges(false);
      // alert('Changes saved successfully!'); // Removed alert to be less intrusive during auto-save
    } catch (error) {
      console.error('Error saving changes:', error);
      alert('Failed to save changes.');
    }
  };

  const exportPDF = async () => {
    if (!sessionId) return;

    try {
      // Auto-save before export
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
      console.error('Error exporting PDF:', error);
      alert('Failed to export PDF.');
    }
  };

  const reorderPages = (fromIndex: number, toIndex: number) => {
    if (fromIndex === toIndex || !sessionId) return;

    // Create new page order array
    const pages = Array.from({ length: pageCount }, (_, i) => i);
    const [movedPage] = pages.splice(fromIndex, 1);
    pages.splice(toIndex, 0, movedPage);

    // TODO: Call API to persist reorder when backend supports it
    // For now, just update the current page if it was affected
    if (currentPage === fromIndex) {
      setCurrentPage(toIndex);
    } else if (fromIndex < currentPage && currentPage <= toIndex) {
      setCurrentPage(currentPage - 1);
    } else if (toIndex <= currentPage && currentPage < fromIndex) {
      setCurrentPage(currentPage + 1);
    }

    console.log(`Reordered page ${fromIndex + 1} to position ${toIndex + 1}`);
  };

  const value: EditorContextType = {
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
    undo,
    redo,
    saveChanges,
    exportPDF,
    reorderPages,
    // History state
    history,
    historyStep,
    canUndo: historyStep > 0,
    canRedo: historyStep < history.length - 1,
    clearHistory: () => {
      if (canvas) {
        const json = JSON.stringify(canvas.toJSON());
        setHistory([json]);
        setHistoryStep(0);
      }
    },
  };

  return (
    <EditorContext.Provider value={value}>
      {children}
    </EditorContext.Provider>
  );
};
