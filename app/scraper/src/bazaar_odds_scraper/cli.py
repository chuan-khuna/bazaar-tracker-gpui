"""`scrape-odds`: save Bazaar DB's odds for every item of the picked heroes to `odds/<template_id>.json`.

Run with no options in a terminal (or with -i) to be asked what to scrape; any other options run
the scrape straight away, as a script would.

Items come from the `items.json` the tracker writes into its data folder when it loads the card
catalog, so each file is keyed by the template id the tracker uses. An item already saved for the hero and tier is skipped (pass `--refresh` to ask again), so a
stopped run picks up where it left off. Between one ask and the next it waits a moment picked at
random, as the tracker does (`src/ui/odds_load.rs`), so Bazaar DB is asked at a pace a person
could keep.
"""

from __future__ import annotations

import argparse
import json
import logging
import random
import sys
import time
from collections import Counter
from pathlib import Path

from rich.progress import BarColumn, MofNCompleteColumn, Progress, SpinnerColumn, TextColumn, TimeElapsedColumn
from rich.table import Table

from . import catalog, prompts
from .browser import ALL, NOT_OWNED, Blocked, Browser, NotFound, OddsPage, name_key
from .log import console, escape, setup
from .store import Store

log = logging.getLogger("scrape-odds")

HERE = Path(__file__).resolve().parents[2]
DATA = HERE.parents[1] / "data" / "scraper"  # at the repo root, beside the app's .bazaar-tracker/


def _args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="scrape-odds", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("-i", "--interactive", action="store_true", help="ask what to scrape, with any other options as the defaults")
    parser.add_argument("--out", type=Path, default=DATA / "odds", help="the odds folder (default: data/scraper/odds)")
    parser.add_argument("--items", type=Path, dest="items_file", help="the tracker's items.json (default: the one in its data folder)")
    parser.add_argument("--hero", action="append", dest="heroes", help="a hero to scrape, as the catalog names it; repeat for more (default: every hero)")
    parser.add_argument("--no-neutral", action="store_true", help="leave out the neutral (Common) items")
    parser.add_argument("--tier", action="append", dest="tiers", choices=prompts.TIERS, help="only this owned tier, one page load per tier; repeat for more "
                        "(default: every owned tier the item has, clicked through on one page)")
    parser.add_argument("--item", action="append", dest="names", help="only items whose name contains this; repeat for more")
    parser.add_argument("--limit", type=int, help="stop after this many asks")
    parser.add_argument("--refresh", action="store_true", help="ask again for odds already saved, and for items skipped before")
    parser.add_argument("--retry-skipped", action="store_true", help="ask again for the items kept in odds/skipped.json")
    parser.add_argument("--retries", type=int, default=1, help="how many times to try a failed item again (no reply, not found, "
                        "blocked), reloading the page each time, before keeping it in skipped.json (default: 1)")
    parser.add_argument("--gap", type=float, nargs=2, default=(0.5, 4.0), metavar=("MIN", "MAX"), help="seconds to wait between asks (default: 0.5 4)")
    parser.add_argument("--timeout", type=float, default=20.0, help="seconds to wait for the page or a reply (default: 20)")
    parser.add_argument("--human-wait", type=float, default=300.0, help="seconds to wait for a Cloudflare check done by hand (default: 300)")
    parser.add_argument("--browser", default="chrome", help="'chrome' (installed Google Chrome) or 'chromium' (Playwright's; run `uv run playwright install chromium`)")
    parser.add_argument("--headless", action="store_true", help="hide the window (Cloudflare is likely to stop it)")
    parser.add_argument("--profile", type=Path, default=HERE / ".browser-profile", help="the browser profile folder")
    parser.add_argument("--cdp", metavar="URL", help="attach to a running Chromium browser (such as Dia or Chrome started with "
                        "--remote-debugging-port=9222) at this URL, such as http://localhost:9222, and scrape in a new tab "
                        "of it, with its own profile; --browser, --headless and --profile are then not used")
    parser.add_argument("--list", action="store_true", help="print what would be asked for, and stop")
    parser.add_argument("--index-only", action="store_true", help="rewrite odds/index.json, and stop")
    parser.add_argument("-v", "--verbose", action="store_true", help="log more, such as each wait between asks")
    return parser.parse_args()


Ask = tuple[str, list[catalog.Item], str, str]  # the name, every item of that name, the hero, the tier


