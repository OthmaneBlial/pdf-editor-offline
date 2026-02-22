# PDF Editor Offline Examples

This directory contains example scripts demonstrating various usage patterns for PDF Editor Offline.

## Scripts

### `batch_process_pdfs.py`

A comprehensive batch processing script that demonstrates how to:

- Process multiple PDF files in a directory
- Extract text from all PDFs
- Extract images from all PDFs
- Generate a summary report

**Usage:**
```bash
python batch_process_pdfs.py <input_directory> <output_directory>
```

**Example:**
```bash
python batch_process_pdfs.py ./pdfs ./processed
```

This will:
- Extract text from all PDFs in `./pdfs` to `./processed`
- Extract images to subdirectories in `./processed`
- Generate a `batch_report.txt` with processing summary

## Requirements

All examples require PDF Editor Offline to be installed:

```bash
pip install pdf-editor-offline
```

Or install from source:

```bash
pip install -e .
```

## Notes

- Scripts include error handling and informative output
- Examples are CLI-first and compatible with headless automation environments
- Examples can be modified for specific use cases
