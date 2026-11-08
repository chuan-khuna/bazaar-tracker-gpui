"""`scrape-runs`: save every run of Bazaar DB profiles to `data/scraper/builds/<profile id>/runs/season-NN/<run id>.json`.

    uv run scrape-runs https://bazaardb.gg/run/profile/829739a0-9c10-44fa-b4b7-d59138eb2ed5

Each run file has the run as the profile lists it (result, rank, final board and skills, fights)
and the run page's timeline (both boards at each moment, every choice offered and taken, every
sale, every fight with its opponent); see `store.py` for the format. A run already saved is skipped
(pass `--refresh` to read it again), so a stopped scrape picks up where it left off. Between one page
and the next it waits a moment picked at random, so Bazaar DB is asked at a pace a person could keep.

Pages are fetched over plain HTTP. A browser opens only when it is needed: for a profile with more
than 20 runs (the rest are paged in by a script that wants a Cloudflare token), or if Cloudflare
stops the plain fetches. Run data is only read from Bazaar DB, never sent to it.
"""

from __future__ import annotations

import argparse
import logging
import random
import time
from contextlib import ExitStack
from pathlib import Path

from rich.progress import BarColumn, MofNCompleteColumn, Progress, SpinnerColumn, TextColumn, TimeElapsedColumn

from bazaar_odds_scraper.browser import Browser
from bazaar_odds_scraper.log import console, escape, setup

from . import site
from .site import Blocked, Http, InBrowser, Missing
from .store import Store

log = logging.getLogger("scrape-runs")

HERE = Path(__file__).resolve().parents[2]
DATA = HERE.parents[1] / "data" / "scraper"  # at the repo root, beside the app's .bazaar-tracker/


def _args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="scrape-runs", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("profiles", nargs="+", metavar="PROFILE", help="a profile's link (https://bazaardb.gg/run/profile/<id>) or id")
    parser.add_argument("--out", type=Path, default=DATA / "builds", help="the folder to save into, one folder per profile (default: data/scraper/builds)")
    parser.add_argument("--refresh", action="store_true", help="read runs already saved again")
    parser.add_argument("--limit", type=int, help="read at most this many runs of each profile")
    parser.add_argument("--list", action="store_true", help="print the runs that would be read, and stop")
    parser.add_argument("--gap", type=float, nargs=2, default=(1.0, 4.0), metavar=("MIN", "MAX"), help="seconds to wait between pages (default: 1 4)")
    parser.add_argument("--timeout", type=float, default=30.0, help="seconds to wait for a page (default: 30)")
    parser.add_argument("--in-browser", action="store_true", help="fetch every page in the browser, not over plain HTTP")
    parser.add_argument("--human-wait", type=float, default=300.0, help="seconds to wait for a Cloudflare check done by hand (default: 300)")
    parser.add_argument("--browser", default="chrome", help="'chrome' (installed Google Chrome) or 'chromium' (Playwright's; run `uv run playwright install chromium`)")
    parser.add_argument("--headless", action="store_true", help="hide the browser window (Cloudflare is likely to stop it)")
    parser.add_argument("--browser-profile", type=Path, default=HERE / ".browser-profile", help="the browser profile folder")
    parser.add_argument("--cdp", metavar="URL", help="attach to a running Chromium browser started with --remote-debugging-port, "
                        "such as http://localhost:9222, and work in a new tab of it")
    parser.add_argument("-v", "--verbose", action="store_true", help="log more, such as each wait between pages")
    return parser.parse_args()


class Pages:
    """Where pages come from: plain HTTP until a browser is needed, then the browser for good."""

    def __init__(self, args: argparse.Namespace, stack: ExitStack) -> None:
        self.args = args
        self.stack = stack
        self.http = Http(args.timeout)
        self.browser: InBrowser | None = None
        if args.in_browser:
            self.open_browser()

    def open_browser(self) -> InBrowser:
        if self.browser is None:
            args = self.args
            where = f"the browser on {args.cdp}" if args.cdp else f"{args.browser} with the profile in {args.browser_profile}"
            log.info(f"opening [path]{escape(where)}[/]")
            channel = None if args.browser == "chromium" else args.browser
            tab = self.stack.enter_context(Browser(args.browser_profile, channel, args.headless, args.cdp))
            self.browser = InBrowser(tab, args.human_wait, args.timeout)
        return self.browser

    def flight(self, url: str):
        if self.browser is None:
            try:
                return self.http.flight(url)
            except Blocked as why:
                log.warning(f"[blocked]blocked[/] over plain HTTP ({escape(str(why))}); fetching in a browser from now on")
        return self.open_browser().flight(url)


