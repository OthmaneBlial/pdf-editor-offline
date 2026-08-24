import pymupdf as fitz

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
        radio_fields = {}
        for page_num, page in enumerate(self.document):
            for widget in page.widgets() or ():
                field_type = self._field_kind(widget)
                choices = list(getattr(widget, "choice_values", None) or [])
                button_values = self._button_values(widget)
                field = {
                        "id": f"{page_num}:{widget.xref}",
                        "page": page_num,
                        "name": widget.field_name,
                        "label": widget.field_label or widget.field_name,
                        "value": widget.field_value,
                        "type": widget.field_type_string,
                        "field_type": field_type,
                        "choices": choices,
                        "button_values": button_values,
                        "flags": widget.field_flags,
                        "read_only": bool(
                            widget.field_flags & fitz.PDF_FIELD_IS_READ_ONLY
                        ),
                        "required": bool(
                            widget.field_flags & fitz.PDF_FIELD_IS_REQUIRED
                        ),
                        "rect": [
                            widget.rect.x0,
                            widget.rect.y0,
                            widget.rect.x1,
                            widget.rect.y1,
                        ],
                    }
                if field_type == "radio" and widget.field_name in radio_fields:
                    existing = radio_fields[widget.field_name]
                    existing["choices"] = list(
                        dict.fromkeys([*existing["choices"], *button_values])
                    )
                    existing["button_values"] = existing["choices"]
                    if widget.field_value not in {None, "", "Off"}:
                        existing["value"] = widget.field_value
                    continue
                if field_type == "radio":
                    field["choices"] = button_values
                    radio_fields[widget.field_name] = field
                fields.append(field)
        fields.sort(key=lambda field: (field["page"], field["rect"][1], field["rect"][0]))
        for tab_index, field in enumerate(fields, start=1):
            field["tab_index"] = tab_index
        return fields

    @staticmethod
    def _button_values(widget) -> list[str]:
        try:
            states = widget.button_states() or {}
        except (AttributeError, RuntimeError, TypeError):
            return []
        values = []
        for state_values in states.values():
            for value in state_values or []:
                if value and value != "Off" and value not in values:
                    values.append(value)
        return values

    def _field_kind(self, widget) -> str:
        raw_type = (widget.field_type_string or "").lower()
        if "check" in raw_type:
            return "checkbox"
        if "radio" in raw_type:
            return "radio"
        if "combo" in raw_type:
            return "dropdown"
        if "list" in raw_type:
            return "listbox"
        if "signature" in raw_type:
            return "signature"
        if "text" not in raw_type:
            return "unknown"
        field_hint = f"{widget.field_name or ''} {widget.field_label or ''}".lower()
        try:
            definition = self.document.xref_object(widget.xref)
        except RuntimeError:
            definition = ""
        return "date" if "date" in field_hint or "AFDate_" in definition else "text"

    def inspect_risks(self) -> dict:
        """Return content-free counts for unsupported or invalidating structures."""
        javascript_actions = 0
        calculation_actions = 0
        for xref in range(1, self.document.xref_length()):
            try:
                source = self.document.xref_object(xref)
            except RuntimeError:
                continue
            javascript_actions += "/JavaScript" in source or "/JS" in source
            calculation_actions += any(
                marker in source for marker in ("/CO", "/Calculate", "AFSimple_Calculate")
            )
        signature_fields = sum(
            widget.field_type == fitz.PDF_WIDGET_TYPE_SIGNATURE
            for page in self.document
            for widget in page.widgets() or ()
        )
        return {
            "has_xfa": self.has_xfa(),
            "javascript_actions": javascript_actions,
            "calculation_actions": calculation_actions,
            "signature_fields": signature_fields,
        }

    def has_xfa(self) -> bool:
        """Return whether the PDF declares an XFA form packet."""
        try:
            catalog = self.document.pdf_catalog()
            kind, value = self.document.xref_get_key(catalog, "AcroForm")
            if kind == "dict":
                return "/XFA" in value
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
        matches = [
            (page, widget)
            for page in self.document
            for widget in page.widgets() or ()
            if widget.field_name == field_name
        ]
        if matches and all(self._field_kind(widget) == "radio" for _, widget in matches):
            if any(widget.field_flags & fitz.PDF_FIELD_IS_READ_ONLY for _, widget in matches):
                raise InvalidOperationError(f"Field '{field_name}' is read-only")
            target = next(
                (
                    widget
                    for _, widget in matches
                    if str(widget.on_state()) == value
                ),
                None,
            )
            if target is None and len(matches) == 1 and value.strip().lower() in {
                "1",
                "true",
                "yes",
                "on",
            }:
                target = matches[0][1]
            if target is None:
                raise InvalidOperationError(
                    f"Field '{field_name}' requires one of its declared radio choices"
                )
            target.field_value = target.on_state()
            target.update()
            return

        found = False
        for page in self.document:
            for widget in page.widgets() or ():
                if widget.field_name == field_name:
                    if widget.field_flags & fitz.PDF_FIELD_IS_READ_ONLY:
                        raise InvalidOperationError(f"Field '{field_name}' is read-only")
                    field_type = self._field_kind(widget)
                    choices = list(getattr(widget, "choice_values", None) or [])
                    if choices and value not in choices:
                        raise InvalidOperationError(
                            f"Field '{field_name}' requires one of its declared choices"
                        )
                    if field_type == "checkbox":
                        truthy = value.strip().lower() in {"1", "true", "yes", "on"}
                        widget.field_value = widget.on_state() if truthy else "Off"
                    else:
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
