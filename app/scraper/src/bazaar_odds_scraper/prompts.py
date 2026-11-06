"""The interactive scrape: questions (questionary) that fill in the same options the flags do,
then a look at the queue before anything is asked of Bazaar DB.

Whatever flags were passed become the answers' defaults, so `scrape-odds -i --hero Jules` starts
with Jules ticked. Ctrl+C or Esc on any question stops without asking for anything.
"""

from __future__ import annotations

import argparse
import json
import urllib.request

import questionary
from questionary import Choice, Style

from . import catalog
from .browser import NOT_OWNED

TIERS = ["None", "Bronze", "Silver", "Gold", "Diamond"]
DEFAULT_CDP = "http://localhost:9222"

STYLE = Style([
    ("qmark", "fg:#5fafff bold"),
    ("question", "bold"),
    ("answer", "fg:#5fd75f bold"),
    ("pointer", "fg:#5fafff bold"),
    ("highlighted", "fg:#5fafff bold"),
    ("selected", "fg:#5fd75f"),
    ("instruction", "fg:#808080 italic"),
])

ATTACH, CHROME, CHROMIUM = "attach", "chrome", "chromium"
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
    found = running_browser(args.cdp or DEFAULT_CDP)
    attach_label = (f"Attach to the browser on {args.cdp or DEFAULT_CDP} ({found}), its own profile"
                    if found else "Attach to a running browser started with --remote-debugging-port (such as Dia)")
    choices = [
        Choice(attach_label, ATTACH),
        Choice("Launch Google Chrome with the scraper's profile", CHROME),
        Choice("Launch Playwright's Chromium with the scraper's profile", CHROMIUM),
    ]
    default = ATTACH if found or args.cdp else (CHROMIUM if args.browser == CHROMIUM else CHROME)
    mode = _answer(questionary.select("Which browser?", choices=choices, default=default, style=STYLE))
    if mode == ATTACH:
        url = args.cdp or DEFAULT_CDP
        if not found:
            url = _answer(questionary.text(
                "Debugging URL (start the browser with --remote-debugging-port=9222):",
                default=url, style=STYLE,
                validate=lambda text: bool(running_browser(text)) or f"nothing answers on {text} yet",
            ))
        args.cdp = url
    else:
        args.cdp = None
        args.browser = mode


def _heroes(args: argparse.Namespace, items: list[catalog.Item]) -> None:
    picked = set(args.heroes or [])
    choices = [
        Choice(f"{hero}  ({len(catalog.for_hero(items, hero, False))} items)", hero, checked=hero in picked)
        for hero in catalog.heroes(items)
    ]
    args.heroes = _answer(questionary.checkbox(
        "Which heroes?", choices=choices, style=STYLE,
        instruction="(space to tick, a for all, enter when done)",
        validate=lambda chosen: bool(chosen) or "tick at least one hero",
    ))


def _tiers(args: argparse.Namespace) -> None:
    picked = set(args.tiers or [NOT_OWNED])
    choices = [Choice("None (not owned)" if tier == NOT_OWNED else tier, tier, checked=tier in picked) for tier in TIERS]
    args.tiers = _answer(questionary.checkbox(
        "Odds for which owned tier?", choices=choices, style=STYLE,
        validate=lambda chosen: bool(chosen) or "tick at least one tier",
    ))


def _filters(args: argparse.Namespace, skipped: int) -> None:
    args.no_neutral = not _answer(questionary.confirm(
        "Include the neutral (Common) items?", default=not args.no_neutral, style=STYLE))
    names = _answer(questionary.text(
        "Only items whose name contains (comma-separated, blank for every item):",
        default=", ".join(args.names or []), style=STYLE))
    args.names = [name.strip() for name in names.split(",") if name.strip()] or None
    args.refresh = _answer(questionary.confirm(
        "Ask again for odds already saved?", default=args.refresh, style=STYLE))
    if not args.refresh and skipped:
        args.retry_skipped = _answer(questionary.confirm(
            f"Ask again for the {skipped} items skipped before (kept in odds/skipped.json)?",
            default=args.retry_skipped, style=STYLE))
    limit = _answer(questionary.text(
        "Stop after how many asks? (blank for no limit):",
        default=str(args.limit or ""), style=STYLE,
        validate=lambda text: not text.strip() or (text.strip().isdigit() and int(text) > 0) or "a whole number above 0",
    ))
    args.limit = int(limit) if limit.strip() else None


def ask(args: argparse.Namespace, items: list[catalog.Item], skipped: int = 0) -> None:
    """Fill in `args` from the person's answers; raises `Cancelled` if they stop."""
    _browser(args)
    _heroes(args, items)
    _tiers(args)
    _filters(args, skipped)


def next_step(count: int) -> str:
    """After the queue is shown: START, SHOW (print every ask) or CANCEL."""
    if not count:
        return CANCEL
    return _answer(questionary.select(
        f"Ask Bazaar DB for {count} items?",
        choices=[Choice("Start the scrape", START), Choice("Show every item first", SHOW), Choice("Cancel", CANCEL)],
        style=STYLE,
    ))
