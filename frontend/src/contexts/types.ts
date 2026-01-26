/**
 * Type definitions for Editor Context
 * Centralized types for better maintainability
 */

import type * as fabric from 'fabric';

// Core editor state
export interface EditorState {
  document: File | null;
  sessionId: string;
  hasUnsavedChanges: boolean;
  pageCount: number;
  isUploading: boolean;
}

// Canvas-related state (separated for performance)
export interface CanvasState {
  canvas: fabric.Canvas | null;
  currentPage: number;
  zoom: number;
  drawingMode: string;
  color: string;
  strokeWidth: number;
  fontSize: number;
  fontFamily: string;
  canvasObjects: fabric.Object[];
}

// Tool settings state (frequently changed)
export interface ToolSettingsState {
  drawingMode: string;
  color: string;
  strokeWidth: number;
  fontSize: number;
  fontFamily: string;
}

// History state
export interface HistoryState {
  history: string[];
  historyStep: number;
  canUndo: boolean;
  canRedo: boolean;
}

// ============================================
// Advanced Editing Types
// ============================================

// TOC / Bookmark types
export interface TOCItem {
  level: number;
  title: string;
  page: number;
  has_link?: boolean;
  link_type?: string;
}

// Font info types
export interface FontInfo {
  name: string;
  size: number;
  char_count: number;
  percentage: number;
  color?: number[];
}

// Link types
export interface LinkItem {
  index: number;
  type: string;
  uri?: string;
  dest_page?: number;
  rect: number[];
}

// Image metadata types
export interface ImageMetadata {
  index: number;
  xref: number;
  width: number;
  height: number;
  bits_per_component: number;
  color_space: string;
  compression: string;
  format?: string;
  size_bytes?: number;
  aspect_ratio?: number;
  bbox?: number[];
  has_mask?: boolean;
  name?: string;
}

// Combined context type (for backwards compatibility)
export interface EditorContextType extends EditorState, CanvasState, HistoryState {
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
  clearHistory: () => void;
  // Advanced Editing additions
  toc: TOCItem[];
  setToc: (toc: TOCItem[]) => void;
  fonts: FontInfo[];
  setFonts: (fonts: FontInfo[]) => void;
  bookmarks: TOCItem[];
  setBookmarks: (bookmarks: TOCItem[]) => void;
  showTOC: boolean;
  setShowTOC: (show: boolean) => void;
  loadTOC: () => Promise<void>;
  addBookmark: (item: Omit<TOCItem, 'page'>) => Promise<void>;
  deleteBookmark: (index: number) => Promise<void>;
  loadFonts: () => Promise<void>;
}

// Constants
export const MAX_HISTORY_SIZE = 50;
export const DEFAULT_ZOOM_MIN = 0.25;
export const DEFAULT_ZOOM_MAX = 5;
export const DEFAULT_STROKE_WIDTH = 2;
export const DEFAULT_FONT_SIZE = 20;
export const DEFAULT_FONT_FAMILY = 'Arial';
export const DEFAULT_COLOR = '#000000';

// File download constants
export const DEFAULT_DOWNLOAD_FILENAME = 'document.pdf';
