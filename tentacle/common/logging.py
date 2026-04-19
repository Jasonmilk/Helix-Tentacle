import logging
from typing import Any, Dict

import structlog

from .config import settings

__all__ = ["logger", "configure_logging"]


def configure_logging() -> None:
    """Configure structlog based on running mode."""
    processors = [
        structlog.contextvars.merge_contextvars,
        structlog.processors.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
    ]

    if settings.TENTACLE_MODE == "embedded":
        # Production mode: JSON output for log aggregation
        processors.append(structlog.processors.JSONRenderer())
    else:
        # Standalone mode: human-readable console output
        processors.extend([
            structlog.processors.UnicodeDecoder(),
            structlog.dev.ConsoleRenderer(),
        ])

    structlog.configure(
        processors=processors,
        wrapper_class=structlog.make_filtering_bound_logger(logging.INFO),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(),
        cache_logger_on_first_use=True,
    )

    # Set root log level
    log_level = getattr(logging, settings.TENTACLE_LOG_LEVEL.upper(), logging.INFO)
    logging.basicConfig(level=log_level)


# Lazy initialization: configure on first logger access
_logger: structlog.BoundLogger | None = None


def get_logger() -> structlog.BoundLogger:
    """Return the global logger, configuring on first call."""
    global _logger
    if _logger is None:
        configure_logging()
        _logger = structlog.get_logger()
    return _logger


# Expose a module-level logger proxy
class _LoggerProxy:
    """Proxy to lazy logger, so imports don't trigger configuration."""

    def __getattr__(self, name: str) -> Any:
        return getattr(get_logger(), name)


logger = _LoggerProxy()