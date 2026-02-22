import logging
import os
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from api.deps import cleanup_all_sessions, cleanup_stale_sessions

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
    yield
    # Shutdown
    logger.info("Shutting down PDF Editor Offline API...")
    cleanup_all_sessions()


app = FastAPI(
    title="PDF Editor Offline API",
    version="1.0.0",
    lifespan=lifespan,
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
    RateLimitMiddleware,
    RequestLoggingMiddleware,
    SecurityHeadersMiddleware,
)

app.add_middleware(SecurityHeadersMiddleware)
app.add_middleware(RequestLoggingMiddleware)
app.add_middleware(RateLimitMiddleware)
# Keep CORS as the outermost middleware so headers are present on all responses.
app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_origin_regex=ALLOW_ORIGIN_REGEX,
    allow_credentials=True,
    allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allow_headers=["*"],
    expose_headers=["Content-Disposition"],
)

# Import routes after app is created to avoid circular imports
from api.routes import documents, tools

# Register routes
app.include_router(documents.router)
app.include_router(tools.router)


if __name__ == "__main__":
    uvicorn.run("api.main:app", host="0.0.0.0", port=8000, reload=True)
