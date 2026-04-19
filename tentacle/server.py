import uvicorn
from tentacle.common.config import settings
from tentacle.api import create_app


def main():
    """Start the embedded mode API server."""
    if settings.TENTACLE_MODE != "embedded":
        raise RuntimeError(
            "Server can only be started in embedded mode. "
            "Set TENTACLE_MODE=embedded to run the server."
        )

    app = create_app()
    
    uvicorn.run(
        app,
        host=settings.TENTACLE_HOST,
        port=settings.TENTACLE_PORT,
        log_level=settings.TENTACLE_LOG_LEVEL.lower(),
    )


if __name__ == "__main__":
    main()
