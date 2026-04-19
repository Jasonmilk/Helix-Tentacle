from bs4 import BeautifulSoup, Tag
from typing import List, Dict, Optional
import re
from cachetools import TTLCache

from tentacle.common.config import settings
from tentacle.common.logging import logger
from tentacle.schemas.forage import ScanResult, SectionTopography, KeywordFilter
from tentacle.schemas.exceptions import FetchError, ParseError
from .safe_fetcher import SafeFetcher


# Global cache for scan results
scan_cache: Optional[TTLCache] = None
if settings.TENTACLE_CACHE_ENABLED:
    scan_cache = TTLCache(
        maxsize=settings.TENTACLE_CACHE_MAX_SIZE,
        ttl=settings.TENTACLE_CACHE_TTL
    )


def get_dom_path(element: Tag) -> str:
    """Generate CSS selector path for an element."""
    path = []
    while element and element.name:
        selector = element.name
        if element.get('id'):
            selector += f"#{element.get('id')}"
        else:
            # Add nth-child if no id
            siblings = element.find_previous_siblings(element.name)
            if siblings:
                selector += f":nth-child({len(siblings) + 1})"
        path.insert(0, selector)
        element = element.parent
    return " > ".join(path)


def chunk_by_heading(soup: BeautifulSoup) -> List[Dict]:
    """Chunk document by H1-H6 headings."""
    headings = soup.find_all(re.compile('^h[1-6]$'))
    chunks = []

    if not headings:
        # No headings, treat whole body as one chunk
        body = soup.body
        if body:
            text = body.get_text(separator='\n', strip=True)
            chunks.append({
                'section_id': 'sec_001',
                'header': None,
                'element': body,
                'text': text,
                'dom_path': get_dom_path(body),
            })
        return chunks

    # Process each heading and its content
    for i, heading in enumerate(headings):
        # Get all siblings until next heading
        content = []
        sibling = heading.next_sibling
        while sibling and sibling.name not in [f'h{n}' for n in range(1, 7)]:
            if isinstance(sibling, Tag):
                content.append(sibling)
            sibling = sibling.next_sibling

        # Combine heading and content
        chunk_element = BeautifulSoup('', 'html.parser').new_tag('div')
        chunk_element.append(heading)
        for elem in content:
            chunk_element.append(elem)

        text = chunk_element.get_text(separator='\n', strip=True)
        header_text = heading.get_text(strip=True)

        chunks.append({
            'section_id': f'sec_{i+1:03d}',
            'header': header_text,
            'element': chunk_element,
            'text': text,
            'dom_path': get_dom_path(heading),
        })

    return chunks


