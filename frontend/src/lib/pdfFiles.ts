export const isPdfFile = (file: File): boolean => {
  const normalizedType = file.type.trim().toLowerCase();
  return normalizedType === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf');
};
