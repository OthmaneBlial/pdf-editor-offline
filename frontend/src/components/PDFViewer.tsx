import React, { useCallback, useEffect, useRef, useState } from 'react';
import * as fabric from 'fabric';
import axios from 'axios';
import { useEditor } from '../contexts/EditorContext';
import { getApiData } from '../utils/apiResponse';
import { API_BASE_URL } from '../lib/apiClient';

// Constants - Fabric.js types are complex, using any for fabric-specific objects
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const FABRIC_IMAGE_CLASS = (fabric as any).FabricImage || (fabric as any).Image;

// Guard against lifecycle races where Fabric internals are already disposed.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const isCanvasReady = (currentCanvas: any): currentCanvas is fabric.Canvas => {
  return Boolean(
    currentCanvas &&
    typeof currentCanvas.setDimensions === 'function' &&
    typeof currentCanvas.requestRenderAll === 'function' &&
    !currentCanvas.disposed &&
    !currentCanvas.destroyed
  );
};

// Helper to update canvas size
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const updateCanvasSize = (currentCanvas: fabric.Canvas, img: any, container: HTMLElement, currentZoom: number) => {
  if (!isCanvasReady(currentCanvas)) {
    return;
  }

  const imageElement =
    (typeof img.getElement === 'function' ? img.getElement() : null) ??
    img?._originalElement ??
    img?._element ??
    null;

  const imgWidth =
    img.width ||
    imageElement?.naturalWidth ||
    imageElement?.width ||
    0;
  const imgHeight =
    img.height ||
    imageElement?.naturalHeight ||
    imageElement?.height ||
    0;
  const containerWidth = container.clientWidth;

  if (imgWidth > 0 && containerWidth > 0) {
    const fitScale = containerWidth / imgWidth;
    const displayWidth = imgWidth * fitScale * currentZoom;
    const displayHeight = imgHeight * fitScale * currentZoom;

    try {
      currentCanvas.setDimensions({ width: displayWidth, height: displayHeight });
      currentCanvas.setZoom(fitScale * currentZoom);
      currentCanvas.requestRenderAll();
    } catch {
      // Ignore late resize calls after canvas disposal.
    }
  }
};

