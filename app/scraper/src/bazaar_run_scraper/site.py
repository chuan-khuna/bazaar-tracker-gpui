"""Bazaar DB's run pages, and the data each is built from.

A profile page (`/run/profile/<id>`) carries the profile, its stats and its newest runs (`initialRuns`,
20 at most); a run page (`/run/<kind>/<id>`) carries the run's timeline: a snapshot of both boards
at each moment, every choice offered and taken, every sale, and every fight. Both are plain pages a
script can fetch, so the scraper asks for them over HTTP.

The runs after the first 20 come from `/api/run`, which wants a Cloudflare Turnstile token that only
a browser earns; for those the profile is opened in a browser and scrolled to its end, and the
`/api/run` replies the page gets are read. If Cloudflare stops the plain fetches too, the scraper
fetches every page in the browser instead.
"""

from __future__ import annotations

import logging
import re
import urllib.error
import urllib.request
from datetime import datetime
from email.utils import parsedate_to_datetime
from urllib.parse import urlparse

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


def profile_url(profile_id: str) -> str:
    return f"{SITE}/run/profile/{profile_id}"


def run_url(run: dict) -> str:
    return f"{SITE}/run/{run.get('kind') or 'tracker'}/{run['id']}"


def _is_challenge(text: str) -> bool:
    return "<title>Just a moment...</title>" in text or "challenges.cloudflare.com/cdn-cgi" in text


class Http:
    """Fetches pages with a plain HTTP client."""

    def __init__(self, timeout_s: float) -> None:
        self.timeout_s = timeout_s

    def flight(self, url: str) -> Flight:
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "text/html"})
        try:
            with urllib.request.urlopen(request, timeout=self.timeout_s) as reply:
                html = reply.read().decode("utf-8", errors="replace")
        except urllib.error.HTTPError as error:
            body = error.read().decode("utf-8", errors="replace")
            if error.code in (403, 429, 503) and (error.code == 429 or _is_challenge(body) or "cloudflare" in body.lower()):
                raise Blocked(f"HTTP {error.code}") from error
            raise Missing(f"HTTP {error.code}") from error
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
        if _is_challenge(self.page.content()):
            log.warning(f"Cloudflare asks for a check: do it in the browser window (waiting up to {self.wait_for_human_s:.0f}s)")
            self.page.wait_for_function("() => !document.title.startsWith('Just a moment')", timeout=self.wait_for_human_s * 1000)
            self.page.wait_for_load_state("load")

    def flight(self, url: str) -> Flight:
        self._open(url)
        return Flight(self.page.evaluate(_FLIGHT_JS))

    def every_run(self, profile_id: str, run_count: int) -> list[dict]:
        """Every run the profile lists, as `/api/run` sends them: scroll the profile's run list to
        its end, keeping each page of runs the page asks for as it goes."""
        pages: list[list[dict]] = []

        def keep(response) -> None:
            path = urlparse(response.url)
            if path.path != "/api/run" or f"profileId={profile_id}" not in path.query or not response.ok:
                return
            try:
                body = response.json()
            except Exception:
                return
            if isinstance(body, list):
                pages.append(body)

        self.page.on("response", keep)
        try:
            self._open(profile_url(profile_id))
            seen = idle = 0
            while idle < 4:
                self.page.mouse.wheel(0, 20000)
                self.page.evaluate("() => window.scrollTo(0, document.body.scrollHeight)")
                self.page.wait_for_timeout(1500)
                got = sum(len(page) for page in pages)
                if got == seen:
                    idle += 1
                else:
                    seen, idle = got, 0
                    log.info(f"[muted]scrolled to {got + PAGE_SIZE} of {run_count} runs[/]")
                if got + PAGE_SIZE >= run_count:
                    break
        finally:
            self.page.remove_listener("response", keep)
        return [run for page in pages for run in page]


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
    def from_site(cls, timeout_s: float) -> Seasons:
        try:
            found = _read_seasons(timeout_s)
        except (OSError, urllib.error.URLError) as why:
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


def _read_seasons(timeout_s: float) -> list[tuple[str, str]]:
    def get(url: str) -> str:
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(request, timeout=timeout_s) as reply:
            return reply.read().decode("utf-8", errors="replace")

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