def _queue(items: list[catalog.Item], args: argparse.Namespace, store: Store) -> list[Ask]:
    """One ask per name, hero and tier (`ALL`, every owned tier, unless tiers were picked): the game
    has several template ids for some names (such as each copy of a package), which Bazaar DB knows
    as one item, so the answer is saved for each."""
    heroes = args.heroes or catalog.heroes(items)
    tiers = args.tiers or [ALL]
    filters = [text.lower() for text in args.names or []]
    queue = []
    for hero in heroes:
        for tier in tiers:
            by_name: dict[str, list[catalog.Item]] = {}
            for item in catalog.for_hero(items, hero, not args.no_neutral):
                if filters and not any(text in item.name.lower() for text in filters):
                    continue
                by_name.setdefault(name_key(item.name), []).append(item)
            for same in by_name.values():
                done = (lambda item: store.has_every_tier(item.template_id, hero)) if tier == ALL else (lambda item: store.has(item.template_id, hero, tier))
                if not args.refresh and all(done(item) for item in same):
                    continue
                if not (args.refresh or args.retry_skipped) and any(store.was_skipped(item.template_id, hero, tier) for item in same):
                    continue
                queue.append((same[0].name, same, hero, tier))
    return queue[: args.limit] if args.limit else queue


def _summary(queue: list[Ask]) -> Table:
    table = Table(title=f"{len(queue)} to ask for", title_justify="left", header_style="bold")
    table.add_column("Hero", style="hero")
    table.add_column("Tier", style="tier")
    table.add_column("Items", justify="right")
    for (hero, tier), count in Counter((hero, tier) for _, _, hero, tier in queue).items():
        table.add_row(hero, tier, str(count))
    return table


def _every_ask(queue: list[Ask]) -> Table:
    table = Table(header_style="bold")
    table.add_column("Hero", style="hero")
    table.add_column("Tier", style="tier")
    table.add_column("Item", style="item")
    table.add_column("Template ids", style="muted")
    for name, same, hero, tier in queue:
        table.add_row(hero, tier, escape(name), ", ".join(item.template_id for item in same))
    return table


def main() -> None:
    args = _args()
    setup(args.verbose)
    try:
        _run(args)
    except prompts.Cancelled:
        log.info("[muted]cancelled; nothing was asked for[/]")
    except SystemExit as stop:
        if isinstance(stop.code, str):
            log.error(escape(stop.code))
            raise SystemExit(1) from None
        raise


def _run(args: argparse.Namespace) -> None:
    interactive = args.interactive or (len(sys.argv) == 1 and sys.stdin.isatty())
    store = Store(args.out)
    if args.index_only:
        log.info(f"index: {store.write_index()} items in [path]{store.folder / 'index.json'}[/]")
        return

    items = catalog.load(args.items_file)
    if interactive:
        console.rule("[bold]Bazaar DB odds scraper")
        prompts.ask(args, items, len(store.skipped()))
    queue = _queue(items, args, store)
    if not queue:
        log.info(f"[saved]nothing to ask for[/]: every picked item is saved or was skipped before (ask again, or retry the skipped, to redo them). "
                 f"index: {store.write_index()} items")
        return

    console.print(_summary(queue))
    if args.list:
        console.print(_every_ask(queue))
        return
    if interactive:
        step = prompts.next_step(len(queue))
        if step == prompts.SHOW:
            console.print(_every_ask(queue))
            step = prompts.next_step(len(queue))
        if step != prompts.START:
            raise prompts.Cancelled
    _scrape(args, queue, store)


