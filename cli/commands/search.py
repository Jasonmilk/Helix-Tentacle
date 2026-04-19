"""Search command for Tentacle CLI."""

import asyncio
import json
from typing import Dict, List, Optional

import typer
from rich.console import Console
from rich.table import Table
from tabulate import tabulate

from tentacle.common.config import settings
from tentacle.common.logging import logger
from tentacle.common.tracing import generate_trace_id, set_trace_id
from tentacle.core import DOMScanner, SearchProxy
from tentacle.schemas.forage import ScanResult, KeywordFilter
from tentacle.schemas.search import SearchResult
from .shared import parse_comma_list, parse_boost_dict

app = typer.Typer(help="Search web pages")
console = Console()


# ------------------------------------------------------------------------
# Format constants (avoid magic strings)
# ------------------------------------------------------------------------
class OutputFormat:
    JSON = "json"
    TABLE = "table"
    RICH = "rich"
    CHOICES = [JSON, TABLE, RICH]


class FilterLevel:
    NONE = "none"
    STANDARD = "standard"
    STRICT = "strict"
    CHOICES = [NONE, STANDARD, STRICT]
    DEFAULT = STANDARD


@app.command(name="search")
def search_command(
    query: str = typer.Argument(..., help="Search query"),
    limit: int = typer.Option(
        5, "--limit", "-l", help="Maximum number of results", min=1, max=20
    ),
    scan: bool = typer.Option(
        False, "--scan", help="Automatically scan all result URLs"
    ),
    format: str = typer.Option(
        OutputFormat.JSON,
        "--format",
        "-f",
        help=f"Output format: {'|'.join(OutputFormat.CHOICES)}",
    ),
    require: Optional[str] = typer.Option(
        None, "--require", help="Comma-separated words that must be present"
    ),
    exclude: Optional[str] = typer.Option(
        None, "--exclude", help="Comma-separated words that invalidate the section"
    ),
    boost: Optional[str] = typer.Option(
        None, "--boost", help="Boost words with weights: word:2.0,word2:1.5"
    ),
    filter_level: str = typer.Option(
        FilterLevel.DEFAULT,
        "--filter-level",
        help=f"Content filtering aggressiveness: {'|'.join(FilterLevel.CHOICES)}",
    ),
    domain: Optional[str] = typer.Option(
        None, "--domain", "-d", help="Domain configuration name to load (e.g., 'trade')"
    ),
    site: Optional[str] = typer.Option(
        None, "--site", help="Comma-separated domains to restrict search to"
    ),
    cookie: Optional[str] = typer.Option(
        None,
        "--cookie",
        help="Cookie file name or path for authenticated sessions (Netscape format)",
    ),
) -> None:
    """Search the web and return results.

    This command wraps Tentacle's SearchProxy and optionally scans each
    result URL using the progressive foraging protocol.
    """
    # --------------------------------------------------------------------
    # 1. Trace ID initialization (required for audit)
    # --------------------------------------------------------------------
    trace_id = generate_trace_id()
    set_trace_id(trace_id)
    logger.info(
        "search.command.start",
        query=query,
        limit=limit,
        scan=scan,
        filter_level=filter_level,
        domain=domain,
        site=site,
        cookie=cookie,
    )

    # Build KeywordFilter from CLI options
    kw_filter = KeywordFilter(
        include=parse_comma_list(require),
        exclude=parse_comma_list(exclude),
        boost=parse_boost_dict(boost),
    )

    # Parse site restrictions
    site_list = parse_comma_list(site)

    # --------------------------------------------------------------------
    # 2. Execute async search in sync wrapper (avoid unawaited coroutine)
    # --------------------------------------------------------------------
    async def _run_search() -> tuple[SearchResult, Optional[Dict[str, Optional[ScanResult]]]]:
        proxy = SearchProxy()
        search_result = await proxy.search(
            query=query,
            limit=limit,
            filter=kw_filter,
            domain_hint=domain,
            site_restrict=site_list,
        )

        scan_results: Dict[str, Optional[ScanResult]] = {}
        if scan:
            scanner = DOMScanner()
            # Extract base keywords from query for scanning
            base_keywords = _parse_keywords(query)
            for item in search_result.items:
                try:
                    scan_res = await scanner.scan(
                        str(item.link),
                        keywords=base_keywords,
                        filter=kw_filter,
                        cookie_file=cookie,  # Pass cookie file path
                    )
                    # Apply filter level post-processing
                    scan_res = _apply_filter_level(scan_res, filter_level)
                    scan_results[str(item.link)] = scan_res
                except Exception as exc:
                    logger.warning(
                        "search.scan.failed",
                        url=str(item.link),
                        error=str(exc),
                    )
                    scan_results[str(item.link)] = None
        return search_result, scan_results if scan else None

    try:
        search_result, scan_results = asyncio.run(_run_search())
    except Exception as e:
        logger.error("search.command.failed", error=str(e))
        typer.echo(f"Error: {str(e)}", err=True)
        raise typer.Exit(1) from e

    # If search returned no results, exit gracefully
    if not search_result.items:
        typer.echo("No results found.", err=True)
        raise typer.Exit(0)

    # --------------------------------------------------------------------
    # 3. Format output based on user selection
    # --------------------------------------------------------------------
    if format == OutputFormat.JSON:
        output_data = search_result.model_dump()
        if scan_results is not None:
            output_data["scan_results"] = {
                url: res.model_dump() if res else None
                for url, res in scan_results.items()
            }
        typer.echo(json.dumps(output_data, indent=2, ensure_ascii=False))

    elif format == OutputFormat.TABLE:
        rows = []
        for item in search_result.items:
            rows.append([
                _truncate(item.title, 50),
                item.source_domain,
                _truncate(item.snippet, 100),
            ])
        headers = ["Title", "Domain", "Snippet"]
        typer.echo(tabulate(rows, headers=headers, tablefmt="grid"))

    elif format == OutputFormat.RICH:
        table = Table(title=f"Search Results: {query}")
        table.add_column("Title", style="cyan", width=40)
        table.add_column("Domain", style="green", width=20)
        table.add_column("Snippet", style="white", width=60)

        for item in search_result.items:
            table.add_row(
                _truncate(item.title, 40),
                item.source_domain,
                _truncate(item.snippet, 60),
            )
        console.print(table)

    else:
        typer.echo(f"Unknown format: {format}", err=True)
        raise typer.Exit(1)

    logger.info("search.command.complete", result_count=len(search_result.items))


def _apply_filter_level(result: ScanResult, filter_level: str) -> ScanResult:
    """Post-process scan result by filtering sections based on quality and hit density."""
    if filter_level == FilterLevel.NONE:
        return result

    filtered = []
    for section in result.topography:
        if filter_level == FilterLevel.STANDARD:
            if section.quality_score is not None and section.quality_score < 0.3:
                continue
        elif filter_level == FilterLevel.STRICT:
            if section.quality_score is not None and section.quality_score < 0.3:
                continue
            if section.hit_density <= 0.0:
                continue
        filtered.append(section)

    return ScanResult(
        url=result.url,
        title=result.title,
        total_words=result.total_words,
        topography=filtered,
        topography_tree=result.topography_tree,
    )


def _parse_keywords(query: str) -> List[str]:
    """
    Parse search query into keyword list for hit density calculation.

    Splits by whitespace and filters empty tokens. Future enhancement could
    integrate with jieba for Chinese word segmentation.
    """
    if not query:
        return []
    return [k.strip() for k in query.split() if k.strip()]


def _truncate(text: str, max_len: int) -> str:
    """Truncate text with ellipsis if needed."""
    if len(text) <= max_len:
        return text
    return text[: max_len - 3] + "..."