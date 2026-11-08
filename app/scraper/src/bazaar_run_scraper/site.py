"""Bazaar DB's run pages, and the data each is built from.

A profile page (`/run/profile/<id>`) carries the profile, its stats and its newest runs (`initialRuns`,
20 at most); a run page (`/run/<kind>/<id>`) carries the run's timeline: a snapshot of both boards
at each moment, every choice offered and taken, every sale, and every fight. Both are plain pages a
script can fetch, so the scraper asks for them over HTTP.

The run feed (`/run?…`, every player's runs with the page's filters, such as
`/run?by=wins&heroes=Jules&tracker=1`) carries no runs in its page, and a profile's runs after the
first 20 are not in its page either: both come from `/api/run`, which wants a Cloudflare Turnstile
token that only a browser earns. For those the list is opened in a browser and scrolled, and the
`/api/run` replies the page gets are read. If Cloudflare stops the plain fetches too, the scraper
fetches every page in the browser instead.
"""

from __future__ import annotations

import logging
import re
from datetime import datetime
from email.utils import parsedate_to_datetime
from urllib.parse import parse_qsl, urlparse

import httpx
from playwright.sync_api import TimeoutError as PlaywrightTimeout

from .rsc import Flight, flight_from_html

SITE = "https://bazaardb.gg"
PAGE_SIZE = 20
USER_AGENT = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) "
              "Chrome/130.0.0.0 Safari/537.36")
_PROFILE_ID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
_FLIGHT_JS = "() => (self.__next_f || []).map(piece => typeof piece[1] === 'string' ? piece[1] : '').join('')"

log = logging.getLogger("scrape-runs")


class Blocked(Exception):
    """Cloudflare stopped the fetch."""


class Missing(Exception):
    """The page has no data of the kind looked for."""


def profile_id_of(text: str) -> str:
    """The profile id in a profile's link (`https://bazaardb.gg/run/profile/<id>`), or the id itself."""
    match = _PROFILE_ID.search(text.lower())
    if not match:
        raise SystemExit(f"{text} is not a Bazaar DB profile link or id")
    return match.group(0)


def feed_of(text: str) -> str | None:
    """`text` as a run feed link (`https://bazaardb.gg/run?…`, the run list with its filters), or None
    if it is not one."""
    link = urlparse(text if "://" in text else f"https://{text}")
    if link.netloc.endswith("bazaardb.gg") and link.path.rstrip("/") == "/run":
        return f"{SITE}/run?{link.query}" if link.query else f"{SITE}/run"
    return None


def source_of(text: str) -> tuple[str, str] | None:
    """`("feed", url)` for a run feed link, `("profile", id)` for a profile link or id, else None."""
    text = text.strip().strip("\"'")
    if not text:
        return None
    if feed := feed_of(text):
        return "feed", feed
    if match := _PROFILE_ID.search(text.lower()):
        return "profile", match.group(0)
    return None


def feed_name(url: str) -> str:
    """A file name for a feed, from its filters: `heroes=Jules&by=wins` is `by-wins_heroes-Jules`."""
    pairs = sorted(parse_qsl(urlparse(url).query, keep_blank_values=True))
    name = "_".join(f"{key}-{value}" if value else key for key, value in pairs) or "all"
    return re.sub(r"[^A-Za-z0-9_.,-]+", "-", name)


def profile_url(profile_id: str) -> str:
    return f"{SITE}/run/profile/{profile_id}"


def run_url(run: dict) -> str:
    return f"{SITE}/run/{run.get('kind') or 'tracker'}/{run['id']}"


def _is_challenge(text: str) -> bool:
    return "<title>Just a moment...</title>" in text or "challenges.cloudflare.com/cdn-cgi" in text


def client(timeout_s: float) -> httpx.Client:
    """A plain HTTP client for Bazaar DB, its connections kept open between pages."""
    return httpx.Client(headers={"User-Agent": USER_AGENT}, timeout=timeout_s, follow_redirects=True)


class Http:
    """Fetches pages with a plain HTTP client. Close it when done (it is a context manager)."""

    def __init__(self, timeout_s: float) -> None:
        self.client = client(timeout_s)

    def __enter__(self) -> Http:
        return self

    def __exit__(self, *_: object) -> None:
        self.client.close()

    def flight(self, url: str) -> Flight:
        reply = self.client.get(url, headers={"Accept": "text/html"})
        html = reply.text
        if reply.is_error:
            code = reply.status_code
            if code in (403, 429, 503) and (code == 429 or _is_challenge(html) or "cloudflare" in html.lower()):
                raise Blocked(f"HTTP {code}")
            raise Missing(f"HTTP {code}")
        if _is_challenge(html):
            raise Blocked("a Cloudflare check")
        return Flight(flight_from_html(html))


