"""Session policy contracts for Agent-programmable perception."""

from pydantic import BaseModel, Field
from typing import Optional, Literal

from .forage import KeywordFilter


class SessionPolicy(BaseModel):
    """
    Reusable policy bound to a session (trace_id or epoch_id).
    Defines perception behavior for all subsequent Tentacle calls.
    """
    domain_hint: Optional[str] = Field(
        None,
        description="Name of domain configuration to apply (e.g., 'trade', 'legal')"
    )
    keyword_filter: KeywordFilter = Field(
        default_factory=KeywordFilter,
        description="Keyword inclusion/exclusion/boosting rules"
    )
    filter_level: Literal["none", "standard", "strict"] = Field(
        "standard",
        description="Content quality filtering aggressiveness"
    )
    cookie_ref: Optional[str] = Field(
        None,
        description="Reference to a saved cookie/session file for authenticated access"
    )