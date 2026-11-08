"""Bazaar DB's shop odds page in a real browser window, asked for one item's odds at a time.

This does what the tracker's hidden web view does (`crates/bazaardb/src/webview/script.js`): type
the item's name into the page's own search, pick the suggestion with exactly that name, and read
the `/api/item-odds` reply the page then gets. Bazaar DB's API wants a Cloudflare Turnstile token
that only a browser earns, so this uses a visible browser with a profile kept between runs; if
Cloudflare asks for a check, do it in the window and the scrape carries on.
"""

from __future__ import annotations

import logging
import time
from collections.abc import Callable
from pathlib import Path
from urllib.parse import parse_qs, quote, urlparse

from playwright.sync_api import Page, Response, sync_playwright
from playwright.sync_api import TimeoutError as PlaywrightTimeout

PAGE = "https://bazaardb.gg/shop-odds"
API_PATH = "/api/item-odds"
SEARCH_BOX = 'input[placeholder^="Search for an item"]'
SUGGESTION = 'div[data-type="item"]'
REMOVE_PICKED = 'button[aria-label^="Remove "]'  # the ⨉ on the picked item's chip
NOT_OWNED = "None"
# Ask for every owned tier the item has: the buttons of the page's "Currently owned tier" group.
ALL = "All"
TIER_BUTTONS = 'div:has(> h4:text-is("Currently owned tier")) > [role="group"] > button'
NOT_OWNED_LABEL = "Not owned"

log = logging.getLogger("scrape-odds")


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
    """The item could not be picked on the page; `suggestions` are the names the page's search
    offered instead, if it got that far."""

    def __init__(self, message: str, suggestions: list[str] | None = None) -> None:
        super().__init__(message)
        self.suggestions = suggestions or []


class Reply:
    def __init__(self, url: str, body: str) -> None:
        query = parse_qs(urlparse(url).query)
        self.item = query.get("item", [""])[0]
        self.hero = query.get("hero", [""])[0]
        self.tier = query.get("tier", [""])[0] or NOT_OWNED
        self.body = body


def tier_of(label: str) -> str:
    """The tier a "Currently owned tier" button is for, as the API and the tracker name it."""
    label = label.strip()
    return NOT_OWNED if label == NOT_OWNED_LABEL else label


class TierReplies:
    """Every owned tier the page offered for an item, and the reply for each tier that came."""

    def __init__(self) -> None:
        self.offered: list[str] = []
        self.replies: dict[str, Reply] = {}
        self.failed: dict[str, str] = {}  # the tier, and why no reply came


