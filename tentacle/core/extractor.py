from bs4 import BeautifulSoup
from typing import List, Dict
from tentacle.common.config import settings
from tentacle.common.logging import logger
from tentacle.schemas.forage import ExtractResult
from tentacle.schemas.exceptions import ParseError
from .safe_fetcher import SafeFetcher
from .scanner import chunk_by_heading


class SnippetExtractor:
    """
    Extract raw text snippets from specific document sections.
    Reuses the same chunking logic as DOMScanner for ID consistency.
    """
    def __init__(self):
        self.fetcher = SafeFetcher()

    async def extract(self, url: str, section_ids: List[str]) -> ExtractResult:
        """
        Extract raw text for the specified section IDs.
        """
        logger.info("Extracting sections", url=url, sections=section_ids)

        # Fetch and parse HTML
        html = await self.fetcher.fetch(url)
        
        try:
            soup = BeautifulSoup(html, 'lxml')
            
            # Remove unwanted elements (same as scanner)
            for unwanted in soup(['script', 'style', 'nav', 'footer', 'aside', 'noscript']):
                unwanted.decompose()

            # Chunk using the exact same logic as scanner
            chunks = chunk_by_heading(soup)
            chunk_map = {c['section_id']: c for c in chunks}

            snippets: Dict[str, str] = {}
            truncated: Dict[str, bool] = {}

            for section_id in section_ids:
                chunk = chunk_map.get(section_id)
                if not chunk:
                    snippets[section_id] = ""
                    truncated[section_id] = False
                    continue

                text = chunk['text']
                # Check size limit
                max_size = settings.TENTACLE_MAX_SNIPPET_SIZE
                if len(text) > max_size:
                    text = text[:max_size] + "..."
                    truncated[section_id] = True
                else:
                    truncated[section_id] = False

                snippets[section_id] = text

            result = ExtractResult(
                url=url,
                snippets=snippets,
                truncated=truncated
            )

            logger.info(
                "Extraction completed", 
                url=url, 
                extracted=len(snippets),
                truncated_count=sum(truncated.values())
            )

            return result

        except Exception as e:
            logger.error("Extract error", url=url, error=str(e))
            raise ParseError(f"Failed to extract sections: {str(e)}")
