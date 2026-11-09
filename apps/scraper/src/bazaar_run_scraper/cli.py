"""`scrape-runs`: save Bazaar DB runs to `.bazaar-tracker/raw_data/scraper/builds/<profile id>/runs/season-NN/<run id>.json`.

    uv run scrape-runs                     # ask what to scrape (in a terminal, with no arguments)
    uv run scrape-runs https://bazaardb.gg/run/profile/829739a0-9c10-44fa-b4b7-d59138eb2ed5
    uv run scrape-runs "https://bazaardb.gg/run?by=wins&heroes=Jules&tracker=1" --max-runs 200

Give it profiles (a link or an id: every run of the player) or run feeds (a link to Bazaar DB's run
list with its filters, such as the best Jules runs: the first `--max-runs` of them, in the feed's
order). Each run is saved in its player's folder; a feed's list of runs is kept in
`feeds/<filters>.json`. With no arguments in a terminal, or with `-i`, it asks which profiles and
feeds (offering the ones saved before), and shows each one's runs before reading them.

Each run file has the run as its list gives it (result, rank, final board and skills, fights) and
the run page's timeline (both boards at each moment, every choice offered and taken, every sale,
every fight with its opponent); see `store.py` for the format. A run already saved is skipped (pass
`--refresh` to read it again), so a stopped scrape picks up where it left off. Between one page and
the next it waits a moment picked at random, so Bazaar DB is asked at a pace a person could keep.

Pages are fetched over plain HTTP. A browser opens only when it is needed: for a feed, or a profile
with more than 20 runs (those runs are paged in by a script that wants a Cloudflare token), or if
Cloudflare stops the plain fetches. It attaches to a browser on the debugging port when one answers
there (`just scraper browser` starts one), else launches Chrome with the scraper's profile. Run data
is only read from Bazaar DB, never sent to it.
"""

from __future__ import annotations

import argparse
import logging
import random
import sys
import time
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path

from rich.progress import BarColumn, MofNCompleteColumn, Progress, SpinnerColumn, TextColumn, TimeElapsedColumn
from rich.table import Table

from bazaar_odds_scraper import browsers
from bazaar_odds_scraper.browser import Browser
from bazaar_odds_scraper.log import console, escape, setup
from bazaar_odds_scraper.prompts import CANCEL, SHOW, START, Cancelled

from . import prompts, site
from .site import Blocked, Http, InBrowser, Missing
from .store import Store, write_feed

log = logging.getLogger("scrape-runs")

HERE = Path(__file__).resolve().parents[2]
DATA = HERE.parents[1] / ".bazaar-tracker" / "raw_data" / "scraper"  # in the app's data folder: raw_data/ holds what the app can import


def _args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="scrape-runs", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("sources", nargs="*", metavar="PROFILE_OR_FEED",
                        help="a profile's link (https://bazaardb.gg/run/profile/<id>) or id, or a run feed's link "
                             "(https://bazaardb.gg/run?…, quoted in the shell); asked for when left out")
    parser.add_argument("-i", "--interactive", action="store_true", help="ask what to scrape, with any other options as the defaults")
    parser.add_argument("--out", type=Path, default=DATA / "builds", help="the folder to save into, one folder per profile (default: .bazaar-tracker/raw_data/scraper/builds)")
    parser.add_argument("--max-runs", type=int, default=100, help="how many runs of a feed to take, from its top (default: 100)")
    parser.add_argument("--refresh", action="store_true", help="read runs already saved again")
    parser.add_argument("--limit", type=int, help="read at most this many runs not saved yet of each profile or feed")
    parser.add_argument("--list", action="store_true", help="print the runs that would be read, and stop")
    parser.add_argument("--gap", type=float, nargs=2, default=(1.0, 4.0), metavar=("MIN", "MAX"), help="seconds to wait between pages (default: 1 4)")
    parser.add_argument("--timeout", type=float, default=30.0, help="seconds to wait for a page (default: 30)")
    parser.add_argument("--in-browser", action="store_true", help="fetch every page in the browser, not over plain HTTP")
    parser.add_argument("--human-wait", type=float, default=300.0, help="seconds to wait for a Cloudflare check done by hand (default: 300)")
    parser.add_argument("--browser", default="chrome", help="'chrome' (installed Google Chrome) or 'chromium' (Playwright's; run `uv run playwright install chromium`)")
    parser.add_argument("--headless", action="store_true", help="hide the browser window (Cloudflare is likely to stop it)")
    parser.add_argument("--browser-profile", type=Path, default=HERE / ".browser-profile", help="the browser profile folder")
    parser.add_argument("--cdp", metavar="URL", help="attach to a running Chromium browser started with --remote-debugging-port, "
                        "such as http://localhost:9222, and work in a new tab of it (`just scraper browser` starts one)")
    parser.add_argument("-v", "--verbose", action="store_true", help="log more, such as each wait between pages")
    return parser.parse_args()


