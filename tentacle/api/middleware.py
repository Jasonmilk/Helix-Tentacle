import time
import structlog
from fastapi import Request
from starlette.middleware.base import BaseHTTPMiddleware
from tentacle.common.tracing import extract_trace_id
from tentacle.common.logging import logger


class TracingMiddleware(BaseHTTPMiddleware):
    """
    Middleware to extract and bind trace_id to logging context.
    """
    async def dispatch(self, request: Request, call_next):
        # Extract trace_id from headers
        trace_id = extract_trace_id(dict(request.headers))
        
        # Bind trace_id to logger context
        structlog.contextvars.bind_contextvars(trace_id=trace_id)
        
        # Process request
        start_time = time.time()
        response = await call_next(request)
        
        # Add trace_id to response headers
        response.headers["X-Trace-Id"] = trace_id
        
        # Log request
        duration_ms = int((time.time() - start_time) * 1000)
        logger.info(
            "Request processed",
            method=request.method,
            path=request.url.path,
            status_code=response.status_code,
            duration_ms=duration_ms
        )
        
        return response
