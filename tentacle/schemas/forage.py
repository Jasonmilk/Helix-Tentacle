"""Contracts for Progressive Foraging (Scan & Extract)."""

from pydantic import BaseModel, Field, HttpUrl
from typing import List, Optional, Dict, Literal


class KeywordFilter(BaseModel):
    """
    Keyword filtering strategy that applies throughout the foraging pipeline.
    Used by both Scan and Search phases to refine hit density and result ranking.
    """
    include: List[str] = Field(
        default_factory=list,
        description="Words that must be present in the section"
    )
    exclude: List[str] = Field(
        default_factory=list,
        description="Words that force hit density to zero if present"
    )
    boost: Dict[str, float] = Field(
        default_factory=dict,
        description="Words with multiplier weight (e.g., {'urgent': 2.0})"
    )
    mode: Literal["any", "all"] = Field(
        "any",
        description="Whether all include words must be present or any"
    )


class ScanRequest(BaseModel):
    """Phase 1: Scan request from Anaphase."""
    url: HttpUrl
    keywords: List[str] = Field(default_factory=list)
    filter: KeywordFilter = Field(
        default_factory=KeywordFilter,
        description="Keyword filtering and boosting rules"
    )
    trace_id: str = Field(..., description="W3C Trace Context compatible 32-char hex")
    strategy: Optional[str] = Field("heading", description="heading | semantic | adaptive")


class SectionTopography(BaseModel):
    """Lightweight chunk metadata."""
    section_id: str
    header: Optional[str]
    word_count: int
    hit_density: float  # normalized keyword hit density (boosted and filtered)
    position_weight: float  # position importance (e.g., early sections)
    dom_path: str  # CSS selector path to the chunk
    quality_score: Optional[float] = Field(
        None,
        description="Content quality score used for filtering (higher is better)"
    )


class ScanResult(BaseModel):
    url: str
    title: str
    total_words: int
    topography: List[SectionTopography]
    topography_tree: Optional[Dict] = None  # experimental hierarchical view


class ExtractRequest(BaseModel):
    """Phase 2: Extract raw text for specific sections."""
    url: HttpUrl
    section_ids: List[str]
    trace_id: str


class ExtractResult(BaseModel):
    url: str
    snippets: Dict[str, str]  # section_id → raw text
    truncated: Dict[str, bool]  # whether truncated due to size limit