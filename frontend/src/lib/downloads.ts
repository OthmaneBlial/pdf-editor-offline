import { saveBlobWithDesktopDialog } from './desktop';

const normalizeBlob = (data: Blob | BlobPart): Blob => {
  if (data instanceof Blob) {
    return data;
  }

  return new Blob([data]);
};

export const saveBlob = async (data: Blob | BlobPart, filename: string) => {
  const blob = normalizeBlob(data);
  const handledByDesktop = await saveBlobWithDesktopDialog(blob, filename);
  if (handledByDesktop) {
    return;
  }

  const url = window.URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.setAttribute('download', filename);
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.URL.revokeObjectURL(url);
};
