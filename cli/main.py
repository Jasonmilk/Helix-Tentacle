import typer
from tentacle.common.config import settings
from .commands.scan import app as scan_app
from .commands.extract import app as extract_app
from .commands.search import app as search_app
from .commands.explore import app as explore_app

# Check mode before enabling CLI
if settings.TENTACLE_MODE != "standalone":
    raise RuntimeError(
        "CLI is only available in standalone mode. "
        "Set TENTACLE_MODE=standalone to enable it."
    )

app = typer.Typer(
    name="tentacle",
    help="Helix-Tentacle CLI: External perception and progressive information sniffing tool",
    add_completion=False,
)

# Add subcommands
app.add_typer(scan_app)
app.add_typer(extract_app)
app.add_typer(search_app)
app.add_typer(explore_app)


def main():
    app()


if __name__ == "__main__":
    main()
