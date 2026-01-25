"""
Middleware components for PDF Smart Editor API.

Provides rate limiting, request logging, and security middleware.
"""

import logging
import os
import time
from collections import defaultdict
from typing import Callable, Dict, Tuple

from fastapi import Request, Response
from starlette.middleware.base import BaseHTTPMiddleware

from api.security import get_security_headers

logger = logging.getLogger(__name__)

# Rate limiting configuration
RATE_LIMIT_REQUESTS = int(
    os.getenv("RATE_LIMIT_REQUESTS", "100")
)  # requests per window
RATE_LIMIT_WINDOW = int(os.getenv("RATE_LIMIT_WINDOW", "60"))  # window in seconds


class RateLimitMiddleware(BaseHTTPMiddleware):
    """
    Simple in-memory rate limiting middleware.

    Limits requests per IP address within a sliding window.
    For production, consider using Redis-based rate limiting.
    """

    def __init__(
        self,
        app,
        requests_limit: int = RATE_LIMIT_REQUESTS,
        window_seconds: int = RATE_LIMIT_WINDOW,
    ):
        super().__init__(app)
        self.requests_limit = requests_limit
        self.window_seconds = window_seconds
        # Track requests per IP: {ip: [(timestamp, count), ...]}
        self._requests: Dict[str, list] = defaultdict(list)

    def _get_client_ip(self, request: Request) -> str:
        """Extract client IP from request, considering proxies."""
        # Check X-Forwarded-For header (for proxied requests)
        forwarded = request.headers.get("X-Forwarded-For")
        if forwarded:
            return forwarded.split(",")[0].strip()
        # Check X-Real-IP header
        real_ip = request.headers.get("X-Real-IP")
        if real_ip:
            return real_ip
        # Fall back to direct client
        return request.client.host if request.client else "unknown"

    def _cleanup_old_requests(self, ip: str, current_time: float) -> None:
        """Remove requests outside the current window."""
        cutoff = current_time - self.window_seconds
        self._requests[ip] = [ts for ts in self._requests[ip] if ts > cutoff]

    def _is_rate_limited(self, ip: str) -> Tuple[bool, int]:
        """
        Check if an IP is rate limited.

        Returns: (is_limited, requests_remaining)
        """
        current_time = time.time()
        self._cleanup_old_requests(ip, current_time)

        request_count = len(self._requests[ip])
        remaining = max(0, self.requests_limit - request_count)

        if request_count >= self.requests_limit:
            return True, remaining

        self._requests[ip].append(current_time)
        return False, remaining - 1

    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        client_ip = self._get_client_ip(request)
        is_limited, remaining = self._is_rate_limited(client_ip)

        if is_limited:
            logger.warning(f"Rate limit exceeded for IP: {client_ip}")
            return Response(
                content='{"detail": "Too many requests. Please try again later."}',
                status_code=429,
                media_type="application/json",
                headers={
                    "Retry-After": str(self.window_seconds),
                    "X-RateLimit-Limit": str(self.requests_limit),
                    "X-RateLimit-Remaining": "0",
                    "X-RateLimit-Reset": str(int(time.time()) + self.window_seconds),
                },
            )

        response = await call_next(request)

        # Add rate limit headers to all responses
        response.headers["X-RateLimit-Limit"] = str(self.requests_limit)
        response.headers["X-RateLimit-Remaining"] = str(remaining)
        response.headers["X-RateLimit-Reset"] = str(
            int(time.time()) + self.window_seconds
        )

        return response


class RequestLoggingMiddleware(BaseHTTPMiddleware):
    """
    Middleware for logging HTTP requests and responses.

    Logs request method, path, status code, and duration.
    """

    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        start_time = time.time()

        # Log request
        logger.info(f"Request: {request.method} {request.url.path}")

        try:
            response = await call_next(request)
        except Exception as exc:
            logger.error(f"Request failed: {request.method} {request.url.path} - {exc}")
            raise

        # Calculate duration
        duration_ms = (time.time() - start_time) * 1000

        # Log response
        logger.info(
            f"Response: {request.method} {request.url.path} "
            f"- Status: {response.status_code} - Duration: {duration_ms:.2f}ms"
        )

        return response


class SecurityHeadersMiddleware(BaseHTTPMiddleware):
    """
    Middleware to add security headers to all responses.
    """

    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        response = await call_next(request)

        # Add security headers
        for header, value in get_security_headers().items():
            response.headers[header] = value

        return response
