class PDFLoadError(Exception):
    """Raised when a PDF cannot be loaded."""

    pass


class PDFSaveError(Exception):
    """Raised when a PDF cannot be saved."""

    pass


class InvalidOperationError(Exception):
    """Raised when an invalid operation is attempted."""

    pass


class MissingDependencyError(RuntimeError):
    """Raised when an optional local executable is required for an operation."""

    def __init__(self, command: str, friendly_name: str):
        self.command = command
        self.friendly_name = friendly_name
        super().__init__(
            f"{friendly_name} is required for this operation but "
            f"'{command}' was not found on PATH."
        )