class Pages:
    """Where pages come from: plain HTTP until a browser is needed, then the browser for good."""

    def __init__(self, args: argparse.Namespace, stack: ExitStack) -> None:
        self.args = args
        self.stack = stack
        self.http = stack.enter_context(Http(args.timeout))
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


@dataclass
class Result:
    """What one profile or feed came to, for the summary at the end."""
    label: str
    listed: int = 0
    had: int = 0
    saved: int = 0
    without_timeline: int = 0
    note: str = ""


def main() -> None:
    args = _args()
    setup(args.verbose)
    interactive = args.interactive or (not args.sources and sys.stdin.isatty())
    results: list[Result] = []
    try:
        if interactive:
            console.rule("[bold]Bazaar DB run scraper")
            prompts.ask(args)
        else:
            sources = [site.source_of(text) for text in args.sources]
            if not sources:
                raise SystemExit("give a profile link or id, or a run feed link (or run it in a terminal with no arguments to be asked)")
            if None in sources:
                bad = args.sources[sources.index(None)]
                raise SystemExit(f"{bad} is not a Bazaar DB profile link or id, nor a run feed link")
            args.sources = sources
            if args.cdp and not browsers.running_browser(args.cdp):
                raise SystemExit(f"nothing answers on {args.cdp}; {browsers.hint(browsers.port_of(args.cdp))}")
        args.interactive = interactive
        with ExitStack() as stack:
            pages = Pages(args, stack)
            seasons = site.Seasons.from_site(pages.http.client)
            for kind, value in args.sources:
                scrape = _scrape_feed if kind == "feed" else _scrape_profile
                results.append(scrape(args, pages, seasons, value))
    except Cancelled:
        log.info("[muted]cancelled[/]")
    except KeyboardInterrupt:
        log.warning("stopped; run again to carry on where this left off")
    except Blocked as why:
        log.error(f"Bazaar DB's bot check stopped the scrape ({escape(str(why))}). Run again to carry on.")
        raise SystemExit(1) from None
    except SystemExit as stop:
        if isinstance(stop.code, str):
            log.error(escape(stop.code))
            raise SystemExit(1) from None
        raise
    finally:
        if len(results) > 1 or any(result.saved for result in results):
            console.print(_summary(results, args.out))


def _summary(results: list[Result], out: Path) -> Table:
    table = Table(title=f"Saved in {out}", title_justify="left", header_style="bold")
    table.add_column("Profile or feed", style="hero")
    table.add_column("Runs", justify="right")
    table.add_column("Saved before", justify="right", style="muted")
    table.add_column("Saved now", justify="right", style="saved")
    table.add_column("No timeline", justify="right", style="skipped")
    table.add_column("", style="muted")
    for result in results:
        table.add_row(escape(result.label), str(result.listed), str(result.had), str(result.saved),
                      str(result.without_timeline or ""), result.note)
    return table


def _pause(args: argparse.Namespace) -> None:
    gap = random.uniform(*args.gap)
    log.debug(f"[muted]waiting {gap:.1f}s[/]")
    time.sleep(gap)


def _every_run(pages: Pages, props: dict) -> list[dict]:
    """The profile's runs, newest first: the ones on its page, and, when it has more, the rest as
    its page pages them in a browser."""
    runs = list(props.get("initialRuns") or [])
    count = (props.get("profile") or {}).get("runCount") or len(runs)
    if count <= len(runs):
        return runs
    log.info(f"the profile has {count} runs and its page shows {len(runs)}; scrolling through the rest in a browser")
    known = {run["id"] for run in runs}
    profile_id = props["profile"]["id"]
    more = pages.open_browser().scroll_runs(site.profile_url(profile_id), count - len(runs), f"profileId={profile_id}")
    runs += [run for run in more if run.get("id") not in known and not known.add(run["id"])]
    if len(runs) < count:
        log.warning(f"found {len(runs)} of the profile's {count} runs; the rest did not load")
    return runs


def _scrape_profile(args: argparse.Namespace, pages: Pages, seasons: site.Seasons, profile_id: str) -> Result:
    url = site.profile_url(profile_id)
    props = site.profile(pages.flight(url))
    name = (props.get("profile") or {}).get("nickname") or profile_id
    log.info(f"profile [hero]{escape(name)}[/] [muted]{url}[/]")
    store = Store(args.out, profile_id)
    store.save_profile(props, url)
    runs = _every_run(pages, props)
    result = Result(name)
    _read_runs(args, pages, seasons, result, runs, lambda run: store)
    return result


