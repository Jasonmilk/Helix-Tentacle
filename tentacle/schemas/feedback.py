from pydantic import BaseModel, Field, HttpUrl
from typing import List, Optional


class FeedbackRequest(BaseModel):
    """Evolution feedback from Anaphase after task completion."""
    trace_id: str
    scan_url: HttpUrl
    adopted_sections: List[str] = Field(default_factory=list)
    rejected_sections: List[str] = Field(default_factory=list)
    user_rating: Optional[int] = Field(None, ge=1, le=5)


class FeedbackResponse(BaseModel):
    status: str = "ok"
