"""
Security-focused tests for PDF Smart Editor API.

Tests file validation, path traversal prevention, and input sanitization.
"""

import os

import pytest
from fastapi import HTTPException

from api.security import (
    get_safe_filename,
    get_security_headers,
    sanitize_filename,
    sanitize_user_input,
    validate_content_type,
    validate_file_signature,
    validate_pdf_file,
)


class TestFileSignatureValidation:
    """Tests for PDF file signature validation."""

    def test_valid_pdf_signature(self):
        """Test that valid PDF signature is accepted."""
        valid_pdf = b"%PDF-1.4\n%%EOF"
        assert validate_file_signature(valid_pdf) is True

    def test_invalid_pdf_signature(self):
        """Test that invalid file signature is rejected."""
        invalid_files = [
            b"Hello World",
            b"<!DOCTYPE html>",
            b"\x89PNG\r\n\x1a\n",  # PNG signature
            b"GIF87a",  # GIF signature
        ]
        for invalid_file in invalid_files:
            assert validate_file_signature(invalid_file) is False

    def test_empty_file(self):
        """Test that empty file is rejected."""
        assert validate_file_signature(b"") is False


class TestFilenameSanitization:
    """Tests for filename sanitization."""

    def test_basic_filename(self):
        """Test that basic filename is preserved."""
        assert sanitize_filename("document.pdf") == "document.pdf"

    def test_path_traversal_prevention(self):
        """Test that path traversal attempts are blocked."""
        dangerous_filenames = [
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32",
            "./secret.pdf",
            "~/.ssh/config",
        ]
        for filename in dangerous_filenames:
            with pytest.raises(HTTPException) as exc_info:
                sanitize_filename(filename)
            assert exc_info.value.status_code == 400
            assert "Path traversal" in str(exc_info.value.detail)

    def test_null_byte_injection(self):
        """Test that null bytes are rejected."""
        with pytest.raises(HTTPException) as exc_info:
            sanitize_filename("test\x00.pdf")
        assert exc_info.value.status_code == 400

    def test_adds_pdf_extension(self):
        """Test that .pdf extension is added if missing."""
        assert sanitize_filename("document") == "document.pdf"
        assert (
            sanitize_filename("document.PDF") == "document.pdf"
        )  # Case preserved, extension added

    def test_empty_filename(self):
        """Test that empty filename is rejected."""
        with pytest.raises(HTTPException) as exc_info:
            sanitize_filename("")
        assert exc_info.value.status_code == 400

    def test_max_filename_length(self):
        """Test that overly long filenames are rejected."""
        long_filename = "a" * 300 + ".pdf"
        with pytest.raises(HTTPException) as exc_info:
            sanitize_filename(long_filename)
        assert exc_info.value.status_code == 400
        assert "too long" in str(exc_info.value.detail).lower()


class TestContentTypeValidation:
    """Tests for content type validation."""

    def test_valid_content_type(self):
        """Test that valid content types are accepted."""
        allowed_types = {"application/pdf"}
        assert validate_content_type("application/pdf", allowed_types) is True

    def test_invalid_content_type(self):
        """Test that invalid content types are rejected."""
        allowed_types = {"application/pdf"}
        invalid_types = [
            "text/html",
            "image/jpeg",
            "application/octet-stream",
            None,
        ]
        for content_type in invalid_types:
            assert validate_content_type(content_type, allowed_types) is False

    def test_wildcard_content_type(self):
        """Test that wildcard patterns work."""
        allowed_types = {"application/*"}
        assert validate_content_type("application/pdf", allowed_types) is True
        assert validate_content_type("application/json", allowed_types) is True
        assert validate_content_type("text/html", allowed_types) is False


