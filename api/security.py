"""
Security utilities for PDF Smart Editor API.

Provides file validation, sanitization, and security headers.
"""

import logging
import os
import re
from pathlib import Path
from typing import Optional, Set

from fastapi import HTTPException

logger = logging.getLogger(__name__)

# PDF file signature (magic bytes)
PDF_SIGNATURE = b'%PDF-'

# Maximum filename length
MAX_FILENAME_LENGTH = 255

# Allowed characters for filenames (sanitized)
ALLOWED_FILENAME_PATTERN = re.compile(r'^[\w\s\-_.()]+\.pdf$', re.IGNORECASE)

# Common dangerous path traversal patterns
PATH_TRAVERSAL_PATTERNS = [
    '..',
    '~',
    './',
    '//',
    '\\',
    '\x00',  # Null byte
]


def validate_file_signature(file_content: bytes) -> bool:
    """
    Validate PDF file signature (magic bytes).

    Args:
        file_content: The file content to validate

    Returns:
        True if the file has a valid PDF signature
    """
    if not file_content:
        return False
    return file_content.startswith(PDF_SIGNATURE)


def sanitize_filename(filename: str) -> str:
    """
    Sanitize filename to prevent path traversal and other attacks.

    Args:
        filename: The original filename

    Returns:
        A sanitized safe filename

    Raises:
        HTTPException: If the filename is dangerous or invalid
    """
    if not filename:
        raise HTTPException(status_code=400, detail="Filename cannot be empty")

    # Check for path traversal patterns BEFORE extracting basename
    for pattern in PATH_TRAVERSAL_PATTERNS:
        if pattern in filename:
            logger.warning(f"Potential path traversal attempt in filename: {filename}")
            raise HTTPException(
                status_code=400,
                detail="Invalid filename. Path traversal not allowed."
            )

    # Use pathlib for safe basename extraction (after path traversal check)
    safe_name = Path(filename).name

    # Check filename length
    if len(safe_name) > MAX_FILENAME_LENGTH:
        raise HTTPException(
            status_code=400,
            detail=f"Filename too long (max {MAX_FILENAME_LENGTH} characters)."
        )

    # Ensure .pdf extension (normalize to lowercase)
    # Remove any existing extension and add .pdf
    safe_name_without_ext = safe_name.rsplit('.', 1)[0] if '.' in safe_name else safe_name
    safe_name = safe_name_without_ext + '.pdf'

    return safe_name


def validate_content_type(
    content_type: Optional[str],
    allowed_types: Set[str]
) -> bool:
    """
    Validate content type against allowed types.

    Args:
        content_type: The content type to validate
        allowed_types: Set of allowed content types

    Returns:
        True if the content type is allowed
    """
    if not content_type:
        return False

    # Normalize content type
    content_type = content_type.lower().strip()

    # Check exact match or wildcard pattern
    for allowed in allowed_types:
        allowed_lower = allowed.lower()
        if content_type == allowed_lower:
            return True
        # Handle wildcards like 'application/*'
        if allowed_lower.endswith('/*'):
            category = allowed_lower.split('/')[0]
            if content_type.startswith(category + '/'):
                return True

    return False


def get_safe_filename(original_filename: str, session_id: str) -> str:
    """
    Generate a safe filename for storage using session ID.

    Args:
        original_filename: The original filename
        session_id: The session UUID

    Returns:
        A safe filename for storage
    """
    safe_name = sanitize_filename(original_filename)
    return f"{session_id}_{safe_name}"


def validate_pdf_file(
    file_content: bytes,
    filename: str,
    max_size_bytes: int
) -> None:
    """
    Comprehensive PDF file validation.

    Args:
        file_content: The file content
        filename: The filename
        max_size_bytes: Maximum allowed file size

    Raises:
        HTTPException: If validation fails
    """
    # Check file size
    if len(file_content) == 0:
        raise HTTPException(status_code=400, detail="File is empty")

    if len(file_content) > max_size_bytes:
        max_mb = max_size_bytes / (1024 * 1024)
        raise HTTPException(
            status_code=413,
            detail=f"File too large (max {max_mb:.0f} MB)."
        )

    # Validate PDF signature
    if not validate_file_signature(file_content):
        logger.warning(f"Invalid PDF signature for file: {filename}")
        raise HTTPException(
            status_code=400,
            detail="Invalid PDF file. File does not have a valid PDF signature."
        )


def sanitize_user_input(text: str, max_length: int = 1000) -> str:
    """
    Sanitize user input text.

    Args:
        text: The user input text
        max_length: Maximum allowed length

    Returns:
        Sanitized text
    """
    if not text:
        return ""

    # Remove null bytes and control characters (except newlines and tabs)
    cleaned = ''.join(
        c for c in text
        if c == '\n' or c == '\t' or c == '\r' or ord(c) >= 32
    )

    # Truncate to max length
    return cleaned[:max_length]


# Security headers configuration
SECURITY_HEADERS = {
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "SAMEORIGIN",
    "X-XSS-Protection": "1; mode=block",
    "Referrer-Policy": "strict-origin-when-cross-origin",
    "Permissions-Policy": "geolocation=(), microphone=(), camera=()",
}


def get_security_headers() -> dict:
    """
    Get security headers for responses.

    Returns:
        Dictionary of security headers
    """
    return SECURITY_HEADERS.copy()
