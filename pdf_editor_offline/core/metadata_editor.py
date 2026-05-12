import fitz

from .exceptions import InvalidOperationError


class MetadataEditor:
    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def read_metadata(self):
        return self.document.metadata

    def write_metadata(self, metadata_dict: dict):
        try:
            self.document.set_metadata(metadata_dict)
        except Exception as e:
            raise InvalidOperationError(f"Failed to set metadata: {e}")

    def update_metadata(self, key: str, value):
        current = self.read_metadata()
        current[key] = value
        self.write_metadata(current)

    def remove_metadata(self, key: str):
        """Remove a specific metadata key."""
        current = self.read_metadata()
        if key in current:
            current[key] = None  # Or ""
            self.write_metadata(current)

    def clear_all_metadata(self):
        """Clear all metadata fields."""
        self.write_metadata({})
