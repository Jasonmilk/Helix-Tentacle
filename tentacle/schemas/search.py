"""Contracts for Search Engine Proxy."""

from pydantic import BaseModel, Field
from typing import List, Optional

from .forage import KeywordFilter


class SearchRequest(BaseModel):
    """Search request with optional domain hint, site restrictions, and keyword filtering."""
    query: str
    limit: int = Field(5, ge=1, le=20)
    filter: KeywordFilter = Field(
        default_factory=KeywordFilter,
        description="Keyword filtering and boosting rules applied to search and subsequent scans"
    )
    domain_hint: Optional[str] = Field(
        None,
        description="Name of a domain configuration to load (e.g., 'trade', 'academic')"
    )
    site_restrict: List[str] = Field(
        default_factory=list,
        description="List of domains to restrict search to (e.g., ['customs.gov.cn'])"
    )
    trace_id: str


class SearchResultItem(BaseModel):
    title: str
    link: str
    snippet: str
    published_date: Optional[str] = None
    source_domain: str
    relevance_score: Optional[float] = None


class SearchResult(BaseModel):
    query: str
    items: List[SearchResultItem]