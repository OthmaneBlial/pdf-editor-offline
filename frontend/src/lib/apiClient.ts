import axios from 'axios';

declare global {
  interface Window {
    __PDF_EDITOR_OFFLINE_API_BASE_URL__?: string;
    __PDF_EDITOR_OFFLINE_API_TOKEN__?: string;
  }
}

export const API_BASE_URL =
  window.__PDF_EDITOR_OFFLINE_API_BASE_URL__ ??
  import.meta.env.VITE_API_BASE_URL ??
  'http://localhost:8000';

export const API_TOKEN =
  window.__PDF_EDITOR_OFFLINE_API_TOKEN__ ??
  import.meta.env.VITE_API_TOKEN ??
  '';

if (API_TOKEN) {
  axios.defaults.headers.common['X-PDF-Editor-Token'] = API_TOKEN;
}

axios.interceptors?.response?.use(response => {
  const warning = response.headers?.['x-pdf-accessibility-warning'];
  if (warning && typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('pdf-accessibility-warning', { detail: warning }));
  }
  return response;
});

export const apiClient = axios.create({
  baseURL: API_BASE_URL,
  headers: API_TOKEN ? { 'X-PDF-Editor-Token': API_TOKEN } : undefined,
});
