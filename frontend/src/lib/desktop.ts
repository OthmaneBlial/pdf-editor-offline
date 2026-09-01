type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
type UnlistenFn = () => void;

export interface DesktopFilePayload {
  name: string;
  size: number;
  bytes: number[];
}

export interface DesktopRecentFile {
  name: string;
  size: number;
  path?: string;
  lastOpened: string;
}

interface DesktopAPIConnection {
  baseUrl: string;
  token: string;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __PDF_EDITOR_OFFLINE_API_BASE_URL__?: string;
    __PDF_EDITOR_OFFLINE_API_TOKEN__?: string;
  }
}

export const isDesktopRuntime = () =>
  typeof window !== 'undefined' &&
  Boolean(window.__TAURI_INTERNALS__ || window.location.protocol === 'tauri:');

const getInvoke = async (): Promise<InvokeFn | null> => {
  if (!isDesktopRuntime()) {
    return null;
  }

  const api = await import('@tauri-apps/api/core');
  return api.invoke as InvokeFn;
};

const payloadToPdfFile = (payload: DesktopFilePayload): File => {
  const bytes = new Uint8Array(payload.bytes);
  return new File([bytes], payload.name, { type: 'application/pdf' });
};

export const initializeDesktopRuntime = async () => {
  const invoke = await getInvoke();
  if (!invoke) {
    return;
  }

  const connection = await invoke<DesktopAPIConnection>('get_api_connection');
  window.__PDF_EDITOR_OFFLINE_API_BASE_URL__ = connection.baseUrl;
  window.__PDF_EDITOR_OFFLINE_API_TOKEN__ = connection.token;
};

export const openPdfWithDesktopDialog = async (): Promise<File | null> => {
  const invoke = await getInvoke();
  if (!invoke) {
    return null;
  }

  const payload = await invoke<DesktopFilePayload | null>('open_pdf_file');
  if (!payload) {
    return null;
  }

  return payloadToPdfFile(payload);
};

export const openPdfFromDesktopPath = async (path: string): Promise<File | null> => {
  const invoke = await getInvoke();
  if (!invoke) {
    return null;
  }

  const payload = await invoke<DesktopFilePayload>('open_pdf_path', { path });
  return payloadToPdfFile(payload);
};

export const listenForDesktopPdfDrops = async (
  onFile: (file: File) => void,
  onDragStateChange: (active: boolean) => void = () => undefined,
): Promise<UnlistenFn | null> => {
  if (!isDesktopRuntime()) {
    return null;
  }

  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  return getCurrentWebview().onDragDropEvent(async event => {
    if (event.payload.type === 'over') {
      onDragStateChange(true);
      return;
    }

    onDragStateChange(false);
    if (event.payload.type !== 'drop') {
      return;
    }

    const pdfPath = event.payload.paths.find(path => path.toLowerCase().endsWith('.pdf'));
    if (!pdfPath) {
      return;
    }

    const file = await openPdfFromDesktopPath(pdfPath);
    if (file) {
      onFile(file);
    }
  });
};

export const saveBlobWithDesktopDialog = async (
  blob: Blob,
  defaultFilename: string,
): Promise<boolean> => {
  const invoke = await getInvoke();
  if (!invoke) {
    return false;
  }

  const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
  await invoke('save_file', {
    defaultFilename,
    bytes,
  });
  return true;
};

export const getDesktopRecentFiles = async (): Promise<DesktopRecentFile[] | null> => {
  const invoke = await getInvoke();
  if (!invoke) {
    return null;
  }

  return invoke<DesktopRecentFile[]>('recent_files_get');
};

export const addDesktopRecentFile = async (file: File) => {
  const invoke = await getInvoke();
  if (!invoke) {
    return false;
  }

  await invoke('recent_files_add', {
    file: {
      name: file.name,
      size: file.size,
      lastOpened: new Date().toISOString(),
    },
  });
  return true;
};

export const removeDesktopRecentFile = async (fileName: string) => {
  const invoke = await getInvoke();
  if (!invoke) {
    return false;
  }

  await invoke('recent_files_remove', { fileName });
  return true;
};

export const clearDesktopRecentFiles = async () => {
  const invoke = await getInvoke();
  if (!invoke) {
    return false;
  }

  await invoke('recent_files_clear');
  return true;
};
