"""The real browsers on this machine, and how to start one with a remote-debugging port so a scrape
can attach to it (`--cdp http://localhost:9222`) and work with its Cloudflare clearance.

    uv run start-browser            # the first one found: Dia on macOS, else Chrome, Edge, Brave
    uv run start-browser edge       # that one
    uv run start-browser --print    # only print the commands

Dia opens the port on its normal profile, so it has your Bazaar DB cookies, but it must be quit
first (a running Dia ignores the flag). Chrome 136 and later ignore the flag on the default profile,
and Edge and Brave are treated the same, so each is started on a profile folder of its own in
`app/scraper/` (Chrome's is `.browser-profile`, the one the scrapers launch Chrome with, so a check
passed there serves both). A new profile has no clearance: pass Bazaar DB's check in its window
once.

While the port is open any program on the machine can drive that browser, with every session in
it. Quit it and start it normally when the scrape is done.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path

DEFAULT_PORT = 9222
DEFAULT_CDP = f"http://localhost:{DEFAULT_PORT}"
START_PAGE = "https://bazaardb.gg/run"
HERE = Path(__file__).resolve().parents[2]  # app/scraper


@dataclass(frozen=True)
class RealBrowser:
    key: str  # "dia", "chrome", "edge", "brave"
    name: str
    path: Path
    # True: it opens the port on its normal profile (Dia), so it must be quit first. False: it is
    # started on a profile folder of its own, beside a copy that may already be running.
    normal_profile: bool

    def profile(self) -> Path | None:
        if self.normal_profile:
            return None
        return HERE / (".browser-profile" if self.key == "chrome" else f".browser-profile-{self.key}")

    def command(self, port: int = DEFAULT_PORT) -> list[str]:
        command = [str(self.path), f"--remote-debugging-port={port}"]
        if (profile := self.profile()) is not None:
            command += [f"--user-data-dir={profile}", "--no-first-run", "--no-default-browser-check"]
        return command + [START_PAGE]

    def shell_line(self, port: int = DEFAULT_PORT) -> str:
        """The command as it would be typed: PowerShell on Windows, sh elsewhere."""
        program, *rest = self.command(port)
        if sys.platform == "win32":
            return "& " + " ".join(f'"{part}"' if " " in part or part.startswith("--user-data-dir") else part
                                    for part in [program, *rest])
        return " ".join(_sh_quote(part) for part in [program, *rest])

    def is_running(self) -> bool:
        """Whether this browser runs already (only asked of Dia, which must be quit first)."""
        if sys.platform == "darwin":
            return subprocess.run(["pgrep", "-f", str(self.path)], capture_output=True).returncode == 0
        return False


def _sh_quote(part: str) -> str:
    if not part or any(char in part for char in " '\"$&()"):
        return "'" + part.replace("'", "'\\''") + "'"
    return part


def installed() -> list[RealBrowser]:
    """The real browsers found here, the best for scraping first."""
    found: list[RealBrowser] = []

    def add(key: str, name: str, paths: list[Path], normal_profile: bool = False) -> None:
        path = next((path for path in paths if path.is_file()), None)
        if path is not None:
            found.append(RealBrowser(key, name, path, normal_profile))

    if sys.platform == "darwin":
        apps = [Path("/Applications"), Path.home() / "Applications"]
        add("dia", "Dia", [root / "Dia.app/Contents/MacOS/Dia" for root in apps], normal_profile=True)
        add("chrome", "Google Chrome", [root / "Google Chrome.app/Contents/MacOS/Google Chrome" for root in apps])
        add("edge", "Microsoft Edge", [root / "Microsoft Edge.app/Contents/MacOS/Microsoft Edge" for root in apps])
        add("brave", "Brave", [root / "Brave Browser.app/Contents/MacOS/Brave Browser" for root in apps])
    elif sys.platform == "win32":
        roots = [Path(value) for name in ("ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA") if (value := os.environ.get(name))]
        add("chrome", "Google Chrome", [root / "Google/Chrome/Application/chrome.exe" for root in roots])
        add("edge", "Microsoft Edge", [root / "Microsoft/Edge/Application/msedge.exe" for root in roots])
        add("brave", "Brave", [root / "BraveSoftware/Brave-Browser/Application/brave.exe" for root in roots])
    else:
        for key, name, programs in (("chrome", "Google Chrome", ["google-chrome", "google-chrome-stable"]),
                                    ("edge", "Microsoft Edge", ["microsoft-edge"]),
                                    ("brave", "Brave", ["brave-browser"]),
                                    ("chromium", "Chromium", ["chromium", "chromium-browser"])):
            add(key, name, [Path(found_at) for program in programs if (found_at := shutil.which(program))])
    return found


def running_browser(url: str = DEFAULT_CDP) -> str | None:
    """What answers on the debugging port at `url` (such as `Chrome/157.0.8103.39`), if anything."""
    try:
        with urllib.request.urlopen(f"{url.rstrip('/')}/json/version", timeout=1) as reply:
            return json.loads(reply.read()).get("Browser") or "a browser"
    except (OSError, ValueError):
        return None


def port_of(url: str) -> int:
    try:
        return int(url.rstrip("/").rsplit(":", 1)[1])
    except (IndexError, ValueError):
        return DEFAULT_PORT


class NotStarted(Exception):
    """The browser could not be started, or its port never answered."""


def start(browser: RealBrowser, port: int = DEFAULT_PORT, wait_s: float = 20.0) -> str:
    """Start `browser` with the debugging port, apart from this process (it keeps running after the
    scrape), and wait for the port to answer. Returns what answers there."""
    if browser.normal_profile and browser.is_running():
        raise NotStarted(f"{browser.name} is running already and would ignore the port: quit it fully (⌘Q) first")
    if (profile := browser.profile()) is not None:
        profile.mkdir(parents=True, exist_ok=True)
    options: dict = {"stdin": subprocess.DEVNULL, "stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
    if sys.platform == "win32":
        options["creationflags"] = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    try:
        subprocess.Popen(browser.command(port), **options)
    except OSError as why:
        raise NotStarted(f"could not start {browser.name}: {why}") from why
    url = f"http://localhost:{port}"
    deadline = time.monotonic() + wait_s
    while time.monotonic() < deadline:
        if found := running_browser(url):
            return found
        time.sleep(0.5)
    raise NotStarted(f"{browser.name} started, but nothing answers on {url} after {wait_s:.0f}s "
                     "(is a copy of it already running on that profile without the port? quit it and try again)")


def hint(port: int = DEFAULT_PORT) -> str:
    """How to start a browser to attach to, for a log message."""
    found = installed()
    if not found:
        return "no Chrome, Edge, Brave or Dia found here; install one, or let the scraper launch Playwright's Chromium (--browser chromium)"
    lines = [f"start one with the debugging port (`just scraper browser`), or by hand:"]
    for browser in found:
        quit_first = "  (quit it fully first)" if browser.normal_profile else ""
        lines.append(f"  {browser.name}{quit_first}:\n    {browser.shell_line(port)}")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(prog="start-browser", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("browser", nargs="?", help="dia, chrome, edge or brave (default: the first one found)")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"the debugging port (default: {DEFAULT_PORT})")
    parser.add_argument("--print", action="store_true", help="only print the commands")
    args = parser.parse_args()

    from .log import console, escape  # rich, only for this command's own output

    found = installed()
    url = f"http://localhost:{args.port}"
    if args.print or not found:
        console.print(escape(hint(args.port)), soft_wrap=True)
        return
    if answering := running_browser(url):
        console.print(f"[saved]{escape(answering)} answers on {url} already[/]; attach with [path]--cdp {url}[/]")
        return
    picked = next((browser for browser in found if browser.key == args.browser), None) if args.browser else found[0]
    if picked is None:
        raise SystemExit(f"{args.browser} is not installed here; found: {', '.join(browser.key for browser in found)}")
    console.print(f"starting [hero]{picked.name}[/]:\n  [muted]{escape(picked.shell_line(args.port))}[/]")
    try:
        answering = start(picked, args.port)
    except NotStarted as why:
        raise SystemExit(str(why)) from None
    console.print(f"[saved]{escape(answering)} answers on {url}[/]. Pass any Bazaar DB check in its window, then scrape "
                  f"(the scrapers attach to it by themselves). Quit it when done: while the port is open any program here can drive it.")


if __name__ == "__main__":
    main()
