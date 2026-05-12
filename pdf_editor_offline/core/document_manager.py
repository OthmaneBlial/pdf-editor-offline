import fitz

from ..utils.pdf_helpers import check_pdf_compatibility
from .exceptions import InvalidOperationError, PDFLoadError, PDFSaveError


class DocumentManager:
    def __init__(self):
        self.document = None

    def load_pdf(self, path: str):
        try:
            self.document = fitz.open(path)
        except Exception as e:
            raise PDFLoadError(f"Failed to load PDF from {path}: {e}")

    def save_pdf(self, path: str):
        if self.document is None:
            raise InvalidOperationError("No document is currently loaded.")
        try:
            self.document.save(path)
        except Exception as e:
            raise PDFSaveError(f"Failed to save PDF to {path}: {e}")

    def get_document(self):
        return self.document

    def close_document(self):
        if self.document:
            self.document.close()
            self.document = None

    def check_compatibility(self, path: str) -> bool:
        """
        Check if PDF is compatible (version 1.4-2.0).
        """
        return check_pdf_compatibility(path)
