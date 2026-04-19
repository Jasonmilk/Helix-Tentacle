from typing import Literal, Optional
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8", extra="ignore")

    # Core mode
    TENTACLE_MODE: Literal["embedded", "standalone"] = "embedded"

    # Server
    TENTACLE_HOST: str = "0.0.0.0"
    TENTACLE_PORT: int = 8021

    # Search
    TENTACLE_SEARCH_PROVIDER: Literal["duckduckgo", "serpapi", "bing"] = "duckduckgo"
    TENTACLE_SERPAPI_KEY: Optional[str] = None

    # Network & Security
    TENTACLE_REQUEST_TIMEOUT: int = 10
    TENTACLE_MAX_SNIPPET_SIZE: int = 10240
    TENTACLE_ALLOW_PRIVATE_IPS: bool = False
    TENTACLE_USER_AGENT: str = "Mozilla/5.0 (compatible; Helix-Tentacle/2.1; +https://helix-ai.dev)"

    # Cache
    TENTACLE_CACHE_ENABLED: bool = True
    TENTACLE_CACHE_TTL: int = 30
    TENTACLE_CACHE_MAX_SIZE: int = 100

    # Logging & Observability
    TENTACLE_LOG_LEVEL: Literal["debug", "info", "warning", "error"] = "info"
    TENTACLE_METRICS_ENABLED: bool = True


# Global settings instance
settings = Settings()