class InBrowser:
    """Fetches pages in a browser tab, which can pass a Cloudflare check (done by hand in the window
    if it asks for one), and pages through a profile's runs."""

    def __init__(self, page, wait_for_human_s: float, timeout_s: float) -> None:
        self.page = page
        self.wait_for_human_s = wait_for_human_s
        self.timeout_ms = timeout_s * 1000

    def _open(self, url: str) -> None:
        self.page.goto(url, wait_until="load", timeout=self.timeout_ms)
        self._past_check()

    def _past_check(self) -> bool:
        """Wait while the tab shows Cloudflare's check (it can come up after the page loaded), for a
        person to do it in the window. Returns whether there was one."""
        try:
            on_check = self.page.title().startswith("Just a moment")
        except Exception:
            return False  # the tab is navigating: no title to read yet
        if not on_check:
            return False
        log.warning(f"Cloudflare asks for a check: do it in the browser window (waiting up to {self.wait_for_human_s:.0f}s)")
        try:
            self.page.wait_for_function("() => !document.title.startsWith('Just a moment')", timeout=self.wait_for_human_s * 1000)
        except PlaywrightTimeout as error:
            raise Blocked("the Cloudflare check was not passed") from error
        self.page.wait_for_load_state("load")
        return True

    def flight(self, url: str) -> Flight:
        self._open(url)
        return Flight(self.page.evaluate(_FLIGHT_JS))

    def scroll_runs(self, url: str, limit: int, query_has: str = "") -> list[dict]:
        """The runs a run list (a profile's, or the run feed) pages in from `/api/run` as it is
        scrolled, in the order it shows them: open `url` and scroll to the end of the list, keeping
        each page of runs the page asks for (only asks whose query has `query_has`), until `limit`
        runs came, a page came short (the list's end), or nothing more came for a while."""
        pages: list[list[dict]] = []

        def keep(response) -> None:
            path = urlparse(response.url)
            if path.path != "/api/run" or query_has not in path.query or not response.ok:
                return
            try:
                body = response.json()
            except Exception:
                return
            if isinstance(body, list):
                pages.append(body)

        self.page.on("response", keep)
        try:
            self._open(url)
            seen = idle = 0
            while idle < 5:
                self.page.wait_for_timeout(1500)
                if self._past_check():
                    idle = 0
                    continue
                got = sum(len(page) for page in pages)
                if got == seen:
                    idle += 1
                else:
                    seen, idle = got, 0
                    log.info(f"[muted]{got} runs loaded[/]")
                if got >= limit or (pages and len(pages[-1]) < PAGE_SIZE):
                    break
                self.page.mouse.wheel(0, 20000)
                self.page.evaluate("() => window.scrollTo(0, document.body.scrollHeight)")
        finally:
            self.page.remove_listener("response", keep)
        runs = [run for page in pages for run in page]
        if idle >= 5:
            log.warning(f"no more runs came after {len(runs)}; the list may have more")
        return runs[:limit]


# The seasons as the site's script had them on 2026-11-08 (Bazaar DB's season picker), for when the
# script can not be read: each season runs from its start to the next one's.
KNOWN_SEASONS = [
    ("Season 10", "Wed, 07 Jan 2026 17:16:55 GMT"), ("Season 11", "Wed, 04 Feb 2026 17:05:40 GMT"),
    ("Season 12", "Wed, 04 Mar 2026 19:05:27 GMT"), ("Season 13", "Wed, 01 Apr 2026 16:12:11 GMT"),
    ("Season 14", "Wed, 06 May 2026 16:24:57 GMT"), ("Season 15", "Wed, 03 Jun 2026 16:56:45 GMT"),
    ("Season 16", "Wed, 01 Jul 2026 16:32:42 GMT"), ("Season 17", "Wed, 05 Aug 2026 16:39:45 GMT"),
    ("Season 18", "Wed, 02 Sep 2026 16:22:48 GMT"), ("Season 19", "Wed, 07 Oct 2026 16:25:13 GMT"),
    ("Season 20", "Wed, 04 Nov 2026 17:27:55 GMT"),
]
_SEASON = re.compile(r'\{name:"Season (\d+)",startDate:"([^"]+)"\}')
_SCRIPT = re.compile(r'src="(/_next/static/chunks/[^"]+\.js)"')


class Seasons:
    """Which season a run was played in, from when it was saved. A run keeps no season of its own:
    the site's season picker has each season's start in its script, read here once."""

    def __init__(self, starts: list[tuple[int, datetime]]) -> None:
        self.starts = sorted(starts, key=lambda start: start[1])

    @classmethod
    def from_site(cls, http: httpx.Client) -> Seasons:
        try:
            found = _read_seasons(http)
        except httpx.HTTPError as why:
            log.debug(f"could not read the seasons from the site ({why})")
            found = []
        if not found:
            log.warning("could not read the seasons from the site's script; using the ones known on 2026-11-08")
            found = KNOWN_SEASONS
        return cls([(int(name.split()[-1]), parsedate_to_datetime(start)) for name, start in found])

    def of(self, created_at: str | None) -> int | None:
        """The season a run saved at `created_at` (ISO time) was played in; None before the first known."""
        if not created_at:
            return None
        when = datetime.fromisoformat(created_at)
        season = None
        for number, start in self.starts:
            if when >= start:
                season = number
        return season


def _read_seasons(http: httpx.Client) -> list[tuple[str, str]]:
    def get(url: str) -> str:
        return http.get(url).raise_for_status().text

    for script in dict.fromkeys(_SCRIPT.findall(get(f"{SITE}/run"))):
        found = [(f"Season {number}", start) for number, start in _SEASON.findall(get(SITE + script))]
        if found:
            return found
    return []


def profile(flight: Flight) -> dict:
    """The props of a profile page: `profile`, `initialRuns`, `profileStats`, `allHeroes`, …"""
    found = next(flight.find(lambda value: "initialRuns" in value and "profile" in value), None)
    if found is None:
        raise Missing("the page has no profile")
    return found


def timeline(flight: Flight) -> dict:
    """The props of a run page's timeline: `snapshots`, `events`, `combats`, `monsterNames`, `player`."""
    found = next(flight.find(lambda value: "snapshots" in value and "combats" in value and "events" in value), None)
    if found is None:
        raise Missing("the page has no timeline")
    return found
