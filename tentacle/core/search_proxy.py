"""Search proxy with pluggable providers. Zero hardcoding."""

import asyncio
from pathlib import Path
from typing import Dict, List, Optional, Protocol, Type, Any
from urllib.parse import urlparse

import yaml

from tentacle.common.config import settings
from tentacle.common.logging import logger
from tentacle.schemas.search import SearchResult, SearchResultItem
from tentacle.schemas.forage import KeywordFilter


# ------------------------------------------------------------------------
# Provider Protocol (contract for all search implementations)
# ------------------------------------------------------------------------
class SearchProvider(Protocol):
    """Protocol that all search providers must implement."""

    async def search(self, query: str, limit: int) -> List[SearchResultItem]:
        ...


# ------------------------------------------------------------------------
# DuckDuckGo Provider
# ------------------------------------------------------------------------
class DuckDuckGoProvider:
    """Search using DuckDuckGo (via ddgs)."""

    def __init__(self, user_agent: str, timeout: int) -> None:
        self.user_agent = user_agent
        self.timeout = timeout

    async def search(self, query: str, limit: int) -> List[SearchResultItem]:
        try:
            from ddgs import DDGS
        except ImportError as e:
            raise ImportError(
                "DuckDuckGo search requires 'ddgs' package. Install with: pip install ddgs"
            ) from e

        def _sync_search() -> List[dict]:
            with DDGS() as ddgs:
                return list(ddgs.text(query, max_results=limit))

        try:
            results = await asyncio.to_thread(_sync_search)
        except Exception as e:
            logger.error("duckduckgo.search.failed", error=str(e))
            return []

        items: List[SearchResultItem] = []
        for res in results:
            link = res.get("href", "")
            domain = urlparse(link).netloc
            items.append(
                SearchResultItem(
                    title=res.get("title", ""),
                    link=link,
                    snippet=res.get("body", ""),
                    published_date=res.get("published_date"),
                    source_domain=domain,
                    relevance_score=None,
                )
            )
        return items


# ------------------------------------------------------------------------
# SerpAPI Provider
# ------------------------------------------------------------------------
class SerpAPIProvider:
    """Search using SerpAPI (requires API key)."""

    def __init__(self, api_key: Optional[str], user_agent: str, timeout: int) -> None:
        if not api_key:
            raise ValueError("SerpAPI provider requires TENTACLE_SERPAPI_KEY to be set")
        self.api_key = api_key
        self.user_agent = user_agent
        self.timeout = timeout

    async def search(self, query: str, limit: int) -> List[SearchResultItem]:
        raise NotImplementedError("SerpAPI provider is not yet implemented")


# ------------------------------------------------------------------------
# Provider Registry (extensible without modifying SearchProxy)
# ------------------------------------------------------------------------
_PROVIDER_REGISTRY: Dict[str, Type] = {
    "duckduckgo": DuckDuckGoProvider,
    "serpapi": SerpAPIProvider,
}


# ------------------------------------------------------------------------
# SearchProxy (thin orchestrator with domain and filter support)
# ------------------------------------------------------------------------
class SearchProxy:
    """Search proxy that delegates to the configured provider."""

    def __init__(self) -> None:
        provider_name = settings.TENTACLE_SEARCH_PROVIDER.lower()
        provider_cls = _PROVIDER_REGISTRY.get(provider_name)
        if provider_cls is None:
            raise ValueError(
                f"Unknown search provider '{provider_name}'. "
                f"Supported: {', '.join(_PROVIDER_REGISTRY.keys())}"
            )

        if provider_name == "serpapi":
            self._provider = provider_cls(
                api_key=settings.TENTACLE_SERPAPI_KEY,
                user_agent=settings.TENTACLE_USER_AGENT,
                timeout=settings.TENTACLE_REQUEST_TIMEOUT,
            )
        else:
            self._provider = provider_cls(
                user_agent=settings.TENTACLE_USER_AGENT,
                timeout=settings.TENTACLE_REQUEST_TIMEOUT,
            )

        self._domains_dir = Path(settings.TENTACLE_DOMAINS_DIR) if hasattr(settings, 'TENTACLE_DOMAINS_DIR') else Path("domains")

        logger.debug(
            "search.proxy.initialized",
            provider=provider_name,
            user_agent=settings.TENTACLE_USER_AGENT,
        )

    async def search(
        self,
        query: str,
        limit: int = 5,
        filter: Optional[KeywordFilter] = None,
        domain_hint: Optional[str] = None,
        site_restrict: Optional[List[str]] = None,
    ) -> SearchResult:
        """
        Execute search with optional domain hint, site restrictions, and keyword filtering.

        Args:
            query: Base search query.
            limit: Maximum number of results.
            filter: Keyword filtering rules (include/exclude/boost).
            domain_hint: Name of domain configuration to load (e.g., 'trade').
            site_restrict: List of domains to restrict search to.
        """
        if filter is None:
            filter = KeywordFilter()

        # Load domain configuration if hint provided
        domain_config = self._load_domain_config(domain_hint) if domain_hint else {}

        # Build final query string
        final_query = self._build_query(
            base_query=query,
            filter=filter,
            domain_config=domain_config,
            site_restrict=site_restrict,
        )

        logger.info(
            "search.proxy.start",
            base_query=query,
            final_query=final_query,
            limit=limit,
            domain_hint=domain_hint,
            site_restrict=site_restrict,
        )

        try:
            items = await self._provider.search(final_query, limit)
        except Exception as e:
            logger.error("search.proxy.failed", error=str(e))
            items = []

        return SearchResult(query=query, items=items)

    def _build_query(
        self,
        base_query: str,
        filter: KeywordFilter,
        domain_config: Dict[str, Any],
        site_restrict: Optional[List[str]],
    ) -> str:
        """
        Construct the final query string by merging base query, include words,
        site restrictions, and domain defaults.
        """
        parts = [base_query]

        # Add include words (force them to appear)
        for word in filter.include:
            parts.append(word)

        # Add domain default keywords
        default_keywords = domain_config.get("default_keywords", [])
        for word in default_keywords:
            parts.append(word)

        # Add site restrictions
        sites = []
        if site_restrict:
            sites.extend(site_restrict)
        # Also add sites from domain config
        domain_sources = domain_config.get("search_sources", [])
        for src in domain_sources:
            if src.get("type") == "site":
                sites.append(src.get("value"))

        if sites:
            # Build OR expression for DuckDuckGo: (site:a OR site:b)
            site_clause = " OR ".join(f"site:{s}" for s in sites)
            parts.append(f"({site_clause})")

        # Add exclude terms (using DuckDuckGo syntax: -term)
        for word in filter.exclude:
            parts.append(f"-{word}")

        return " ".join(parts)

    def _load_domain_config(self, domain_hint: str) -> Dict[str, Any]:
        """
        Load domain configuration from YAML file.

        Looks for {domain_hint}.yaml in the domains directory.
        Returns empty dict if file not found.
        """
        config_path = self._domains_dir / f"{domain_hint}.yaml"
        if not config_path.exists():
            logger.warning("domain.config.not_found", hint=domain_hint, path=str(config_path))
            return {}

        try:
            with open(config_path, "r", encoding="utf-8") as f:
                return yaml.safe_load(f) or {}
        except Exception as e:
            logger.error("domain.config.load_failed", hint=domain_hint, error=str(e))
            return {}