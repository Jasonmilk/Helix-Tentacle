"""Extract command for Tentacle CLI."""

import asyncio
import json
from typing import List

import typer
from rich.console import Console

from tentacle.common.logging import logger
from tentacle.common.tracing import generate_trace_id, set_trace_id
from tentacle.core import SnippetExtractor
from tentacle.schemas.forage import ExtractResult

app = typer.Typer(help="Extract raw text from specific sections")
console = Console()


class OutputFormat:
    """Format constants — avoid magic strings."""
    JSON = "json"
    TEXT = "text"
    CHOICES = [JSON, TEXT]


@app.command(name="extract")
def extract_command(
    url: str = typer.Argument(..., help="URL to extract from"),
    sections: str = typer.Option(
        ..., "--sections", "-s", help="Comma-separated section IDs to extract"
    ),
    format: str = typer.Option(
        OutputFormat.TEXT,
        "--format",
        "-f",
        help=f"Output format: {'|'.join(OutputFormat.CHOICES)}",
    ),
) -> None:
    """
    Phase 2: Extract raw text snippets for specific section IDs.

    Use after `tentacle scan` to retrieve the actual content of high-value chunks.
    """
    # --------------------------------------------------------------------
    # 1. Trace ID initialization (mandatory for audit)
    # --------------------------------------------------------------------
    trace_id = generate_trace_id()
    set_trace_id(trace_id)
    logger.info("extract.command.start", url=url, sections=sections)

    # Parse section IDs
    section_ids: List[str] = [s.strip() for s in sections.split(",") if s.strip()]

    # --------------------------------------------------------------------
    # 2. Execute async extraction in sync wrapper (avoid unawaited coroutine)
    # --------------------------------------------------------------------
    async def _run_extract() -> ExtractResult:
        extractor = SnippetExtractor()
        return await extractor.extract(url, section_ids)

    try:
        result = asyncio.run(_run_extract())
    except Exception as e:
        logger.error("extract.command.failed", error=str(e))
        typer.echo(f"Error: {str(e)}", err=True)
        raise typer.Exit(1) from e

    # --------------------------------------------------------------------
    # 3. Format output based on user selection
    # --------------------------------------------------------------------
    if format == OutputFormat.JSON:
        typer.echo(json.dumps(result.model_dump(), indent=2, ensure_ascii=False))

    elif format == OutputFormat.TEXT:
        for section_id, snippet in result.snippets.items():
            if snippet:
                console.print(f"\n[bold cyan]--- {section_id} ---[/bold cyan]")
                console.print(snippet)
            else:
                console.print(
                    f"\n[bold yellow]--- {section_id} (not found or empty) ---[/bold yellow]"
                )
        # Warn about truncations
        for section_id, truncated in result.truncated.items():
            if truncated:
                console.print(
                    f"[yellow]Warning: {section_id} was truncated (max size exceeded)[/yellow]"
                )

    else:
        typer.echo(f"Unknown format: {format}", err=True)
        raise typer.Exit(1)

    logger.info("extract.command.complete", sections_extracted=len(result.snippets))