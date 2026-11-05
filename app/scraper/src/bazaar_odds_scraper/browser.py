"""Bazaar DB's shop odds page in a real browser window, asked for one item's odds at a time.

This does what the tracker's hidden web view does (`crates/bazaardb/src/webview/script.js`): type
the item's name into the page's own search, pick the suggestion with exactly that name, and read
the `/api/item-odds` reply the page then gets. Bazaar DB's API wants a Cloudflare Turnstile token
that only a browser earns, so this uses a visible browser with a profile kept between runs; if
Cloudflare asks for a check, do it in the window and the scrape carries on.
"""

from __future__ import annotations

import time
from pathlib import Path
from urllib.parse import parse_qs, quote, urlparse

from playwright.sync_api import BrowserContext, Page, Response, sync_playwright
from playwright.sync_api import TimeoutError as PlaywrightTimeout

PAGE = "https://bazaardb.gg/shop-odds"
API_PATH = "/api/item-odds"
SEARCH_BOX = 'input[placeholder^="Search for an item"]'
SUGGESTION = 'div[data-type="item"]'
NOT_OWNED = "None"


def name_key(name: str) -> str:
    """A name with case, spaces and punctuation taken out, as `bazaardb::item_odds::name_key`."""
    return "".join(c.lower() for c in name if c.isalnum())


def page_for(item: str | None, hero: str, tier: str) -> str:
    """As `bazaardb::webview::item_odds::page_for`."""
    pairs = []
    if item:
        pairs.append(f"item={quote(item, safe='')}")
    if hero:
        pairs.append(f"hero={quote(hero, safe='')}")
    if tier and tier != NOT_OWNED:
        pairs.append(f"tier={quote(tier, safe='')}")
    return f"{PAGE}?{'&'.join(pairs)}" if pairs else PAGE


def _is_challenge(text: str) -> bool:
    return "<title>Just a moment...</title>" in text or "challenges.cloudflare.com/cdn-cgi" in text


class Blocked(Exception):
    """Cloudflare stopped the page or the API."""


class NotFound(Exception):
    """The item could not be picked on the page."""


class Reply:
    def __init__(self, url: str, body: str) -> None:
        query = parse_qs(urlparse(url).query)
        self.item = query.get("item", [""])[0]
        self.hero = query.get("hero", [""])[0]
        self.tier = query.get("tier", [""])[0] or NOT_OWNED
        self.body = body


class OddsPage:
    def __init__(self, context: BrowserContext, wait_for_human_s: float, timeout_s: float) -> None:
        self.context = context
        self.page: Page = context.pages[0] if context.pages else context.new_page()
        self.wait_for_human_s = wait_for_human_s
        self.timeout_ms = timeout_s * 1000
        self.place: tuple[str, str] | None = None  # the hero and tier the page is set to

    def _is_reply(self, response: Response) -> bool:
        return urlparse(response.url).path == API_PATH

    def _read(self, response: Response) -> Reply:
        body = response.text()
        if response.status == 403 or _is_challenge(body):
            raise Blocked(f"HTTP {response.status}")
        if not 200 <= response.status < 300:
            raise NotFound(f"Bazaar DB answered HTTP {response.status}")
        return Reply(response.url, body)

    def _wait_for_search_box(self) -> None:
        """Wait for the page, giving a person time to pass a Cloudflare check in the window."""
        try:
            self.page.wait_for_selector(SEARCH_BOX, timeout=self.timeout_ms)
            return
        except PlaywrightTimeout:
            pass
        print(f"  the page has no search box yet: if Cloudflare asks for a check, do it in the browser window "
              f"(waiting up to {self.wait_for_human_s:.0f}s)")
        try:
            self.page.wait_for_selector(SEARCH_BOX, timeout=self.wait_for_human_s * 1000)
        except PlaywrightTimeout as error:
            raise Blocked("the shop odds page did not load") from error

    def open(self, hero: str, tier: str) -> None:
        """Load the page set to `hero` and `tier`."""
        self.page.goto(page_for(None, hero, tier), wait_until="domcontentloaded")
        self._wait_for_search_box()
        self.place = (hero, tier)

    def ask(self, name: str, hero: str, tier: str, url_id: str | None) -> Reply:
        """The odds of the item called `name`, found in the page's search; or, when it can not be
        found there and its Bazaar DB id is known, by loading the page on it."""
        if self.place != (hero, tier):
            self.open(hero, tier)
        try:
            return self._pick(name)
        except NotFound:
            if not url_id:
                raise
        return self._load_on(url_id, hero, tier)

    def _pick(self, name: str) -> Reply:
        box = self.page.locator(SEARCH_BOX).first
        box.fill("")
        box.fill(name)
        wanted = name_key(name)
        deadline = time.monotonic() + self.timeout_ms / 1000
        while time.monotonic() < deadline:
            for option in self.page.locator(SUGGESTION).all():
                try:
                    if name_key(option.text_content() or "") != wanted:
                        continue
                    with self.page.expect_response(self._is_reply, timeout=self.timeout_ms) as info:
                        option.click()
                    return self._read(info.value)
                except PlaywrightTimeout as error:
                    raise NotFound(f"no odds came back for {name}") from error
            self.page.wait_for_timeout(250)
        raise NotFound(f"{name} is not in the page's search")

    def _load_on(self, url_id: str, hero: str, tier: str) -> Reply:
        try:
            with self.page.expect_response(self._is_reply, timeout=self.timeout_ms) as info:
                self.page.goto(page_for(url_id, hero, tier), wait_until="domcontentloaded")
        except PlaywrightTimeout as error:
            raise NotFound(f"no odds came back for {url_id}") from error
        self.place = (hero, tier)
        return self._read(info.value)


class Browser:
    """A visible browser with a profile kept in `profile`, so a passed Cloudflare check lasts."""

    def __init__(self, profile: Path, channel: str | None, headless: bool) -> None:
        self.profile = profile
        self.channel = channel
        self.headless = headless

    def __enter__(self) -> BrowserContext:
        self._playwright = sync_playwright().start()
        self.profile.mkdir(parents=True, exist_ok=True)
        self._context = self._playwright.chromium.launch_persistent_context(
            str(self.profile), channel=self.channel, headless=self.headless, viewport={"width": 1280, "height": 900}
        )
        return self._context

    def __exit__(self, *_: object) -> None:
        self._context.close()
        self._playwright.stop()
