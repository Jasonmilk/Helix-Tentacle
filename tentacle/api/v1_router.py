from fastapi import APIRouter, HTTPException

from tentacle.schemas import (
    ScanRequest, ScanResult,
    ExtractRequest, ExtractResult,
    SearchRequest, SearchResult,
    FeedbackRequest, FeedbackResponse,
    FetchError, ParseError, SecurityViolationError,
    KeywordFilter,
)
from tentacle.core import DOMScanner, SnippetExtractor, SearchProxy
from tentacle.common.logging import logger

router = APIRouter(prefix="/v1/tentacle", tags=["tentacle"])


@router.post("/scan", response_model=ScanResult)
async def scan_url(request: ScanRequest):
    try:
        scanner = DOMScanner()
        return await scanner.scan(
            url=str(request.url),
            keywords=request.keywords,
            strategy=request.strategy,
            filter=request.filter if hasattr(request, 'filter') else KeywordFilter(),
        )
    except SecurityViolationError as e:
        logger.warning("Security violation", error=str(e))
        raise HTTPException(status_code=403, detail=str(e))
    except (FetchError, ParseError) as e:
        logger.error("Scan error", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))
    except Exception as e:
        logger.error("Unexpected scan error", error=str(e))
        raise HTTPException(status_code=500, detail="Internal server error")


@router.post("/extract", response_model=ExtractResult)
async def extract_sections(request: ExtractRequest):
    try:
        extractor = SnippetExtractor()
        return await extractor.extract(
            url=str(request.url),
            section_ids=request.section_ids
        )
    except SecurityViolationError as e:
        logger.warning("Security violation", error=str(e))
        raise HTTPException(status_code=403, detail=str(e))
    except (FetchError, ParseError) as e:
        logger.error("Extract error", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))
    except Exception as e:
        logger.error("Unexpected extract error", error=str(e))
        raise HTTPException(status_code=500, detail="Internal server error")


@router.post("/search", response_model=SearchResult)
async def search_web(request: SearchRequest):
    try:
        proxy = SearchProxy()
        return await proxy.search(
            query=request.query,
            limit=request.limit,
            filter=request.filter if hasattr(request, 'filter') else KeywordFilter(),
            domain_hint=request.domain_hint if hasattr(request, 'domain_hint') else None,
            site_restrict=request.site_restrict if hasattr(request, 'site_restrict') else [],
        )
    except Exception as e:
        logger.error("Search error", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/feedback", response_model=FeedbackResponse)
async def submit_feedback(request: FeedbackRequest):
    try:
        # Log feedback for future evolution
        logger.info(
            "Received feedback",
            scan_url=str(request.scan_url),
            adopted=request.adopted_sections,
            rejected=request.rejected_sections,
            rating=request.user_rating
        )
        # TODO: Store feedback in database for offline training
        return FeedbackResponse(status="ok")
    except Exception as e:
        logger.error("Feedback error", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))