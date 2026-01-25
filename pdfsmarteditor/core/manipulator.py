import os
import shutil
from typing import List, Optional

import fitz


def _require_dependency(command: str, friendly_name: str):
    """Ensure an external binary exists before running a tool."""
    if not shutil.which(command):
        raise RuntimeError(
            f"{friendly_name} is required for this operation but '{command}' was not found on PATH."
        )


class PDFManipulator:
    def __init__(self):
        pass

    def merge_pdfs(self, file_paths: List[str], output_path: str):
        """
        Merge multiple PDFs into one.
        """
        merged_doc = fitz.open()

        for path in file_paths:
            try:
                doc = fitz.open(path)
                merged_doc.insert_pdf(doc)
                doc.close()
            except Exception as e:
                print(f"Error merging file {path}: {str(e)}")
                # Continue with other files or raise?
                # For now, let's raise to be safe
                raise e

        merged_doc.save(output_path)
        merged_doc.close()

    def split_pdf(
        self, file_path: str, page_ranges: List[str], output_dir: str
    ) -> List[str]:
        """
        Split PDF based on page ranges.
        page_ranges: List of strings like "1-3", "5", "7-9" (1-based indexing)
        Returns list of paths to created files.
        """
        doc = fitz.open(file_path)
        output_files = []

        for i, range_str in enumerate(page_ranges):
            new_doc = fitz.open()

            # Parse range
            if "-" in range_str:
                start, end = map(int, range_str.split("-"))
            else:
                start = int(range_str)
                end = start

            # Convert to 0-based indexing
            start -= 1
            end -= 1  # Inclusive in request, but fitz needs careful handling

            # Validate
            if start < 0 or end >= len(doc):
                continue  # Or raise error

            new_doc.insert_pdf(doc, from_page=start, to_page=end)

            output_filename = f"split_{i+1}_{os.path.basename(file_path)}"
            output_path = os.path.join(output_dir, output_filename)
            new_doc.save(output_path)
            new_doc.close()
            output_files.append(output_path)

        doc.close()
        return output_files

    def compress_pdf(self, file_path: str, output_path: str, level: int = 4):
        """
        Compress PDF.
        level: 0-4 (4 is max compression)
        """
        doc = fitz.open(file_path)
        # deflate=True compresses streams
        # garbage=4 removes unused objects and compacts xref
        doc.save(output_path, garbage=4, deflate=True)
        doc.close()

    def add_signature(
        self,
        file_path: str,
        signature_path: str,
        output_path: str,
        page_num: int,
        x: int,
        y: int,
        width: int,
        height: int,
    ):
        """
        Add a signature image to a PDF page.
        """
        doc = fitz.open(file_path)
        page = doc[page_num]

        rect = fitz.Rect(x, y, x + width, y + height)
        page.insert_image(rect, filename=signature_path)

        doc.save(output_path)
        doc.close()

    def add_watermark(
        self,
        file_path: str,
        text: str,
        output_path: str,
        opacity: float = 0.3,
        rotation: int = 0,
        font_size: int = 50,
        color: tuple = (0, 0, 0),
    ):
        """
        Add text watermark to all pages of a PDF.
        Note: For watermarks, rotation must be a multiple of 90 for PyMuPDF.
        """
        import math

        doc = fitz.open(file_path)

        for page in doc:
            # Calculate center
            rect = page.rect
            center_x = rect.width / 2
            center_y = rect.height / 2

            # Normalize rotation to 0-360 range and round to nearest multiple of 90
            # PyMuPDF only supports rotation values that are multiples of 90
            rotation = rotation % 360
            rotation = round(rotation / 90) * 90 % 360

            # For rotated text, we need to adjust the position
            # since insert_text rotates around the insertion point
            if rotation == 0:
                # No rotation - use center directly
                point = fitz.Point(center_x, center_y)
                page.insert_text(
                    point,
                    text,
                    fontsize=font_size,
                    fontname="helv",
                    color=color,
                    fill_opacity=opacity,
                )
            else:
                # For rotation, use a textbox with proper alignment
                # Create a centered textbox
                box_width = rect.width * 0.8
                box_height = rect.height * 0.2
                textbox_rect = fitz.Rect(
                    center_x - box_width / 2,
                    center_y - box_height / 2,
                    center_x + box_width / 2,
                    center_y + box_height / 2,
                )

                # Use 0 (center) for align
                page.insert_textbox(
                    textbox_rect,
                    text,
                    fontsize=font_size,
                    fontname="helv",
                    color=color,
                    fill_opacity=opacity,
                    align=0,  # Center align
                    rotate=rotation,
                )

        doc.save(output_path)
        doc.close()

    def rotate_pdf(
        self,
        file_path: str,
        output_path: str,
        rotation: int = 90,
        page_nums: Optional[List[int]] = None,
    ):
        """
        Rotate PDF pages.
        rotation: Angle in degrees (must be multiple of 90).
        page_nums: List of page numbers to rotate (0-indexed). If None, rotate all.
        """
        doc = fitz.open(file_path)

        if page_nums is None:
            pages_to_rotate = list(range(len(doc)))
        else:
            pages_to_rotate = page_nums

        for page_num in pages_to_rotate:
            if 0 <= page_num < len(doc):
                page = doc[page_num]
                # Normalize rotation to 0-360 range
                new_rotation = (page.rotation + rotation) % 360
                page.set_rotation(new_rotation)

        doc.save(output_path)
        doc.close()

    def unlock_pdf(self, file_path: str, password: str, output_path: str):
        """
        Unlock PDF (remove password).
        """
        doc = fitz.open(file_path)
        if doc.is_encrypted:
            if not doc.authenticate(password):
                raise ValueError("Incorrect password")

        doc.save(output_path)
        doc.close()

    def protect_pdf(self, file_path: str, password: str, output_path: str):
        """
        Protect PDF with password.
        """
        doc = fitz.open(file_path)
        doc.save(
            output_path,
            encryption=fitz.PDF_ENCRYPT_AES_256,
            owner_pw=password,
            user_pw=password,
        )
        doc.close()

    def organize_pdf(self, file_path: str, page_order: List[int], output_path: str):
        """
        Organize PDF pages (reorder/delete).
        page_order: List of page numbers in desired order.
        """
        doc = fitz.open(file_path)
        new_doc = fitz.open()

        for page_num in page_order:
            if 0 <= page_num < len(doc):
                new_doc.insert_pdf(doc, from_page=page_num, to_page=page_num)

        new_doc.save(output_path)
        new_doc.close()
        doc.close()

    def repair_pdf(self, file_path: str, output_path: str):
        """
        Repair PDF using Ghostscript.
        """
        import subprocess

        _require_dependency("gs", "Ghostscript")

        cmd = [
            "gs",
            "-o",
            output_path,
            "-sDEVICE=pdfwrite",
            "-dPDFSETTINGS=/prepress",
            file_path,
        ]

        try:
            subprocess.run(
                cmd, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
            )
        except subprocess.CalledProcessError as e:
            print(f"Ghostscript repair failed: {e.stderr.decode()}")
            raise ValueError("Failed to repair PDF")

    def add_page_numbers(
        self, file_path: str, output_path: str, position: str = "bottom-center"
    ):
        """
        Add page numbers to PDF.
        position: 'bottom-center', 'bottom-right', 'bottom-left', 'top-center', 'top-right', 'top-left'
        """
        doc = fitz.open(file_path)

        for page_num, page in enumerate(doc):
            text = f"{page_num + 1}"
            rect = page.rect

            # Determine position
            if position == "bottom-center":
                point = fitz.Point(rect.width / 2, rect.height - 20)
                align = 1  # Center
            elif position == "bottom-right":
                point = fitz.Point(rect.width - 40, rect.height - 20)
                align = 2  # Right
            elif position == "bottom-left":
                point = fitz.Point(40, rect.height - 20)
                align = 0  # Left
            elif position == "top-center":
                point = fitz.Point(rect.width / 2, 40)
                align = 1
            elif position == "top-right":
                point = fitz.Point(rect.width - 40, 40)
                align = 2
            elif position == "top-left":
                point = fitz.Point(40, 40)
                align = 0
            else:
                point = fitz.Point(rect.width / 2, rect.height - 20)
                align = 1

            # Let's use insert_textbox for alignment support
            if "bottom" in position:
                y0 = rect.height - 40
                y1 = rect.height - 10
            else:
                y0 = 10
                y1 = 40

            if "center" in position:
                x0 = 0
                x1 = rect.width
            elif "right" in position:
                x0 = rect.width - 100
                x1 = rect.width - 10
            else:  # left
                x0 = 10
                x1 = 100

            textbox_rect = fitz.Rect(x0, y0, x1, y1)
            page.insert_textbox(
                textbox_rect,
                text,
                fontsize=12,
                fontname="helv",
                color=(0, 0, 0),
                align=align,
            )

        doc.save(output_path)
        doc.close()

    def compare_pdfs(self, file_path1: str, file_path2: str, output_path: str):
        """
        Compare two PDFs and generate a visual diff PDF.
        """
        import io

        import fitz
        from PIL import Image, ImageChops

        doc1 = fitz.open(file_path1)
        doc2 = fitz.open(file_path2)
        out_doc = fitz.open()

        # Iterate over max pages
        max_pages = max(len(doc1), len(doc2))

        for i in range(max_pages):
            # Get page 1 image
            img1: Image.Image
            if i < len(doc1):
                pix1 = doc1[i].get_pixmap()
                img1 = Image.open(io.BytesIO(pix1.tobytes("png")))
            else:
                # Create blank white image
                img1 = Image.new("RGB", (595, 842), "white")  # A4 approx

            # Get page 2 image
            img2: Image.Image
            if i < len(doc2):
                pix2 = doc2[i].get_pixmap()
                img2 = Image.open(io.BytesIO(pix2.tobytes("png")))
            else:
                img2 = Image.new("RGB", (595, 842), "white")

            # Resize to match if needed
            if img1.size != img2.size:
                img2 = img2.resize(img1.size)

            # Compute difference
            diff = ImageChops.difference(img1, img2)

            # Invert diff to make it white background with black diffs
            diff = ImageChops.invert(diff)

            # Save diff to temp file
            temp_diff_path = f"diff_page_{i}.jpg"
            diff.save(temp_diff_path)

            # Add to output PDF
            img_doc = fitz.open(temp_diff_path)
            pdfbytes = img_doc.convert_to_pdf()
            img_pdf = fitz.open("pdf", pdfbytes)
            out_doc.insert_pdf(img_pdf)
            img_doc.close()
            img_pdf.close()

            if os.path.exists(temp_diff_path):
                os.remove(temp_diff_path)

        out_doc.save(output_path)
        out_doc.close()
        doc1.close()
        doc2.close()
