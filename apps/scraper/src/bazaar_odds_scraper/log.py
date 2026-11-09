"""The scraper's console: one rich `Console` for the log, the progress bar and the tables, so the
log lines print above the bar instead of through it.

Messages use rich markup (`[saved]…[/]`, with the styles of `THEME`); pass any text that comes from
outside, such as an item's name, through `escape` so a `[` in it is not read as markup.
"""

from __future__ import annotations

import logging

from rich.console import Console
from rich.logging import RichHandler
from rich.markup import escape
from rich.theme import Theme

THEME = Theme({
    "saved": "bold green",
    "skipped": "yellow",
    "blocked": "bold red",
    "item": "bold cyan",
    "hero": "magenta",
    "tier": "blue",
    "muted": "dim",
    "path": "underline",
})

console = Console(theme=THEME, highlight=False)

__all__ = ["console", "escape", "setup"]


def setup(verbose: bool = False) -> None:
    """Send every log record to `console`, coloured by level, with the time on the left."""
    logging.basicConfig(
        level=logging.DEBUG if verbose else logging.INFO,
        format="%(message)s",
        datefmt="[%X]",
        handlers=[RichHandler(console=console, markup=True, rich_tracebacks=True, show_path=False)],
        force=True,
    )
    # asyncio and httpx log their own plumbing (httpx every request, at INFO); keep it out.
    for noisy in ("asyncio", "httpx", "httpcore"):
        logging.getLogger(noisy).setLevel(logging.WARNING)
