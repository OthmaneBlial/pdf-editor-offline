class PDFLoadError(Exception):
    """Raised when a PDF cannot be loaded."""

    pass


class PDFSaveError(Exception):
    """Raised when a PDF cannot be saved."""

    pass


class InvalidOperationError(Exception):
    """Raised when an invalid operation is attempted."""

    pass
