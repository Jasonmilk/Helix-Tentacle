import typer
from rich.prompt import Prompt, Confirm
from rich.console import Console
from tentacle.core import DOMScanner, SnippetExtractor
from tentacle.common import generate_trace_id, logger

app = typer.Typer(help="Interactive exploration mode")
console = Console()


@app.command(name="explore")
async def explore_command(
    url: str = typer.Argument(..., help="URL to explore"),
):
    """Interactive mode to explore document sections and extract content."""
    trace_id = generate_trace_id()
    logger.bind(trace_id=trace_id)

    console.print(f"[bold cyan]Exploring: {url}[/bold cyan]")
    console.print("Scanning document...")

    try:
        # First scan the document
        scanner = DOMScanner()
        scan_result = await scanner.scan(url, [])

        console.print(f"\nFound {len(scan_result.topography)} sections:")
        for i, section in enumerate(scan_result.topography):
            header = section.header or "Untitled"
            console.print(f"  {i+1}. [{section.section_id}] {header} ({section.word_count} words)")

        # Interactive loop
        while True:
            choice = Prompt.ask(
                "\nEnter section number to extract, or 'q' to quit",
                default="q"
            )

            if choice.lower() == "q":
                break

            try:
                idx = int(choice) - 1
                if idx < 0 or idx >= len(scan_result.topography):
                    console.print("[red]Invalid section number[/red]")
                    continue

                section = scan_result.topography[idx]
                console.print(f"\nExtracting section: {section.header or 'Untitled'}...")

                extractor = SnippetExtractor()
                extract_result = await extractor.extract(url, [section.section_id])

                text = extract_result.snippets[section.section_id]
                console.print("\n[bold]Content:[/bold]")
                console.print(text)
                if extract_result.truncated[section.section_id]:
                    console.print("[yellow]... (truncated)[/yellow]")

            except ValueError:
                console.print("[red]Invalid input, please enter a number[/red]")
            except Exception as e:
                console.print(f"[red]Error: {str(e)}[/red]")

    except Exception as e:
        logger.error("Explore failed", error=str(e))
        typer.echo(f"Error: {str(e)}", err=True)
        raise typer.Exit(1)