class OddsPage:
    def __init__(self, page: Page, wait_for_human_s: float, timeout_s: float) -> None:
        self.page = page
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
        log.warning(f"the page has no search box yet: if Cloudflare asks for a check, do it in the browser window "
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

    def ask_all(self, name: str, hero: str, url_id: str | None, have: set[str], pause: Callable[[], None]) -> TierReplies:
        """The odds of the item called `name` at every owned tier it has, on one page: pick it (as
        `ask`), keep the reply that comes, then click each other button of "Currently owned tier"
        whose tier is not in `have` (saved already), calling `pause` before each click.

        The page keeps the tier last clicked for the next item, so the first reply may be for any
        tier; it is kept if the item has that tier. A click on the button already pressed sends
        nothing, so that one is clicked last, once another click has moved the page off it."""
        if self.place is None or self.place[0] != hero:
            self.open(hero, NOT_OWNED)
        try:
            first = self._pick(name)
        except NotFound:
            if not url_id:
                raise
            first = self._load_on(url_id, hero, NOT_OWNED)
        self.place = (hero, ALL)

        result = TierReplies()
        buttons = self.page.locator(TIER_BUTTONS)
        try:
            buttons.first.wait_for(timeout=self.timeout_ms)
        except PlaywrightTimeout:
            pass  # no group: the reply that came is all there is
        labels = [label.strip() for label in buttons.all_inner_texts()]
        result.offered = [tier_of(label) for label in labels] or [first.tier]
        pressed = next((tier_of(label) for label, state in zip(labels, [b.get_attribute("aria-pressed") for b in buttons.all()])
                        if state == "true"), None)
        if first.tier in result.offered:
            result.replies[first.tier] = first
        wanted = [tier for tier in result.offered if tier not in result.replies and tier not in have]
        wanted.sort(key=lambda tier: tier == pressed)  # the pressed one last
        for tier in wanted:
            pause()
            label = labels[result.offered.index(tier)]
            try:
                with self.page.expect_response(self._is_reply, timeout=self.timeout_ms) as info:
                    buttons.nth(labels.index(label)).click()
                reply = self._read(info.value)
            except PlaywrightTimeout:
                result.failed[tier] = f"no odds came back for {name} at {tier}"
                continue
            except NotFound as why:
                result.failed[tier] = str(why)
                continue
            if reply.tier != tier:
                result.failed[tier] = f"Bazaar DB answered for {reply.tier}, not {tier}"
                continue
            result.replies[tier] = reply
        return result

    def _search_box_back(self) -> None:
        """Once an item is picked, the page swaps its item search for a chip of that item with a
        ⨉ ("Remove <name>"); remove it, as a person would, to get the search back. If the search
        still does not come back, load the page again."""
        remove = self.page.locator(REMOVE_PICKED)
        if remove.count():
            remove.first.click()
        try:
            self.page.wait_for_selector(SEARCH_BOX, timeout=self.timeout_ms)
        except PlaywrightTimeout:
            log.debug("the item search did not come back; loading the page again")
            hero, tier = self.place or ("", NOT_OWNED)
            self.open(hero, NOT_OWNED if tier == ALL else tier)
            if tier == ALL:
                self.place = (hero, ALL)

    def _pick(self, name: str) -> Reply:
        if not self.page.locator(SEARCH_BOX).count():
            self._search_box_back()
        box = self.page.locator(SEARCH_BOX).first
        box.fill("")
        box.fill(name)
        wanted = name_key(name)
        deadline = time.monotonic() + self.timeout_ms / 1000
        seen: list[str] = []
        while time.monotonic() < deadline:
            options = self.page.locator(SUGGESTION).all()
            seen = [(option.text_content() or "").strip() for option in options]
            for option, text in zip(options, seen):
                try:
                    if name_key(text) != wanted:
                        continue
                    with self.page.expect_response(self._is_reply, timeout=self.timeout_ms) as info:
                        option.click()
                    return self._read(info.value)
                except PlaywrightTimeout as error:
                    raise NotFound(f"no odds came back for {name}") from error
            self.page.wait_for_timeout(250)
        raise NotFound(f"{name} is not in the page's search", [text for text in seen if text])

    def _load_on(self, url_id: str, hero: str, tier: str) -> Reply:
        try:
            with self.page.expect_response(self._is_reply, timeout=self.timeout_ms) as info:
                self.page.goto(page_for(url_id, hero, tier), wait_until="domcontentloaded")
        except PlaywrightTimeout as error:
            raise NotFound(f"no odds came back for {url_id}") from error
        self.place = (hero, tier)
        return self._read(info.value)


class Browser:
    """The page the scrape works in: either in a visible browser launched with a profile kept in
    `profile`, so a passed Cloudflare check lasts; or, given `cdp` (such as
    `http://localhost:9222`), in a new tab of a browser already running with
    `--remote-debugging-port`, so the scrape uses that browser's own profile and Cloudflare
    clearance. An attached browser is left running: only the scrape's tab is closed."""

    def __init__(self, profile: Path, channel: str | None, headless: bool, cdp: str | None = None) -> None:
        self.profile = profile
        self.channel = channel
        self.headless = headless
        self.cdp = cdp

    def __enter__(self) -> Page:
        self._playwright = sync_playwright().start()
        if self.cdp:
            browser = self._playwright.chromium.connect_over_cdp(self.cdp)
            if not browser.contexts:
                raise SystemExit(f"the browser at {self.cdp} has no window open")
            self._context = None
            self._page = browser.contexts[0].new_page()
            return self._page
        self.profile.mkdir(parents=True, exist_ok=True)
        self._context = self._playwright.chromium.launch_persistent_context(
            str(self.profile), channel=self.channel, headless=self.headless, viewport={"width": 1280, "height": 900}
        )
        self._page = self._context.pages[0] if self._context.pages else self._context.new_page()
        return self._page

    def __exit__(self, *_: object) -> None:
        if self._context:
            self._context.close()
        else:
            try:
                self._page.close()
            except Exception:
                pass  # the tab was closed by hand, or the browser quit
        self._playwright.stop()
