# PDF Editor Offline Architecture

PDF Editor Offline is a professional PDF editing platform with a React frontend and a FastAPI backend. It leverages PyMuPDF for core operations and includes advanced tools for conversion, manipulation, and OCR.

## Architecture

- **Frontend**: React 18, TypeScript, Tailwind CSS, Fabric.js.
- **Backend**: FastAPI, PyMuPDF, pdf2docx, python-pptx, etc.
- **Modular Design**: Separated routes for documents and tools, atomic processing, and robust session management.

## Components

1. **DocumentManager**: Core lifecycle management (load/save).
2. **Editor**: Surface-level modifications (text/images/redaction).
3. **Tools**: Advanced processing (Merge/Split/Convert/OCR).
4. **Metadata**: Comprehensive property management.

## Roadmap

- Advanced OCR integration.
- Collaboration features.
- Performance optimizations for cloud environments.

For more details, refer to the code in `pdfsmarteditor/core`.