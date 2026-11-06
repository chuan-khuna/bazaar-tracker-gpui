"""The interactive scrape: a few questions (questionary) for what changes from run to run (the
characters, whether to redo what is saved or skipped, the retries), then a look at the queue before
anything is asked of Bazaar DB. The browser is picked without asking, and every owned tier is
scraped; the other options come from the flags.

Whatever flags were passed become the answers' defaults, so `scrape-odds -i --hero Jules` starts
with Jules ticked. Ctrl+C or Esc on any question stops without asking for anything.
"""

from __future__ import annotations

import argparse
import json
import logging
import urllib.request

import questionary
from questionary import Choice, Style

from . import catalog

TIERS = ["None", "Bronze", "Silver", "Gold", "Diamond"]
DEFAULT_CDP = "http://localhost:9222"

log = logging.getLogger("scrape-odds")

STYLE = Style([
    ("qmark", "fg:#5fafff bold"),
    ("question", "bold"),
    ("answer", "fg:#5fd75f bold"),
    ("pointer", "fg:#5fafff bold"),
    ("highlighted", "fg:#5fafff bold"),
    ("selected", "fg:#5fd75f"),
    ("instruction", "fg:#808080 italic"),
])

START, SHOW, CANCEL = "start", "show", "cancel"


class Cancelled(Exception):
    """The person stopped the questions."""


def _answer(question: questionary.Question):
    answer = question.ask()  # None on Ctrl+C or Esc
    if answer is None:
        raise Cancelled
    return answer


def running_browser(url: str = DEFAULT_CDP) -> str | None:
    """What answers on the debugging port at `url` (such as `Chrome/157.0.8103.39`), if anything."""
    try:
        with urllib.request.urlopen(f"{url.rstrip('/')}/json/version", timeout=1) as reply:
            return json.loads(reply.read()).get("Browser") or "a browser"
    except (OSError, ValueError):
        return None


def _browser(args: argparse.Namespace) -> None:
    """Attach to the browser on the debugging port when one answers there, else launch Chrome."""
    url = args.cdp or DEFAULT_CDP
    found = running_browser(url)
    if found:
        args.cdp = url
        log.info(f"browser: attaching to [path]{url}[/] ({found}), with its own profile")
    else:
        args.cdp = None
        log.info(f"browser: nothing answers on {url}, so launching {args.browser} with the scraper's profile "
                 "(start Dia or Chrome with --remote-debugging-port=9222 to use it instead)")


def _heroes(args: argparse.Namespace, items: list[catalog.Item]) -> None:
    picked = set(args.heroes or [])
    choices = [
        Choice(f"{hero}  ({len(catalog.for_hero(items, hero, not args.no_neutral))} items)", hero, checked=hero in picked)
        for hero in catalog.heroes(items)
    ]
    args.heroes = _answer(questionary.checkbox(
        "Which characters?", choices=choices, style=STYLE,
        instruction="(space to tick, a for all, enter when done)",
        validate=lambda chosen: bool(chosen) or "tick at least one character",
    ))


def ask(args: argparse.Namespace, items: list[catalog.Item], skipped: int = 0) -> None:
    """Fill in `args` from the person's answers; raises `Cancelled` if they stop. Only what changes
    from run to run is asked; every owned tier is scraped unless `--tier` was passed."""
    _browser(args)
    _heroes(args, items)
    args.refresh = not _answer(questionary.confirm(
        "Only items not scraped yet?", default=not args.refresh, style=STYLE))
    if not args.refresh and skipped:
        args.retry_skipped = _answer(questionary.confirm(
            f"Scrape the skipped list too ({skipped} in odds/skipped.json)?", default=args.retry_skipped, style=STYLE))
    retries = _answer(questionary.text(
        "Retries per failed item:", default=str(args.retries), style=STYLE,
        validate=lambda text: text.strip().isdigit() or "a whole number, 0 or more",
    ))
    args.retries = int(retries)


def next_step(count: int) -> str:
    """After the queue is shown: START, SHOW (print every ask) or CANCEL."""
    if not count:
        return CANCEL
    return _answer(questionary.select(
        f"Ask Bazaar DB for {count} items?",
        choices=[Choice("Start the scrape", START), Choice("Show every item first", SHOW), Choice("Cancel", CANCEL)],
        style=STYLE,
    ))
