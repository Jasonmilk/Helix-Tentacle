from fastapi import FastAPI
from prometheus_client import generate_latest, CONTENT_TYPE_LATEST
from fastapi.responses import Response
from tentacle.common.config import settings
from .v1_router import router as v1_router
from .middleware import TracingMiddleware


def create_app() -> FastAPI:
    """Create FastAPI application for Embedded mode."""
    app = FastAPI(
        title="Helix-Tentacle API",
        description="External perception and progressive information sniffing microservice",
        version="2.1.0",
        docs_url="/docs" if settings.TENTACLE_MODE == "embedded" else None,
        redoc_url="/redoc" if settings.TENTACLE_MODE == "embedded" else None,
    )

    # Add middleware
    app.add_middleware(TracingMiddleware)

    # Add routes
    app.include_router(v1_router)

    # Add metrics endpoint if enabled
    if settings.TENTACLE_METRICS_ENABLED:
        @app.get("/metrics")
        async def metrics():
            return Response(
                generate_latest(),
                media_type=CONTENT_TYPE_LATEST
            )

    @app.get("/health")
    async def health():
        return {"status": "ok", "mode": settings.TENTACLE_MODE}

    return app


__all__ = ["create_app"]