def main() -> None:
    args = _args()
    setup(args.verbose)
    profile_ids = [site.profile_id_of(text) for text in args.profiles]
    try:
        with ExitStack() as stack:
            pages = Pages(args, stack)
            seasons = site.Seasons.from_site(args.timeout)
            for profile_id in profile_ids:
                _scrape_profile(args, pages, seasons, profile_id)
    except KeyboardInterrupt:
        log.warning("stopped; run again to carry on where this left off")
    except Blocked as why:
        log.error(f"Bazaar DB's bot check stopped the scrape ({escape(str(why))}). Run again to carry on.")
        raise SystemExit(1) from None


def _pause(args: argparse.Namespace) -> None:
    gap = random.uniform(*args.gap)
    log.debug(f"[muted]waiting {gap:.1f}s[/]")
    time.sleep(gap)


def _every_run(args: argparse.Namespace, pages: Pages, props: dict) -> list[dict]:
    """The profile's runs, newest first: the ones on its page, and, when it has more, the rest as
    its page pages them in a browser."""
    runs = list(props.get("initialRuns") or [])
    count = (props.get("profile") or {}).get("runCount") or len(runs)
    if count <= len(runs):
        return runs
    log.info(f"the profile has {count} runs and its page shows {len(runs)}; scrolling through the rest in a browser")
    known = {run["id"] for run in runs}
    more = pages.open_browser().every_run(props["profile"]["id"], count)
    runs += [run for run in more if run.get("id") not in known and not known.add(run["id"])]
    if len(runs) < count:
        log.warning(f"found {len(runs)} of the profile's {count} runs; the rest did not load")
    return runs


def _scrape_profile(args: argparse.Namespace, pages: Pages, seasons: site.Seasons, profile_id: str) -> None:
    url = site.profile_url(profile_id)
    props = site.profile(pages.flight(url))
    name = (props.get("profile") or {}).get("nickname") or profile_id
    store = Store(args.out, profile_id)
    store.save_profile(props, url)
    runs = _every_run(args, pages, props)
    queue = [run for run in runs if args.refresh or not store.has(run["id"])]
    if args.limit:
        queue = queue[: args.limit]
    label = f"[hero]{escape(name)}[/]"
    log.info(f"{label}: {len(runs)} runs, {len(runs) - len([r for r in runs if not store.has(r['id'])])} saved already, "
             f"{len(queue)} to read")
    if args.list:
        for run in queue:
            console.print(f"  {run.get('createdAt', '')[:16]}  [hero]{escape(run.get('hero') or '')}[/]  "
                          f"{run.get('statWins')}W {run.get('victoryTier') or ''}  [muted]{site.run_url(run)}[/]", soft_wrap=True)
        return
    if not queue:
        log.info(f"{label}: [saved]nothing new to read[/] (pass --refresh to read every run again); "
                 f"index: {store.write_index()} runs in [path]{store.folder}[/]")
        return

    saved = without_timeline = 0
    started = time.monotonic()
    progress = Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), BarColumn(),
                        MofNCompleteColumn(), TimeElapsedColumn(), console=console)
    try:
        with progress:
            task = progress.add_task(name, total=len(queue))
            for number, run in enumerate(queue, 1):
                run_url = site.run_url(run)
                what = f"[item]{escape(run.get('hero') or '?')}[/] [muted]{run.get('createdAt', '')[:16]} ({run['id'][:8]})[/]"
                progress.update(task, description=what)
                try:
                    timeline = site.timeline(pages.flight(run_url))
                except Missing as why:
                    timeline = None
                    without_timeline += 1
                    log.warning(f"[skipped]no timeline[/] {what}: {escape(str(why))}; saving the summary only")
                store.save_run(run, timeline, run_url, seasons.of(run.get("createdAt")))
                saved += 1
                log.info(f"[saved]saved[/]    {what}: {run.get('statWins')} wins, {escape(run.get('victoryTier') or 'no victory')}")
                progress.advance(task)
                if number < len(queue):
                    _pause(args)
    finally:
        minutes = (time.monotonic() - started) / 60
        note = f", {without_timeline} without a timeline" if without_timeline else ""
        log.info(f"{label}: [saved]{saved} saved[/]{note} in {minutes:.1f} min; "
                 f"index: {store.write_index()} runs in [path]{store.folder}[/]")


if __name__ == "__main__":
    main()
