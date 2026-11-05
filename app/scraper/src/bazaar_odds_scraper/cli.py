"""`scrape-odds`: save Bazaar DB's odds for every item of the picked heroes to `odds/<template_id>.json`.

Items come from the `items.json` the tracker writes into its data folder when it loads the card
catalog, so each file is keyed by the template id the tracker uses. An item already saved for the hero and tier is skipped (pass `--refresh` to ask again), so a
stopped run picks up where it left off. Between one ask and the next it waits a moment picked at
random, as the tracker does (`src/ui/odds_load.rs`), so Bazaar DB is asked at a pace a person
could keep.
"""

from __future__ import annotations

import argparse
import json
import random
import time
from pathlib import Path

from . import catalog
from .browser import NOT_OWNED, Blocked, Browser, NotFound, OddsPage, name_key
from .store import Store

HERE = Path(__file__).resolve().parents[2]
TIERS = ["None", "Bronze", "Silver", "Gold", "Diamond"]


def _args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="scrape-odds", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=HERE / "odds", help="the odds folder (default: app/scraper/odds)")
    parser.add_argument("--items", type=Path, dest="items_file", help="the tracker's items.json (default: the one in its data folder)")
    parser.add_argument("--hero", action="append", dest="heroes", help="a hero to scrape, as the catalog names it; repeat for more (default: every hero)")
    parser.add_argument("--no-neutral", action="store_true", help="leave out the neutral (Common) items")
    parser.add_argument("--tier", action="append", dest="tiers", choices=TIERS, help="the tier owned; repeat for more (default: None, not owned)")
    parser.add_argument("--item", action="append", dest="names", help="only items whose name contains this; repeat for more")
    parser.add_argument("--limit", type=int, help="stop after this many asks")
    parser.add_argument("--refresh", action="store_true", help="ask again for odds already saved")
    parser.add_argument("--gap", type=float, nargs=2, default=(0.5, 4.0), metavar=("MIN", "MAX"), help="seconds to wait between asks (default: 0.5 4)")
    parser.add_argument("--timeout", type=float, default=20.0, help="seconds to wait for the page or a reply (default: 20)")
    parser.add_argument("--human-wait", type=float, default=300.0, help="seconds to wait for a Cloudflare check done by hand (default: 300)")
    parser.add_argument("--browser", default="chrome", help="'chrome' (installed Google Chrome) or 'chromium' (Playwright's; run `uv run playwright install chromium`)")
    parser.add_argument("--headless", action="store_true", help="hide the window (Cloudflare is likely to stop it)")
    parser.add_argument("--profile", type=Path, default=HERE / ".browser-profile", help="the browser profile folder")
    parser.add_argument("--list", action="store_true", help="print what would be asked for, and stop")
    parser.add_argument("--index-only", action="store_true", help="rewrite odds/index.json, and stop")
    return parser.parse_args()


Ask = tuple[str, list[catalog.Item], str, str]  # the name, every item of that name, the hero, the tier


def _queue(items: list[catalog.Item], args: argparse.Namespace, store: Store) -> list[Ask]:
    """One ask per name, hero and tier: the game has several template ids for some names (such as
    each copy of a package), which Bazaar DB knows as one item, so the answer is saved for each."""
    heroes = args.heroes or catalog.heroes(items)
    tiers = args.tiers or [NOT_OWNED]
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
                if args.refresh or not all(store.has(item.template_id, hero, tier) for item in same):
                    queue.append((same[0].name, same, hero, tier))
    return queue[: args.limit] if args.limit else queue


def main() -> None:
    args = _args()
    store = Store(args.out)
    if args.index_only:
        print(f"index: {store.write_index()} items in {store.folder / 'index.json'}")
        return

    items = catalog.load(args.items_file)
    queue = _queue(items, args, store)
    print(f"{len(queue)} to ask for")
    if args.list:
        for name, same, hero, tier in queue:
            print(f"  {hero:<10} {tier:<7} {name}  ({', '.join(item.template_id for item in same)})")
        return
    if not queue:
        print(f"index: {store.write_index()} items")
        return

    saved = skipped = 0
    started = time.monotonic()
    channel = None if args.browser == "chromium" else args.browser
    try:
        with Browser(args.profile, channel, args.headless) as context:
            page = OddsPage(context, args.human_wait, args.timeout)
            for number, (name, same, hero, tier) in enumerate(queue, 1):
                label = f"[{number}/{len(queue)}] {name} ({hero}, {tier})"
                url_id = next((known for item in same if (known := store.url_id(item.template_id))), None)
                for attempt in (1, 2):
                    try:
                        reply = page.ask(name, hero, tier, url_id)
                        for item in same:
                            entry = _entry(item, reply)
                            store.save(entry)
                        saved += 1
                        note = "" if entry["hero"] == hero else f" (saved for {entry['hero']}, Bazaar DB's choice)"
                        print(f"{label}: saved{note}")
                        break
                    except Blocked as why:
                        if attempt == 2:
                            raise SystemExit(f"{label}: Bazaar DB's bot check stopped the scrape ({why}). Run again to carry on.")
                        print(f"{label}: blocked ({why}); reloading the page, do any check in the window")
                        page.place = None
                    except (NotFound, ValueError) as why:
                        skipped += 1
                        print(f"{label}: skipped: {why}")
                        break
                if number < len(queue):
                    time.sleep(random.uniform(*args.gap))
    except KeyboardInterrupt:
        print("\nstopped")
    finally:
        minutes = (time.monotonic() - started) / 60
        print(f"done: {saved} saved, {skipped} skipped in {minutes:.1f} min; index: {store.write_index()} items")


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
        "tier": reply.tier or NOT_OWNED,
        "url_id": answer.get("itemUrlId") or reply.item,
        "item_name": got or item.name,
        "body": reply.body,
        "saved_ms": int(time.time() * 1000),
    }


if __name__ == "__main__":
    main()
