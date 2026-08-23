import fitz

from .exceptions import InvalidOperationError


class FormHandler:
    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def list_form_fields(self):
        """
        List all form fields in the document.
        Returns a list of dictionaries with field info.
        """
        fields = []
        for page_num, page in enumerate(self.document):
            for widget in page.widgets():
                fields.append(
                    {
                        "page": page_num,
                        "name": widget.field_name,
                        "value": widget.field_value,
                        "type": widget.field_type_string,
                        "flags": widget.field_flags,
                        "read_only": bool(
                            widget.field_flags & fitz.PDF_FIELD_IS_READ_ONLY
                        ),
                        "rect": [
                            widget.rect.x0,
                            widget.rect.y0,
                            widget.rect.x1,
                            widget.rect.y1,
                        ],
                    }
                )
        return fields

    def has_xfa(self) -> bool:
        """Return whether the PDF declares an XFA form packet."""
        try:
            catalog = self.document.pdf_catalog()
            kind, value = self.document.xref_get_key(catalog, "AcroForm")
            if kind != "xref":
                return False
            form_xref = int(value.split()[0])
            xfa_kind, _ = self.document.xref_get_key(form_xref, "XFA")
            return xfa_kind != "null"
        except (AttributeError, TypeError, ValueError, RuntimeError):
            return False

    def fill_form_field(self, field_name: str, value: str):
        """
        Fill a form field with a given value.
        """
        found = False
        for page in self.document:
            for widget in page.widgets():
                if widget.field_name == field_name:
                    widget.field_value = value
                    widget.update()
                    found = True

        if not found:
            raise InvalidOperationError(f"Field '{field_name}' not found")

    def flatten_form(self) -> int:
        """
        Flatten all form fields, making them part of the page content.
        """
        flattened = 0
        for page in self.document:
            widgets = list(page.widgets() or [])
            for widget in widgets:
                widget.update()
                rect = fitz.Rect(widget.rect)
                pixmap = page.get_pixmap(
                    matrix=fitz.Matrix(2, 2),
                    clip=rect,
                    annots=True,
                    alpha=False,
                )
                page.delete_widget(widget)
                page.insert_image(rect, pixmap=pixmap, overlay=True)
                flattened += 1
        return flattened
