#!/usr/bin/env python3
"""
Example script demonstrating batch processing of PDF files using PDF Editor Offline.

This script shows how to:
- Process multiple PDF files in a directory
- Extract text from all PDFs
- Generate a summary report
- Handle errors gracefully
"""

import os
import subprocess
import sys
from pathlib import Path


def run_command(cmd):
    """Run a command and return success status."""
    try:
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        return result.returncode == 0, result.stdout, result.stderr
    except Exception as e:
        return False, "", str(e)


def batch_extract_text(input_dir, output_dir):
    """Extract text from all PDF files in input_dir to output_dir."""
    input_path = Path(input_dir)
    output_path = Path(output_dir)
    output_path.mkdir(exist_ok=True)

    pdf_files = list(input_path.glob("*.pdf"))
    if not pdf_files:
        print(f"No PDF files found in {input_dir}")
        return

    print(f"Processing {len(pdf_files)} PDF files...")

    success_count = 0
    for pdf_file in pdf_files:
        print(f"Extracting text from {pdf_file.name}...")

        output_file = output_path / f"{pdf_file.stem}_text.txt"
        cmd = f"pdf-editor-offline extract text '{pdf_file}' > '{output_file}'"

        success, stdout, stderr = run_command(cmd)
        if success:
            print(f"  ✓ Extracted to {output_file}")
            success_count += 1
        else:
            print(f"  ✗ Failed: {stderr}")

    print(
        f"\nBatch processing complete: {success_count}/{len(pdf_files)} files processed successfully"
    )


def batch_extract_images(input_dir, output_dir):
    """Extract images from all PDF files in input_dir to output_dir."""
    input_path = Path(input_dir)
    output_path = Path(output_dir)
    output_path.mkdir(exist_ok=True)

    pdf_files = list(input_path.glob("*.pdf"))
    if not pdf_files:
        print(f"No PDF files found in {input_dir}")
        return

    print(f"Extracting images from {len(pdf_files)} PDF files...")

    for pdf_file in pdf_files:
        print(f"Extracting images from {pdf_file.name}...")

        image_dir = output_path / pdf_file.stem
        cmd = f"pdf-editor-offline extract images '{pdf_file}' --output-dir '{image_dir}'"

        success, stdout, stderr = run_command(cmd)
        if success:
            print(f"  ✓ Images extracted to {image_dir}")
        else:
            print(f"  ✗ Failed: {stderr}")


def generate_report(input_dir, output_dir):
    """Generate a summary report of processed files."""
    input_path = Path(input_dir)
    output_path = Path(output_dir)

    pdf_files = list(input_path.glob("*.pdf"))
    text_files = list(output_path.glob("*_text.txt"))

    report = f"""PDF Batch Processing Report
{'='*30}

Input directory: {input_dir}
Output directory: {output_dir}

Total PDF files: {len(pdf_files)}
Text extraction files: {len(text_files)}

PDF Files:
"""

    for pdf in sorted(pdf_files):
        size = pdf.stat().st_size / 1024  # KB
        report += f"- {pdf.name} ({size:.1f} KB)\n"

    report += "\nText Files Generated:\n"
    for txt in sorted(text_files):
        size = txt.stat().st_size
        report += f"- {txt.name} ({size} bytes)\n"

    report_file = output_path / "batch_report.txt"
    report_file.write_text(report)
    print(f"Report generated: {report_file}")


def main():
    if len(sys.argv) != 3:
        print("Usage: python batch_process_pdfs.py <input_dir> <output_dir>")
        print("Example: python batch_process_pdfs.py ./pdfs ./output")
        sys.exit(1)

    input_dir = sys.argv[1]
    output_dir = sys.argv[2]

    if not os.path.isdir(input_dir):
        print(f"Error: Input directory '{input_dir}' does not exist")
        sys.exit(1)

    # Create output directory
    os.makedirs(output_dir, exist_ok=True)

    print("Starting batch PDF processing...")
    print(f"Input: {input_dir}")
    print(f"Output: {output_dir}")
    print()

    # Extract text
    batch_extract_text(input_dir, output_dir)
    print()

    # Extract images
    batch_extract_images(input_dir, output_dir)
    print()

    # Generate report
    generate_report(input_dir, output_dir)

    print("\nBatch processing completed!")


if __name__ == "__main__":
    main()
