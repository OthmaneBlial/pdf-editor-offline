import logging
import os
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from api.deps import cleanup_all_sessions, cleanup_stale_sessions, get_session
from api.change_reviews import cleanup_all_change_reviews, cleanup_stale_change_reviews
from api.ocr_jobs import shutdown_ocr_jobs
from api.capabilities import get_runtime_capabilities
from api.security import sanitize_error_detail
from pdf_editor_offline import __version__ as package_version
from pdf_editor_offline.core.exceptions import InvalidOperationError, MissingDependencyError
from pdf_editor_offline.core.accessibility_inspector import accessibility_preservation_warnings

# Setup logging
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Lifespan context manager for startup and shutdown events."""
    # Startup
    logger.info("Starting up PDF Editor Offline API...")
    cleanup_stale_sessions()
    cleanup_stale_change_reviews()
    yield
    # Shutdown
    logger.info("Shutting down PDF Editor Offline API...")
    shutdown_ocr_jobs()
    cleanup_all_change_reviews()
    cleanup_all_sessions()


app = FastAPI(
    title="PDF Editor Offline API",
    version=package_version,
    lifespan=lifespan,
)


@app.middleware("http")
async def warn_before_editing_tagged_documents(request: Request, call_next):
    """Expose a visible warning for every successful document mutation."""
    path_parts = request.url.path.strip("/").split("/")
    document_route = "/".join(path_parts[3:])
    non_editing_post = (
        document_route in {
            "redaction/review",
            "digital-signatures/validate",
            "images/extract",
        }
        or document_route.endswith("/text/search")
        or document_route.endswith("/text/replace/preflight")
    )
    mutating_document = (
        request.method in {"POST", "PUT", "PATCH", "DELETE"}
        and len(path_parts) >= 4
        and path_parts[:2] == ["api", "documents"]
        and path_parts[2] not in {"upload", "maintenance", "recovery"}
        and not non_editing_post
    )
    warning = False
    if mutating_document:
        try:
            session = get_session(path_parts[2])
            document = session["document_manager"].get_document()
            warning = bool(
                accessibility_preservation_warnings(document, "content_edit")
            )
        except Exception:
            # Route validation owns unknown or expired session errors.
            pass
    response = await call_next(request)
    if warning and response.status_code < 400:
        response.headers[
            "X-PDF-Accessibility-Warning"
        ] = "accessibility_semantics_may_be_degraded"
    return response


@app.get("/api/health", tags=["health"])
async def health_check():
    return {
        "status": "ok",
        "name": "PDF Editor Offline API",
        "version": package_version,
        "auth_required": bool(os.getenv("PDF_EDITOR_OFFLINE_API_TOKEN")),
    }


@app.get("/api/capabilities", tags=["health"])
async def runtime_capabilities():
    return get_runtime_capabilities()


@app.exception_handler(InvalidOperationError)
async def handle_invalid_operation(
    request: Request, exc: InvalidOperationError
) -> JSONResponse:
    """Return consistent 400 responses for user-triggered invalid operations."""
    return JSONResponse(
        status_code=400, content={"detail": sanitize_error_detail(str(exc))}
    )


@app.exception_handler(MissingDependencyError)
async def handle_missing_dependency(
    request: Request, exc: MissingDependencyError
) -> JSONResponse:
    return JSONResponse(
        status_code=503,
        content={
            "detail": sanitize_error_detail(str(exc)),
            "code": "missing_local_dependency",
            "dependency": exc.friendly_name,
            "command": exc.command,
        },
    )


# CORS configuration:
# - If CORS_ORIGINS is set, only those origins are allowed.
# - Otherwise, allow localhost/127.0.0.1 on any port for local dev.
cors_origins_env = os.getenv("CORS_ORIGINS", "").strip()
if cors_origins_env:
    ALLOWED_ORIGINS = [o.strip() for o in cors_origins_env.split(",") if o.strip()]
    ALLOW_ORIGIN_REGEX = None
else:
    ALLOWED_ORIGINS = [
        "http://localhost",
        "http://127.0.0.1",
    ]
    ALLOW_ORIGIN_REGEX = r"^https?://(localhost|127\.0\.0\.1)(:\d+)?$"

# Add security and logging middleware
from api.middleware import (
    LocalAPITokenMiddleware,
    RateLimitMiddleware,
    RequestLoggingMiddleware,
    SecurityHeadersMiddleware,
)

app.add_middleware(SecurityHeadersMiddleware)
app.add_middleware(RequestLoggingMiddleware)
app.add_middleware(RateLimitMiddleware)
app.add_middleware(LocalAPITokenMiddleware)
# Keep CORS as the outermost middleware so headers are present on all responses.
app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_origin_regex=ALLOW_ORIGIN_REGEX,
    allow_credentials=True,
    allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allow_headers=["*"],
    expose_headers=[
        "Content-Disposition",
        "X-Signature-Field",
        "X-Source-Preserved",
        "X-Private-Key-Persisted",
        "X-Timestamp-Embedded",
        "X-Online-Revocation-Checked",
        "X-Safe-Edit",
        "X-Change-Audit-SHA256",
        "X-Output-SHA256",
        "X-PDF-Accessibility-Warning",
        "X-PDF-Editor-Warnings",
    ],
)

# Import routes after app is created to avoid circular imports
from api.routes import documents, ocr, sanitization, tools

# Register routes
app.include_router(documents.router)
app.include_router(ocr.router)
app.include_router(sanitization.router)
app.include_router(tools.router)


if __name__ == "__main__":
    uvicorn.run(
        "api.main:app",
        host=os.getenv("PDF_EDITOR_OFFLINE_API_HOST", "127.0.0.1"),
        port=int(os.getenv("PDF_EDITOR_OFFLINE_API_PORT", "8000")),
        reload=os.getenv("PDF_EDITOR_OFFLINE_RELOAD", "0") == "1",
    )