def _scrape_feed(args: argparse.Namespace, pages: Pages, seasons: site.Seasons, url: str) -> Result:
    name = site.feed_name(url)
    log.info(f"feed [hero]{escape(name)}[/]: taking up to {args.max_runs} runs, scrolling it in a browser [muted]{url}[/]")
    runs = pages.open_browser().scroll_runs(url, args.max_runs)
    stores: dict[str, Store] = {}

    def store_of(run: dict) -> Store:
        profile_id = run["profileId"]
        if profile_id not in stores:
            stores[profile_id] = Store(args.out, profile_id)
        return stores[profile_id]

    result = Result(f"feed {name}")
    files = _read_runs(args, pages, seasons, result, runs, store_of)
    if args.list:
        return result
    entries = [{
        "run_id": run["id"],
        "profile_id": run.get("profileId"),
        "username": run.get("username"),
        "hero": run.get("hero"),
        "season": seasons.of(run.get("createdAt")),
        "created_at": run.get("createdAt"),
        "wins": run.get("statWins"),
        "losses": run.get("statLosses"),
        "victory_tier": run.get("victoryTier"),
        "file": files[run["id"]].relative_to(args.out).as_posix() if run["id"] in files else None,
    } for run in runs]
    log.info(f"feed: {len(entries)} runs listed in [path]{write_feed(args.out, name, url, entries)}[/]")
    return result


def _runs_table(runs: list[dict]) -> Table:
    table = Table(header_style="bold")
    table.add_column("Saved at", style="muted")
    table.add_column("Player")
    table.add_column("Hero", style="hero")
    table.add_column("Wins", justify="right")
    table.add_column("Victory", style="item")
    table.add_column("Run", style="muted")
    for run in runs:
        table.add_row((run.get("createdAt") or "")[:16].replace("T", " "), escape(run.get("username") or ""),
                      escape(run.get("hero") or ""), str(run.get("statWins") or 0), escape(run.get("victoryTier") or ""),
                      site.run_url(run))
    return table


def _read_runs(args: argparse.Namespace, pages: Pages, seasons: site.Seasons, result: Result,
               runs: list[dict], store_of) -> dict[str, Path]:
    """Read and save each of `runs` not saved yet (each in `store_of(run)`), and write the index of
    every profile touched. Returns the file of every run of `runs` that is saved."""
    label = f"[hero]{escape(result.label)}[/]"
    runs = [run for run in runs if run.get("id") and run.get("profileId")]
    files = {run["id"]: found for run in runs if (found := store_of(run).find(run["id"]))}
    queue = [run for run in runs if args.refresh or run["id"] not in files]
    if args.limit:
        queue = queue[: args.limit]
    result.listed, result.had = len(runs), len(files)
    log.info(f"{label}: {len(runs)} runs, {len(files)} saved already, {len(queue)} to read")
    if args.list:
        if queue:
            console.print(_runs_table(queue))
        return files
    touched = {id(store_of(run)): store_of(run) for run in runs}
    if not queue:
        log.info(f"{label}: [saved]nothing new to read[/] (read saved runs again to redo them)")
        _write_indexes(touched.values())
        return files
    if args.interactive:
        step = prompts.next_step(result.label, len(queue))
        if step == SHOW:
            console.print(_runs_table(queue))
            step = prompts.next_step(result.label, len(queue))
        if step == CANCEL:
            raise Cancelled
        if step != START:
            result.note = "skipped"
            log.info(f"{label}: [muted]skipped[/]")
            return files

    started = time.monotonic()
    progress = Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), BarColumn(),
                        MofNCompleteColumn(), TimeElapsedColumn(), console=console)
    try:
        with progress:
            task = progress.add_task("runs", total=len(queue))
            for number, run in enumerate(queue, 1):
                run_url = site.run_url(run)
                what = (f"[item]{escape(run.get('hero') or '?')}[/] {escape(run.get('username') or '')} "
                        f"[muted]{(run.get('createdAt') or '')[:16]} ({run['id'][:8]})[/]")
                progress.update(task, description=what)
                try:
                    timeline = site.timeline(pages.flight(run_url))
                except Missing as why:
                    timeline = None
                    result.without_timeline += 1
                    log.warning(f"[skipped]no timeline[/] {what}: {escape(str(why))}; saving the summary only")
                store = store_of(run)
                files[run["id"]] = store.save_run(run, timeline, run_url, seasons.of(run.get("createdAt")))
                store.note_player(run["profileId"], run.get("username"), site.profile_url(run["profileId"]))
                result.saved += 1
                log.info(f"[saved]saved[/]    {what}: {run.get('statWins')} wins, {escape(run.get('victoryTier') or 'no victory')}")
                progress.advance(task)
                if number < len(queue):
                    _pause(args)
    finally:
        minutes = (time.monotonic() - started) / 60
        note = f", {result.without_timeline} without a timeline" if result.without_timeline else ""
        log.info(f"{label}: [saved]{result.saved} saved[/]{note} in {minutes:.1f} min, in [path]{args.out}[/]")
        result.note = f"{minutes:.1f} min"
        _write_indexes(touched.values())
    return files


def _write_indexes(stores) -> None:
    for store in stores:
        store.write_index()


if __name__ == "__main__":
    main()
