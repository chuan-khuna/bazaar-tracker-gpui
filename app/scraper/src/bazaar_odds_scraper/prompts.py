"""The interactive scrape: a few questions (questionary) for what changes from run to run (the
characters, whether to redo what is saved or skipped, the retries), then a look at the queue before
anything is asked of Bazaar DB. A browser answering on the debugging port is used without asking;
when none does, `choose_browser` shows how to start a real one and offers to start it. Every owned
tier is scraped; the other options come from the flags.

Whatever flags were passed become the answers' defaults, so `scrape-odds -i --hero Jules` starts
with Jules ticked. Ctrl+C or Esc on any question stops without asking for anything.
"""

from __future__ import annotations

import argparse
import logging

import questionary
from questionary import Choice, Separator, Style

from . import browsers, catalog
from .browsers import DEFAULT_CDP, running_browser
from .log import console, escape

TIERS = ["None", "Bronze", "Silver", "Gold", "Diamond"]

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


_CHECK, _LAUNCH = "check", "launch"


def choose_browser(cdp: str | None, launch_label: str) -> str | None:
    """The debugging port to attach to, or None to let the scraper launch its own browser.

    A browser already answering on the port is used without asking. Otherwise the commands to start
    each real browser found here are shown, and the person picks one to start (the port is then
    waited for), checks the port again after starting one by hand, or takes `launch_label`."""
    url = cdp or DEFAULT_CDP
    port = browsers.port_of(url)
    while True:
        if found := running_browser(url):
            log.info(f"browser: attaching to [path]{url}[/] ({escape(found)}), with its own profile and Cloudflare clearance")
            return url
        real = browsers.installed()
        console.print(f"\n[bold]No browser answers on {url}.[/] A real browser with a debugging port keeps your Cloudflare "
                      "clearance best. To start one by hand:")
        for browser in real:
            quit_first = " [muted](quit it fully first: it opens the port on its normal profile)[/]" if browser.normal_profile else ""
            console.print(f"  [hero]{browser.name}[/]{quit_first}\n    [muted]{escape(browser.shell_line(port))}[/]", soft_wrap=True)
        if not real:
            console.print("  [skipped]no Chrome, Edge, Brave or Dia found here[/]")
        console.print()
        choices = [Choice(f"Start {browser.name} with the port for me", browser) for browser in real]
        choices += [Choice(f"I started one myself: check {url} again", _CHECK), Choice(launch_label, _LAUNCH),
                    Separator(), Choice("Cancel", CANCEL)]
        picked = _answer(questionary.select("Which browser?", choices=choices, style=STYLE))
        if picked == CANCEL:
            raise Cancelled
        if picked == _LAUNCH:
            return None
        if picked == _CHECK:
            continue
        log.info(f"starting [hero]{picked.name}[/]: [muted]{escape(picked.shell_line(port))}[/]")
        try:
            found = browsers.start(picked, port)
        except browsers.NotStarted as why:
            log.warning(f"[skipped]{escape(str(why))}[/]")
            continue
        log.info(f"[saved]{escape(found)} answers on {url}[/]; pass any Bazaar DB check in its window. "
                 "Quit it when the scrape is done: while the port is open any program here can drive it.")
        return url


def _browser(args: argparse.Namespace) -> None:
    args.cdp = choose_browser(args.cdp, f"Launch {args.browser} with the scraper's own profile instead")


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
