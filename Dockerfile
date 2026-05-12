# Use Python 3.12 slim image
FROM python:3.12-slim

# Set working directory
WORKDIR /app

# Install system dependencies for PyMuPDF and conversion tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    libmupdf-dev \
    libfreetype6-dev \
    libjpeg-dev \
    libpng-dev \
    libtiff-dev \
    liblcms2-dev \
    libwebp-dev \
    libharfbuzz-dev \
    libfribidi-dev \
    ghostscript \
    libreoffice \
    tesseract-ocr \
    && rm -rf /var/lib/apt/lists/*
# Copy project files
COPY pyproject.toml README.md LICENSE ./
COPY pdf_editor_offline/ ./pdf_editor_offline/
COPY api/ ./api/

# Install the package
RUN pip install --no-cache-dir .

# Create a non-root user
RUN useradd --create-home --shell /bin/bash app \
    && chown -R app:app /app
USER app

# Default command
CMD ["uvicorn", "api.main:app", "--host", "0.0.0.0", "--port", "8000"]