class DOMScanner:
    """
    DOM Scanner for progressive information foraging.
    Performs low-resolution scan to generate document topography.
    """

    def __init__(self):
        self.fetcher = SafeFetcher()

    async def scan(
        self,
        url: str,
        keywords: List[str],
        strategy: str = "heading",
        filter: Optional[KeywordFilter] = None,
        cookie_file: Optional[str] = None,
    ) -> ScanResult:
        """
        Scan a URL and return document topography.

        Args:
            url: Target URL to scan.
            keywords: Base keywords for hit density calculation.
            strategy: Chunking strategy ("heading" | "semantic" | "adaptive").
            filter: Optional keyword filter with include/exclude/boost rules.
            cookie_file: Optional path to a Netscape format cookie file for authenticated sessions.
        """
        if filter is None:
            filter = KeywordFilter()

        # Build cache key including filter state
        cache_key = (
            url,
            tuple(keywords),
            strategy,
            tuple(sorted(filter.include)),
            tuple(sorted(filter.exclude)),
            tuple(sorted(filter.boost.items())),
            cookie_file,  # Include in cache key as content may differ
        )
        if scan_cache is not None and cache_key in scan_cache:
            logger.debug("Cache hit for scan", url=url)
            return scan_cache[cache_key]

        logger.info(
            "Starting scan",
            url=url,
            strategy=strategy,
            keywords=keywords,
            filter_include=filter.include,
            filter_exclude=filter.exclude,
            cookie_file=cookie_file,
        )

        # Fetch HTML with optional cookie file
        html = await self.fetcher.fetch(url, cookie_file=cookie_file)

        # Parse and clean HTML
        try:
            soup = BeautifulSoup(html, 'lxml')

            # Remove unwanted elements
            for unwanted in soup(['script', 'style', 'nav', 'footer', 'aside', 'noscript']):
                unwanted.decompose()

            # Get page title
            title = soup.title.string.strip() if soup.title else "Unknown"

            # Chunk document
            if strategy == "heading":
                chunks = chunk_by_heading(soup)
            elif strategy == "semantic":
                # TODO: Implement semantic chunking (optional feature)
                chunks = chunk_by_heading(soup)
            elif strategy == "adaptive":
                # TODO: Implement adaptive chunking (experimental)
                chunks = chunk_by_heading(soup)
            else:
                chunks = chunk_by_heading(soup)

            # Prepare keyword sets for fast lookup
            keywords_lower = [k.lower() for k in keywords]
            include_lower = [w.lower() for w in filter.include]
            exclude_lower = [w.lower() for w in filter.exclude]
            boost_lower = {w.lower(): weight for w, weight in filter.boost.items()}

            total_words = 0
            topography = []

            for i, chunk in enumerate(chunks):
                text = chunk['text']
                text_lower = text.lower()
                words = text_lower.split()
                word_count = len(words)
                total_words += word_count

                # Compute hit density with boost and exclude logic
                hit_density = self._compute_hit_density(
                    text_lower=text_lower,
                    word_count=word_count,
                    base_keywords=keywords_lower,
                    include_words=include_lower,
                    exclude_words=exclude_lower,
                    boost_weights=boost_lower,
                )

                # Position weight: earlier sections have higher weight
                position_weight = 1.0 - (i / max(len(chunks), 1)) * 0.3

                # Content quality score (experimental)
                quality_score = self._compute_quality_score(chunk['element'], text)

                topography.append(
                    SectionTopography(
                        section_id=chunk['section_id'],
                        header=chunk['header'],
                        word_count=word_count,
                        hit_density=hit_density,
                        position_weight=position_weight,
                        dom_path=chunk['dom_path'],
                        quality_score=quality_score,
                    )
                )

            result = ScanResult(
                url=url,
                title=title,
                total_words=total_words,
                topography=topography,
            )

            # Cache the result
            if scan_cache is not None:
                scan_cache[cache_key] = result

            logger.info(
                "Scan completed",
                url=url,
                sections=len(topography),
                max_density=max(t.hit_density for t in topography) if topography else 0,
            )

            return result

        except Exception as e:
            logger.error("Parse error", url=url, error=str(e))
            raise ParseError(f"Failed to parse document: {str(e)}")

    def _compute_hit_density(
        self,
        text_lower: str,
        word_count: int,
        base_keywords: List[str],
        include_words: List[str],
        exclude_words: List[str],
        boost_weights: Dict[str, float],
    ) -> float:
        """
        Compute normalized hit density with boost weights and exclusion logic.

        Returns 0.0 if any exclude word is present or if word_count == 0.
        """
        if word_count == 0:
            return 0.0

        # If any exclude word is present, the whole section is invalid
        for ex in exclude_words:
            if ex in text_lower:
                return 0.0

        # If include words are required but not present, return 0.0
        # Note: mode='any' is assumed (at least one include word present)
        if include_words:
            if not any(inc in text_lower for inc in include_words):
                return 0.0

        # Base hits from keywords
        base_hits = sum(text_lower.count(kw) for kw in base_keywords)

        # Boost hits (additional weighted occurrences)
        boost_hits = 0.0
        for boost_word, weight in boost_weights.items():
            count = text_lower.count(boost_word)
            boost_hits += count * (weight - 1.0)  # extra hits beyond 1.0

        total_hits = base_hits + boost_hits
        return total_hits / word_count

    def _compute_quality_score(self, element: Tag, text: str) -> float:
        """
        Estimate content quality based on text density, structural importance,
        and tag diversity.
        """
        # Text purity: ratio of visible text to total text in element
        total_text = element.get_text()
        total_len = len(total_text)
        if total_len == 0:
            return 0.0
        content_score = min(1.0, len(text) / total_len)

        # Position bonus if inside article/main
        position_score = 0.3 if element.find_parent(['article', 'main']) else 0.0

        # Diversity: count distinct tag names within the chunk
        distinct_tags = len({t.name for t in element.find_all()})
        diversity_score = min(0.5, 0.1 * distinct_tags)

        return content_score * (1 + position_score) * (1 + diversity_score)