def _scrape(args: argparse.Namespace, queue: list[Ask], store: Store) -> None:
    saved = skipped = 0
    started = time.monotonic()
    channel = None if args.browser == "chromium" else args.browser
    where = f"the browser on {args.cdp}" if args.cdp else f"{args.browser} with the profile in {args.profile}"
    log.info(f"opening [path]{escape(where)}[/]")
    progress = Progress(
        SpinnerColumn(), TextColumn("[progress.description]{task.description}"), BarColumn(),
        MofNCompleteColumn(), TimeElapsedColumn(), console=console,
    )
    try:
        with Browser(args.profile, channel, args.headless, args.cdp) as tab, progress:
            page = OddsPage(tab, args.human_wait, args.timeout)
            task = progress.add_task("asking", total=len(queue))
            for number, (name, same, hero, tier) in enumerate(queue, 1):
                label = f"[item]{escape(name)}[/] [muted]([/][hero]{hero}[/][muted],[/] [tier]{tier}[/][muted])[/]"
                progress.update(task, description=f"[item]{escape(name)}[/]")
                url_id = next((known for item in same if (known := store.url_id(item.template_id))), None)
                def pause() -> None:
                    gap = random.uniform(*args.gap)
                    log.debug(f"[muted]waiting {gap:.1f}s[/]")
                    time.sleep(gap)

                tries = 1 + args.retries
                for attempt in range(1, tries + 1):
                    last = attempt == tries
                    again = f"[muted](try {attempt + 1} of {tries})[/]"
                    try:
                        if tier == ALL:
                            got, missing = _ask_every_tier(page, store, name, same, hero, url_id, args.refresh and attempt == 1, pause, last)
                            if missing and not last:
                                log.warning(f"[skipped]retrying[/] {label}: no odds at {', '.join(missing)}; reloading the page {again}")
                                page.place = None
                                pause()
                                continue
                            saved += 1 if got else 0
                            skipped += 0 if got else 1
                        else:
                            reply = page.ask(name, hero, tier, url_id)
                            entry = {}
                            for item in same:
                                entry = _entry(item, reply)
                                store.save(entry)
                            saved += 1
                            note = "" if entry["hero"] == hero else f" [muted](saved for {escape(entry['hero'])}, Bazaar DB's choice)[/]"
                            log.info(f"[saved]saved[/]    {label}{note}")
                        break
                    except Blocked as why:
                        if last:
                            raise SystemExit(f"{name} ({hero}, {tier}): Bazaar DB's bot check stopped the scrape ({why}). Run again to carry on.")
                        log.warning(f"[blocked]blocked[/]  {label}: {escape(str(why))}; reloading the page, do any check in the window {again}")
                        page.place = None
                    except (NotFound, ValueError) as why:
                        suggestions = why.suggestions if isinstance(why, NotFound) else []
                        offered = f" [muted](the search offered: {escape(', '.join(suggestions))})[/]" if suggestions else ""
                        if not last:
                            log.warning(f"[skipped]retrying[/] {label}: {escape(str(why))}{offered}; reloading the page {again}")
                            page.place = None
                            pause()
                            continue
                        skipped += 1
                        store.skip(name, [item.template_id for item in same], hero, tier, str(why), suggestions)
                        log.warning(f"[skipped]skipped[/]  {label}: {escape(str(why))}{offered}")
                        break
                progress.advance(task)
                if number < len(queue):
                    pause()
    except KeyboardInterrupt:
        log.warning("stopped; run again to carry on where this left off")
    finally:
        minutes = (time.monotonic() - started) / 60
        log.info(f"done: [saved]{saved} saved[/], [skipped]{skipped} skipped[/] in {minutes:.1f} min; "
                 f"index: {store.write_index()} items")


def _ask_every_tier(page: OddsPage, store: Store, name: str, same: list[catalog.Item], hero: str,
                    url_id: str | None, refresh: bool, pause, last: bool) -> tuple[int, list[str]]:
    """Ask for `name` at every owned tier it has, on one page, and save each answer for every item of
    that name. Returns how many tiers were saved, and the tiers that got no usable answer; on the
    `last` try those are kept in skipped.json, else the caller tries again."""
    have = set() if refresh else set.intersection(*(store.saved_tiers(item.template_id, hero) for item in same))
    result = page.ask_all(name, hero, url_id, have, pause)
    label = f"[item]{escape(name)}[/] [muted]([/][hero]{hero}[/][muted])[/]"
    template_ids = [item.template_id for item in same]
    saved, wrong = [], []
    for tier in result.offered:
        reply = result.replies.get(tier)
        if reply is None:
            continue
        try:
            for item in same:
                store.save(_entry(item, reply))
            saved.append(tier)
        except ValueError as why:
            wrong.append(tier)
            if last:
                store.skip(name, template_ids, hero, tier, str(why), [])
                log.warning(f"[skipped]skipped[/]  {label} at [tier]{tier}[/]: {escape(str(why))}")
    for item in same:
        store.set_owned_tiers(item.template_id, result.offered)
    if saved:
        store.forget(template_ids, hero, ALL)  # an earlier "not in the search" for the whole item
    for tier, why in result.failed.items():
        if last:
            store.skip(name, template_ids, hero, tier, why, [])
            log.warning(f"[skipped]skipped[/]  {label} at [tier]{tier}[/]: {escape(why)}")
    kept = sorted(have & set(result.offered), key=result.offered.index)
    tiers = ", ".join(f"[tier]{tier}[/]" for tier in saved) or "[muted]nothing new[/]"
    already = f" [muted](had {', '.join(kept)})[/]" if kept else ""
    if saved or not (result.failed or wrong):
        log.info(f"[saved]saved[/]    {label}: {tiers}{already}")
    return len(saved), [*result.failed, *wrong]


def _entry(item: catalog.Item, reply) -> dict:
    """A `SavedItemOdds` for `reply`, if it is for `item` (as `src/tracking/item_odds.rs::accept`)."""
    answer = json.loads(reply.body)
    if not isinstance(answer, dict):
        raise ValueError("Bazaar DB's answer is not an object")
    got = answer.get("itemName") or ""
    if name_key(got) != name_key(item.name):
        raise ValueError(f"Bazaar DB sent the odds for {got or 'another item'}, not {item.name}")
    return {
        "template_id": item.template_id,
        "hero": answer.get("hero") or reply.hero,
        "tier": answer.get("ownedTier") or reply.tier or NOT_OWNED,
        "url_id": answer.get("itemUrlId") or reply.item,
        "item_name": got or item.name,
        "body": reply.body,
        "saved_ms": int(time.time() * 1000),
    }


if __name__ == "__main__":
    main()
