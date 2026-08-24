import pymupdf as fitz

from .exceptions import InvalidOperationError


class PDFPrivacyCleaner:
    """Privacy-focused cleanup for metadata and hidden PDF data."""

    def __init__(self, document=None):
        self.document = document

    def _get_document(self, document=None):
        doc = document or self.document
        if doc is None:
            raise InvalidOperationError("Document is None")
        return doc

    def clear_metadata(self, document=None) -> dict:
        doc = self._get_document(document)
        metadata_before = doc.metadata or {}
        cleared_fields = [
            key
            for key, value in metadata_before.items()
            if key not in {"format", "encryption"} and value
        ]
        xml_metadata_removed = bool(doc.get_xml_metadata())

        doc.set_metadata({})
        try:
            doc.del_xml_metadata()
        except Exception:
            doc.set_xml_metadata("")

        return {
            "metadata_fields_cleared": len(cleared_fields),
            "xml_metadata_removed": xml_metadata_removed,
        }

    def cleanup_hidden_data(
        self,
        document=None,
        *,
        remove_metadata: bool = True,
        remove_embedded_files: bool = True,
        remove_hidden_text: bool = True,
        remove_javascript: bool = True,
        remove_links: bool = False,
        remove_annotations: bool = False,
        remove_thumbnails: bool = True,
        reset_form_fields: bool = False,
        apply_redactions: bool = True,
        clean_pages: bool = True,
    ) -> dict:
        doc = self._get_document(document)

        metadata_stats = {
            "metadata_fields_cleared": 0,
            "xml_metadata_removed": False,
        }
        if remove_metadata:
            metadata_stats = self.clear_metadata(doc)

        annotations_removed = 0
        if remove_annotations:
            for page in doc:
                for annot in list(page.annots() or []):
                    page.delete_annot(annot)
                    annotations_removed += 1

        links_removed = 0
        if remove_links:
            links_removed = sum(len(page.get_links()) for page in doc)

        embedded_files_removed = doc.embfile_count() if remove_embedded_files else 0

        doc.scrub(
            attached_files=remove_embedded_files,
            clean_pages=clean_pages,
            embedded_files=remove_embedded_files,
            hidden_text=remove_hidden_text,
            javascript=remove_javascript,
            metadata=False,
            redactions=apply_redactions,
            redact_images=fitz.PDF_REDACT_IMAGE_PIXELS,
            remove_links=remove_links,
            reset_fields=reset_form_fields,
            reset_responses=reset_form_fields,
            thumbnails=remove_thumbnails,
            xml_metadata=False,
        )

        # PyMuPDF clears JavaScript source but can leave an inert OpenAction
        # reference in the catalog. Remove JavaScript launch points entirely so
        # independent inspection does not report an active document action.
        if remove_javascript:
            catalog_xref = doc.pdf_catalog()
            action_type, action_value = doc.xref_get_key(
                catalog_xref, "OpenAction"
            )
            if action_type == "xref":
                try:
                    action_xref = int(action_value.split()[0])
                    if "/JavaScript" in doc.xref_object(action_xref):
                        doc.xref_set_key(catalog_xref, "OpenAction", "null")
                except (TypeError, ValueError, RuntimeError):
                    pass
            doc.xref_set_key(catalog_xref, "AA", "null")

        return {
            **metadata_stats,
            "annotations_removed": annotations_removed,
            "embedded_files_removed": embedded_files_removed,
            "links_removed": links_removed,
            "pages_cleaned": len(doc) if clean_pages else 0,
            "hidden_text_removed": remove_hidden_text,
            "javascript_removed": remove_javascript,
            "thumbnails_removed": remove_thumbnails,
            "form_fields_reset": reset_form_fields,
            "redactions_applied": apply_redactions,
        }

    def clean_metadata_file(self, input_path: str, output_path: str) -> dict:
        doc = fitz.open(input_path)
        try:
            stats = self.clear_metadata(doc)
            doc.save(
                output_path,
                garbage=4,
                clean=True,
                deflate=True,
                preserve_metadata=False,
            )
            return stats
        finally:
            doc.close()

    def cleanup_hidden_data_file(
        self, input_path: str, output_path: str, **options
    ) -> dict:
        doc = fitz.open(input_path)
        try:
            stats = self.cleanup_hidden_data(doc, **options)
            doc.save(
                output_path,
                garbage=4,
                clean=True,
                deflate=True,
                preserve_metadata=False,
            )
            return stats
        finally:
            doc.close()
