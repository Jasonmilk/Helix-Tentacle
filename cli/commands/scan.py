"""Scan command for Tentacle CLI."""

import asyncio
import json
from typing import List, Optional

import typer
from rich.console import Console
from rich.table import Table
from tabulate import tabulate

from tentacle.common.logging import logger
from tentacle.common.tracing import generate_trace_id, set_trace_id
from tentacle.core import DOMScanner
from tentacle.schemas.forage import ScanResult, KeywordFilter
from .shared import parse_comma_list, parse_boost_dict

app = typer.Typer(help="Scan and map web pages")
console = Console()


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


@app.command(name="scan")
def scan_command(
    url: str = typer.Argument(..., help="URL to scan"),
    keywords: Optional[str] = typer.Option(
        None, "--keywords", "-k", help="Comma-separated keywords to score"
    ),
    strategy: str = typer.Option(
        "heading", "--strategy", "-s", help="Chunking strategy: heading|semantic|adaptive"
    ),
    format: str = typer.Option(
        OutputFormat.TABLE,
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
    cookie: Optional[str] = typer.Option(
        None,
        "--cookie",
        help="Cookie file name or path for authenticated sessions (Netscape format)",
    ),
) -> None:
    """
    Phase 1: Scan a URL and return document topography.

    Calculates keyword hit density across semantic chunks, returning a
    lightweight "terrain map" for progressive disclosure.
    """
    # --------------------------------------------------------------------
    # 1. Trace ID initialization (required for audit)
    # --------------------------------------------------------------------
    trace_id = generate_trace_id()
    set_trace_id(trace_id)
    logger.info(
        "scan.command.start",
        url=url,
        keywords=keywords,
        strategy=strategy,
        filter_level=filter_level,
        require=require,
        exclude=exclude,
        boost=boost,
        cookie=cookie,
    )

    # Parse base keywords
    keyword_list: List[str] = []
    if keywords:
        keyword_list = [k.strip() for k in keywords.split(",") if k.strip()]

    # Build KeywordFilter from CLI options
    kw_filter = KeywordFilter(
        include=parse_comma_list(require),
        exclude=parse_comma_list(exclude),
        boost=parse_boost_dict(boost),
    )

    # --------------------------------------------------------------------
    # 2. Execute async scan in sync wrapper (avoid unawaited coroutine)
    # --------------------------------------------------------------------
    async def _run_scan() -> ScanResult:
        scanner = DOMScanner()
        return await scanner.scan(
            url, keyword_list, strategy, filter=kw_filter, cookie_file=cookie
        )

    try:
        result = asyncio.run(_run_scan())
    except Exception as e:
        logger.error("scan.command.failed", error=str(e))
        typer.echo(f"Error: {str(e)}", err=True)
        raise typer.Exit(1) from e

    # --------------------------------------------------------------------
    # 3. Apply filter level post-processing
    # --------------------------------------------------------------------
    if filter_level != FilterLevel.NONE:
        filtered_topography = []
        for section in result.topography:
            # Standard: filter out low-quality sections
            if filter_level == FilterLevel.STANDARD:
                if section.quality_score is not None and section.quality_score < 0.3:
                    continue
            # Strict: additionally require positive hit density
            elif filter_level == FilterLevel.STRICT:
                if section.quality_score is not None and section.quality_score < 0.3:
                    continue
                if section.hit_density <= 0.0:
                    continue
            filtered_topography.append(section)
        # Create a new ScanResult with filtered topography
        result = ScanResult(
            url=result.url,
            title=result.title,
            total_words=result.total_words,
            topography=filtered_topography,
            topography_tree=result.topography_tree,
        )

    # --------------------------------------------------------------------
    # 4. Format output based on user selection
    # --------------------------------------------------------------------
    if format == OutputFormat.JSON:
        typer.echo(json.dumps(result.model_dump(), indent=2, ensure_ascii=False))

    elif format == OutputFormat.TABLE:
        rows = []
        for section in result.topography:
            row = [
                section.section_id,
                section.header or "-",
                section.word_count,
                f"{section.hit_density:.4f}",
                f"{section.position_weight:.2f}",
            ]
            if section.quality_score is not None:
                row.append(f"{section.quality_score:.2f}")
            rows.append(row)

        headers = ["Section ID", "Header", "Words", "Hit Density", "Position Weight"]
        if result.topography and result.topography[0].quality_score is not None:
            headers.append("Quality")
        typer.echo(tabulate(rows, headers=headers, tablefmt="grid"))
        typer.echo(f"\nTitle: {result.title}")
        typer.echo(f"Total words: {result.total_words}")

    elif format == OutputFormat.RICH:
        table = Table(title=f"Scan Result: {result.title}")
        table.add_column("Section ID", style="cyan")
        table.add_column("Header", style="white")
        table.add_column("Words", justify="right", style="green")
        table.add_column("Hit Density", justify="right", style="yellow")
        table.add_column("Position Weight", justify="right", style="blue")
        if result.topography and result.topography[0].quality_score is not None:
            table.add_column("Quality", justify="right", style="magenta")

        for section in result.topography:
            row_values = [
                section.section_id,
                section.header or "-",
                str(section.word_count),
                f"{section.hit_density:.4f}",
                f"{section.position_weight:.2f}",
            ]
            if section.quality_score is not None:
                row_values.append(f"{section.quality_score:.2f}")
            table.add_row(*row_values)

        console.print(table)
        console.print(f"Total words: {result.total_words}")

    else:
        typer.echo(f"Unknown format: {format}", err=True)
        raise typer.Exit(1)

    logger.info("scan.command.complete", sections_found=len(result.topography))