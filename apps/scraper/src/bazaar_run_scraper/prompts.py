"""The interactive run scrape (questionary): which profiles and feeds, how many runs of a feed,
whether to read saved runs again, and the browser; then, for each profile or feed, a look at its
runs before any of them is read.

Profiles and feeds scraped before (found in the builds folder) are offered to tick; new ones are
pasted as links, which also spares the shell's quoting of a feed link's `?` and `&`. Whatever flags
were passed become the answers' defaults. Ctrl+C or Esc on any question stops.
"""

from __future__ import annotations

import argparse
import json
import logging
from pathlib import Path

import questionary
from questionary import Choice

from bazaar_odds_scraper.log import console
from bazaar_odds_scraper.prompts import CANCEL, SHOW, START, STYLE, Cancelled, _answer, choose_browser

from . import site

SKIP = "skip"

log = logging.getLogger("scrape-runs")


def _read(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def _known(out: Path) -> list[Choice]:
    """A choice for each profile and feed in the builds folder, the most runs saved first."""
    found: list[tuple[int, Choice]] = []
    for metadata in out.glob("*/metadata.json"):
        profile = _read(metadata)
        saved = len(_read(metadata.with_name("index.json")).get("runs") or [])
        profile_id = profile.get("profile_id") or metadata.parent.name
        if not metadata.with_name("stats.json").is_file():
            continue  # a player only seen in feeds: their profile was never scraped
        name = profile.get("profile_name") or profile_id
        found.append((saved, Choice(f"{name}  (profile, {saved} runs saved)", ("profile", profile_id))))
    for feed in out.glob("feeds/*.json"):
        listed = _read(feed)
        if url := listed.get("url"):
            count = len(listed.get("runs") or [])
            found.append((count, Choice(f"{feed.stem}  (feed, {count} runs listed)", ("feed", url))))
    return [choice for _, choice in sorted(found, key=lambda pair: -pair[0])]


def _check_links(text: str) -> bool | str:
    bad = [part for part in text.split() if site.source_of(part) is None]
    return not bad or f"not a profile link or id, nor a bazaardb.gg/run feed link: {bad[0]}"


def ask(args: argparse.Namespace) -> None:
    """Fill in `args.sources` and the rest from the person's answers; raises `Cancelled` if they stop."""
    sources: list[tuple[str, str]] = [found for text in args.sources if (found := site.source_of(text))]
    known = _known(args.out)
    if known:
        picked = _answer(questionary.checkbox(
            "Scrape again any saved profiles or feeds?", choices=known, style=STYLE,
            instruction="(space to tick, enter when done; none is fine)",
        ))
        sources += [source for source in picked if source not in sources]
    while True:
        first = not sources

        def check(text: str, first: bool = first) -> bool | str:
            if not text.strip():
                return not first or "paste at least one link"
            return _check_links(text)

        text = _answer(questionary.text(
            "Paste a profile link or id, or a run feed link (bazaardb.gg/run?…):" if first
            else "Another link (enter when done):",
            style=STYLE, validate=check,
        ))
        if not text.strip():
            break
        sources += [found for part in text.split() if (found := site.source_of(part)) and found not in sources]
    args.sources = sources

    if any(kind == "feed" for kind, _ in sources):
        args.max_runs = int(_answer(questionary.text(
            "How many runs of each feed, from its top?", default=str(args.max_runs), style=STYLE,
            validate=lambda text: (text.strip().isdigit() and int(text) > 0) or "a whole number, 1 or more",
        )))
    args.refresh = not _answer(questionary.confirm("Only runs not saved yet?", default=not args.refresh, style=STYLE))

    needs_browser = args.in_browser or any(kind == "feed" for kind, _ in sources)
    launch = (f"Launch {args.browser} with the scraper's own profile instead" if needs_browser
              else f"Plain HTTP; launch {args.browser} with the scraper's profile only if a page needs it")
    if not needs_browser:
        console.print("[muted]A profile's first 20 runs and every run page come over plain HTTP; a browser is needed "
                      "for its runs past the 20th, and if Cloudflare stops the plain fetches.[/]")
    args.cdp = choose_browser(args.cdp, launch)


def next_step(label: str, count: int) -> str:
    """Before a profile's or feed's runs are read: START, SHOW (print every run), SKIP or CANCEL."""
    return _answer(questionary.select(
        f"Read {count} runs of {label}?",
        choices=[Choice("Start", START), Choice("Show every run first", SHOW),
                 Choice("Skip this one", SKIP), Choice("Cancel the rest", CANCEL)],
        style=STYLE,
    ))
