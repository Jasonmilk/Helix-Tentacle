"""Trace ID generation and propagation — W3C Trace Context compatible."""

import uuid
from contextvars import ContextVar

import structlog

# Context variable to store trace_id across async tasks
_trace_id_var: ContextVar[str] = ContextVar("trace_id", default="")


def generate_trace_id() -> str:
    """
    Generate a 128-bit trace identifier compatible with W3C Trace Context.
    Returns a 32-character hexadecimal string (UUIDv4 hex without dashes).
    """
    return uuid.uuid4().hex


def set_trace_id(trace_id: str) -> None:
    """
    Set the trace ID for the current request context.
    Also binds it to structlog's contextvars so all subsequent logs
    automatically include the trace_id field.
    """
    _trace_id_var.set(trace_id)
    structlog.contextvars.bind_contextvars(trace_id=trace_id)


def get_trace_id() -> str:
    """Retrieve the trace ID for the current request context."""
    return _trace_id_var.get()


def generate_epoch_id() -> str:
    """Generate a unique epoch identifier."""
    return f"epoch_{uuid.uuid4().hex[:12]}"