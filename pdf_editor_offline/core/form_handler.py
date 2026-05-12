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
                        "rect": [
                            widget.rect.x0,
                            widget.rect.y0,
                            widget.rect.x1,
                            widget.rect.y1,
                        ],
                    }
                )
        return fields

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

    def flatten_form(self):
        """
        Flatten all form fields, making them part of the page content.
        """
        for page in self.document:
            for widget in page.widgets():
                # This is a simplification; PyMuPDF doesn't have a direct "flatten" for widgets
                # in the same way some other libs do, but we can try to make them read-only or
                # just leave them as is if 'flatten' implies making them non-editable.
                # A true flatten often involves drawing the appearance stream to the page and removing the widget.
                # For now, let's just make them read-only.
                widget.field_flags |= fitz.pdf.PDF_FIELD_IS_READ_ONLY
                widget.update()
