import axios from 'axios';

declare global {
  interface Window {
    __PDF_EDITOR_OFFLINE_API_BASE_URL__?: string;
  }
}

export const API_BASE_URL =
  window.__PDF_EDITOR_OFFLINE_API_BASE_URL__ ??
  import.meta.env.VITE_API_BASE_URL ??
  'http://localhost:8000';

export const apiClient = axios.create({
  baseURL: API_BASE_URL,
});
