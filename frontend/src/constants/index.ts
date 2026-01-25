/**
 * Application constants
 * Centralized location for all hardcoded values
 */

// API Configuration
export const API_DEFAULTS = {
  BASE_URL: import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8000',
  TIMEOUT: 30000, // 30 seconds
} as const;

// File names for downloads
export const FILENAMES = {
  EXTRACTED_PAGES: 'extracted_pages.pdf',
  MERGED_WITH_INSERT: 'merged_with_insert.pdf',
  DUPLICATED: 'duplicated.pdf',
  DEFAULT_DOWNLOAD: 'document.pdf',
} as const;

// Canvas and Drawing Constants
export const CANVAS_DEFAULTS = {
  INITIAL_LEFT: 100,
  INITIAL_TOP: 100,
  INITIAL_SCALE: 0.5,
  RECTANGLE_SIZE: 100,
  CIRCLE_RADIUS: 50,
} as const;

// Zoom Constants
export const ZOOM_CONSTANTS = {
  MIN: 0.25,
  MAX: 5,
  STEP: 0.25,
  DEFAULT: 1,
  FIT_WIDTH_PADDING: 0.9,
} as const;

// PDF Constants (standard PDF dimensions in points)
export const PDF_CONSTANTS = {
  WIDTH: 612, // 8.5 inches * 72 points/inch
  HEIGHT: 792, // 11 inches * 72 points/inch
} as const;

// History Constants
export const HISTORY_CONSTANTS = {
  MAX_SIZE: 50,
} as const;

// Font Options
export const FONT_FAMILIES = [
  'Arial',
  'Times New Roman',
  'Courier New',
  'Georgia',
  'Verdana',
] as const;

// Font Size Range
export const FONT_SIZE = {
  MIN: 8,
  MAX: 72,
  DEFAULT: 20,
} as const;

// Stroke Width Range
export const STROKE_WIDTH = {
  MIN: 1,
  MAX: 20,
  DEFAULT: 2,
} as const;

// Zoom Presets
export const ZOOM_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3] as const;

// Color Defaults
export const COLORS = {
  DEFAULT: '#000000',
  SUCCESS: '#10B981',
  ERROR: '#EF4444',
  WARNING: '#F59E0B',
} as const;

// Comment Colors
export const COMMENT_COLORS = [
  '#EF4444', // Red
  '#F59E0B', // Amber
  '#10B981', // Emerald
  '#3B82F6', // Blue
  '#8B5CF6', // Violet
  '#EC4899', // Pink
] as const;

// Local Storage Keys
export const STORAGE_KEYS = {
  USERNAME: 'pdf_editor_username',
  RECENT_FILES: 'pdf_editor_recent_files',
} as const;

// View Modes
export const VIEW_MODES = {
  EDITOR: 'editor',
  MANIPULATION: 'manipulation',
  CONVERSION: 'conversion',
  SECURITY: 'security',
  ADVANCED: 'advanced',
} as const;

// Drawing Modes
export const DRAWING_MODES = {
  SELECT: 'select',
  PEN: 'pen',
  RECTANGLE: 'rectangle',
  CIRCLE: 'circle',
  TEXT: 'text',
} as const;