class TestPDFValidation:
    """Tests for comprehensive PDF validation."""

    def test_valid_pdf_passes(self):
        """Test that valid PDF passes all validations."""
        valid_pdf = b"%PDF-1.4\n" + b"x" * 1000
        validate_pdf_file(valid_pdf, "test.pdf", max_size_bytes=10 * 1024 * 1024)
        # No exception raised

    def test_empty_pdf_rejected(self):
        """Test that empty file is rejected."""
        with pytest.raises(HTTPException) as exc_info:
            validate_pdf_file(b"", "test.pdf", max_size_bytes=10 * 1024 * 1024)
        assert exc_info.value.status_code == 400
        assert "empty" in str(exc_info.value.detail).lower()

    def test_oversized_pdf_rejected(self):
        """Test that oversized file is rejected."""
        large_pdf = b"%PDF-1.4\n" + b"x" * (10 * 1024 * 1024 + 1)
        with pytest.raises(HTTPException) as exc_info:
            validate_pdf_file(large_pdf, "test.pdf", max_size_bytes=10 * 1024 * 1024)
        assert exc_info.value.status_code == 413
        assert "too large" in str(exc_info.value.detail).lower()

    def test_invalid_signature_rejected(self):
        """Test that file without PDF signature is rejected."""
        invalid_content = b"This is not a PDF"
        with pytest.raises(HTTPException) as exc_info:
            validate_pdf_file(
                invalid_content, "fake.pdf", max_size_bytes=10 * 1024 * 1024
            )
        assert exc_info.value.status_code == 400
        assert "signature" in str(exc_info.value.detail).lower()


class TestUserInputSanitization:
    """Tests for user input sanitization."""

    def test_basic_text(self):
        """Test that normal text is preserved."""
        assert sanitize_user_input("Hello World") == "Hello World"

    def test_removes_null_bytes(self):
        """Test that null bytes are removed."""
        assert sanitize_user_input("test\x00text") == "testtext"

    def test_removes_control_characters(self):
        """Test that control characters are removed (except newlines/tabs)."""
        assert sanitize_user_input("test\x01\x02text") == "testtext"
        assert "\n" in sanitize_user_input("test\ntext")
        assert "\t" in sanitize_user_input("test\ttext")

    def test_truncates_long_input(self):
        """Test that long input is truncated."""
        long_input = "a" * 2000
        result = sanitize_user_input(long_input, max_length=100)
        assert len(result) == 100

    def test_empty_input(self):
        """Test that empty input returns empty string."""
        assert sanitize_user_input("") == ""


class TestSafeFilenameGeneration:
    """Tests for safe filename generation."""

    def test_generates_safe_filename(self):
        """Test that safe filename includes session ID."""
        result = get_safe_filename("document.pdf", "session-123")
        assert result.startswith("session-123_")
        assert result.endswith(".pdf")

    def test_sanitizes_before_prefixing(self):
        """Test that dangerous filenames are sanitized."""
        with pytest.raises(HTTPException):
            get_safe_filename("../../../etc/passwd", "session-123")


class TestSecurityHeaders:
    """Tests for security headers."""

    def test_returns_expected_headers(self):
        """Test that all expected security headers are present."""
        headers = get_security_headers()

        expected_keys = {
            "X-Content-Type-Options",
            "X-Frame-Options",
            "X-XSS-Protection",
            "Referrer-Policy",
            "Permissions-Policy",
        }
        assert set(headers.keys()) == expected_keys

    def test_x_frame_options(self):
        """Test that X-Frame-Options prevents clickjacking."""
        headers = get_security_headers()
        assert headers["X-Frame-Options"] == "SAMEORIGIN"

    def test_content_type_options(self):
        """Test that X-Content-Type-Options prevents MIME sniffing."""
        headers = get_security_headers()
        assert headers["X-Content-Type-Options"] == "nosniff"

    def test_xss_protection(self):
        """Test that X-XSS-Protection is enabled."""
        headers = get_security_headers()
        assert headers["X-XSS-Protection"] == "1; mode=block"

    def test_permissions_policy(self):
        """Test that Permissions-Policy restricts sensitive APIs."""
        headers = get_security_headers()
        policy = headers["Permissions-Policy"]
        assert "geolocation=()" in policy
        assert "microphone=()" in policy
        assert "camera=()" in policy