const PDFViewer: React.FC<{ forceRefresh?: number }> = ({ forceRefresh }) => {
  const {
    document,
    currentPage,
    canvas,
    setCanvas,
    drawingMode,
    setDrawingMode,
    color,
    strokeWidth,
    fontSize,
    fontFamily,
    sessionId,
    setSessionId,
    setPageCount,
    zoom,
    setCurrentPage,
    setIsUploading,
  } = useEditor();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const [pageImage, setPageImage] = useState<string>('');
  const [pageLoading, setPageLoading] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [apiStatus, setApiStatus] = useState<'checking' | 'ready' | 'unreachable'>('checking');
  const currentSessionRef = useRef<string>('');
  const currentPageRef = useRef<number>(0);
  const lastForceRefreshRef = useRef<number>(0);

  // Track last uploaded file to prevent re-upload on remount
  const lastUploadedFileRef = useRef<File | null>(null);

  const checkApiHealth = useCallback(async () => {
    try {
      await axios.get(`${API_BASE_URL}/openapi.json`, { timeout: 4000 });
      setApiStatus('ready');
    } catch {
      setApiStatus('unreachable');
    }
  }, []);

  useEffect(() => {
    checkApiHealth();
  }, [checkApiHealth]);

  // Upload document when file is selected.
  // If a different file is selected, always upload it even if a session exists.
  useEffect(() => {
    if (!document) {
      return;
    }

    // Skip re-upload when the same file/session are already active (e.g. remounts)
    if (lastUploadedFileRef.current === document && sessionId) {
      return;
    }

    const uploadDocument = async () => {
      setIsUploading(true);
      setErrorMessage('');
      // Reset to page 0 for any new upload attempt
      setCurrentPage(0);
      const formData = new FormData();
      formData.append('file', document);

      try {
        const response = await axios.post(`${API_BASE_URL}/api/documents/upload`, formData, {
          headers: { 'Content-Type': 'multipart/form-data' }
        });
        const data = getApiData<{ id?: string; page_count?: number }>(response.data);
        if (!data?.id || typeof data.page_count !== 'number') {
          throw new Error('Unexpected upload response format');
        }

        setSessionId(data.id);
        setPageCount(data.page_count);
        setApiStatus('ready');
        // Mark this file as uploaded
        lastUploadedFileRef.current = document;
      } catch (error) {
        if (axios.isAxiosError(error)) {
          if (!error.response) {
            setApiStatus('unreachable');
            setErrorMessage(`Backend API is not reachable at ${API_BASE_URL}. Start the backend and retry.`);
          } else {
            setErrorMessage(`Upload failed: ${error.response?.data?.detail || error.message}`);
          }
        } else {
          setErrorMessage('Upload failed. Please check the file and try again.');
        }
        setSessionId('');
        setPageImage('');
      } finally {
        setIsUploading(false);
      }
    };
    uploadDocument();
  }, [document, sessionId, setSessionId, setPageCount, setCurrentPage, setIsUploading]);

  // Load page image when session or page changes - FIXED DEPENDENCIES
  useEffect(() => {
    const currentSession = sessionId;
    const currentPg = currentPage;
    const normalizedForceRefresh = forceRefresh ?? 0;

    // Skip if session hasn't changed and page hasn't changed and no force refresh
    if (
      currentSession === currentSessionRef.current &&
      currentPg === currentPageRef.current &&
      normalizedForceRefresh === lastForceRefreshRef.current
    ) {
      return;
    }

    const loadPageImage = async () => {
      setPageLoading(true);
      setErrorMessage('');
      // Clear previous canvas/image so navigation doesn't overlay stale state
      if (canvas) {
        canvas.dispose();
        setCanvas(null);
      }
      setPageImage('');

      try {
        if (!currentSession) return;

        // Add cache busting parameter to prevent stale images
        const cacheBuster = normalizedForceRefresh || Date.now();
        const response = await axios.get(
          `${API_BASE_URL}/api/documents/${currentSession}/pages/${currentPg}?t=${cacheBuster}`
        );
        const data = getApiData<{ image?: string }>(response.data);
        if (!data?.image || typeof data.image !== 'string') {
          throw new Error('Unexpected page response format');
        }

        setPageImage(data.image);
        setApiStatus('ready');
        currentSessionRef.current = currentSession;
        currentPageRef.current = currentPg;
        lastForceRefreshRef.current = normalizedForceRefresh;
      } catch (error) {
        if (axios.isAxiosError(error)) {
          if (!error.response) {
            setApiStatus('unreachable');
            setErrorMessage(`Backend API is not reachable at ${API_BASE_URL}. Start the backend and retry.`);
          } else {
            setErrorMessage(`Unable to load page: ${error.response?.data?.detail || error.message}`);
          }
        } else {
          setErrorMessage('Unable to load page preview. Try again.');
        }
      } finally {
        setPageLoading(false);
      }
    };
    loadPageImage();
  }, [sessionId, currentPage, forceRefresh, canvas, setCanvas]);

  // Keep track of latest zoom for ResizeObserver
  const zoomRef = useRef(zoom);
  useEffect(() => {
    zoomRef.current = zoom;
  }, [zoom]);

  // Handle Zoom Changes
  useEffect(() => {
    if (canvas && containerRef.current && canvas.backgroundImage) {
      updateCanvasSize(canvas, canvas.backgroundImage, containerRef.current, zoom);
    }
  }, [zoom, canvas]);

  // Initialize Fabric.js canvas - FIXED DEPENDENCIES
  useEffect(() => {
    if (!canvasRef.current || !pageImage || !containerRef.current) {
      return;
    }

    let isDisposed = false;

    // Clean up any existing canvas before creating a new one
    if (canvas) {
      canvas.dispose();
      setCanvas(null);
    }

    const fabricCanvas = new fabric.Canvas(canvasRef.current, {
      isDrawingMode: drawingMode === 'pen',
      selection: true,
    });

    // Set drawing brush properties
    if (drawingMode === 'pen') {
      const brush = new fabric.PencilBrush(fabricCanvas);
      brush.color = color;
      brush.width = strokeWidth;
      fabricCanvas.freeDrawingBrush = brush;
    }

    // Add background image and size canvas to the page
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    FABRIC_IMAGE_CLASS.fromURL(pageImage).then((img: any) => {
      if (isDisposed || !isCanvasReady(fabricCanvas)) {
        return;
      }

      img.set({
        selectable: false,
        evented: false,
        originX: 'left',
        originY: 'top',
      });

      // Initial sizing
      const container = containerRef.current;
      if (container) {
        updateCanvasSize(fabricCanvas, img, container, zoomRef.current);
      }

      fabricCanvas.backgroundImage = img;
      fabricCanvas.requestRenderAll();

      // Create and store ResizeObserver for proper cleanup
      const resizeObserver = new ResizeObserver(() => {
        if (isDisposed || !isCanvasReady(fabricCanvas)) {
          return;
        }

        const currentContainer = containerRef.current;
        if (currentContainer) {
          requestAnimationFrame(() => {
            if (isDisposed || !isCanvasReady(fabricCanvas)) {
              return;
            }
            updateCanvasSize(fabricCanvas, img, currentContainer, zoomRef.current);
          });
        }
      });

      if (container) {
        resizeObserver.observe(container);
      }

      // Store observer ref for cleanup
      resizeObserverRef.current = resizeObserver;

    }).catch(() => {
      if (!isDisposed) {
        setErrorMessage('Failed to load page image. Please try again.');
      }
    });

    setCanvas(fabricCanvas);

    // Proper cleanup function
    return () => {
      isDisposed = true;
      // Disconnect ResizeObserver first
      if (resizeObserverRef.current) {
        resizeObserverRef.current.disconnect();
        resizeObserverRef.current = null;
      }
      // Remove all event listeners
      fabricCanvas.off();
      // Dispose canvas
      fabricCanvas.dispose();
      // Clear the canvas reference from context
      setCanvas(null);
    };
  }, [pageImage, setCanvas]); // eslint-disable-line react-hooks/exhaustive-deps -- Only recreate canvas when page image changes

  // Handle drawing mode and properties changes
  useEffect(() => {
    if (canvas) {
      canvas.isDrawingMode = drawingMode === 'pen';
      if (drawingMode === 'pen') {
        if (!canvas.freeDrawingBrush) {
          canvas.freeDrawingBrush = new fabric.PencilBrush(canvas);
        }
        canvas.freeDrawingBrush.color = color;
        canvas.freeDrawingBrush.width = strokeWidth;
      }
    }
  }, [canvas, drawingMode, color, strokeWidth]);

  // Update active object when font properties change
  useEffect(() => {
    if (canvas) {
      const activeObject = canvas.getActiveObject();
      if (activeObject && (activeObject.type === 'i-text' || activeObject.type === 'text' || activeObject.type === 'textbox')) {
        activeObject.set({
          fontSize: fontSize,
          fontFamily: fontFamily,
          fill: color
        });
        canvas.requestRenderAll();
      }
    }
  }, [canvas, fontSize, fontFamily, color]);

  // Handle canvas interactions (Tools)
  useEffect(() => {
    if (canvas) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handleMouseDown = (opt: any) => {
        const evt = opt.e;
        if (opt.target) {
          return;
        }

        const pointer = canvas.getPointer(evt);

        if (drawingMode === 'text') {
          const text = new fabric.IText('Enter text', {
            left: pointer.x,
            top: pointer.y,
            fill: color,
            fontSize: fontSize,
            fontFamily: fontFamily,
            selectable: true,
            hasControls: true,
          });
          canvas.add(text);
          canvas.setActiveObject(text);
          text.enterEditing();
          text.selectAll();
          setDrawingMode('select');
        } else if (drawingMode === 'rectangle') {
          const rect = new fabric.Rect({
            left: pointer.x,
            top: pointer.y,
            width: 100,
            height: 100,
            fill: 'transparent',
            stroke: color,
            strokeWidth: strokeWidth,
            selectable: true,
            hasControls: true,
          });
          canvas.add(rect);
          canvas.setActiveObject(rect);
          setDrawingMode('select');
        } else if (drawingMode === 'circle') {
          const circle = new fabric.Circle({
            left: pointer.x,
            top: pointer.y,
            radius: 50,
            fill: 'transparent',
            stroke: color,
            strokeWidth: strokeWidth,
            selectable: true,
            hasControls: true,
          });
          canvas.add(circle);
          canvas.setActiveObject(circle);
          setDrawingMode('select');
        }
      };

      canvas.on('mouse:down', handleMouseDown);

      return () => {
        canvas.off('mouse:down', handleMouseDown);
      };
    }
  }, [canvas, drawingMode, color, strokeWidth, fontSize, fontFamily, setDrawingMode]);

  // Handle canvas object creation (general)
  useEffect(() => {
    if (canvas) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handleObjectAdded = (e: any) => {
        const obj = e.target;
        if (obj) {
          obj.set({
            selectable: true,
            hasControls: true,
            hasBorders: true,
            lockScalingFlip: true,
          });
        }
      };

      canvas.on('object:added', handleObjectAdded);

      return () => {
        canvas.off('object:added', handleObjectAdded);
      };
    }
  }, [canvas]);

  return (
    <div ref={containerRef} className="pdf-viewer relative w-full h-full overflow-hidden">
      {apiStatus === 'unreachable' && !pageImage && !pageLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-white/90 z-30 p-4">
          <div className="max-w-md w-full rounded-xl border border-amber-200 bg-amber-50 text-amber-900 p-4 shadow-sm" role="alert">
            <p className="font-semibold text-sm mb-2">Backend API is not reachable.</p>
            <p className="text-xs leading-relaxed mb-3">
              Check that the API is running at <code>{API_BASE_URL}</code>. This commonly appears as a CORS/network error when the backend is down.
            </p>
            <button
              type="button"
              onClick={checkApiHealth}
              className="px-3 py-2 text-xs font-medium rounded-lg bg-amber-600 text-white hover:bg-amber-700 transition-colors"
            >
              Retry connection
            </button>
          </div>
        </div>
      )}

      {pageLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-white/80 z-20" role="status" aria-live="polite">
          <div className="px-4 py-2 rounded-lg bg-white shadow border border-sky-100 text-sky-700 text-sm font-medium">
            Loading page…
          </div>
        </div>
      )}
      {errorMessage && (
        <div className="absolute top-4 right-4 z-30 px-4 py-3 bg-rose-50 text-rose-700 border border-rose-200 rounded-xl shadow" role="alert">
          {errorMessage}
        </div>
      )}
      {pageImage && (
        <canvas
          ref={canvasRef}
          style={{
            border: '1px solid #e5e7eb',
            display: 'block',
          }}
        />
      )}
      {!pageImage && !pageLoading && !errorMessage && (
        <div className="absolute inset-0 flex items-center justify-center text-slate-400 text-sm">
          Upload a PDF to get started.
        </div>
      )}
    </div>
  );
};

export default PDFViewer;
