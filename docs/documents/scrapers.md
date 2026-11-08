# Scrapers

How the Python scrapers in `apps/scraper/` get data out of Bazaar DB (bazaardb.gg), the rules both
keep, and the run scraper in full. The odds scraper's use (starting the browser, its questions and
options, importing into the app) is in [odds-scraper.md](odds-scraper.md); how the app itself
reads Bazaar DB is in [bazaar-db.md](bazaar-db.md).

## Two scrapers, one package

`apps/scraper/` is one `uv` project (`pyproject.toml`, Python 3.12+, Playwright, questionary, rich)
with two entry points:

| Command | Package | Saves | Into |
|---|---|---|---|
| `scrape-odds` (`just scraper scrape`) | `bazaar_odds_scraper` | Bazaar DB's item odds, per item, hero and owned tier | `.bazaar-tracker/raw_data/scraper/odds/` |
| `scrape-runs` (`uv run scrape-runs …` in `apps/scraper/`) | `bazaar_run_scraper` | Players' runs: the run summary and the run page's timeline | `.bazaar-tracker/raw_data/scraper/builds/` |

The run scraper reuses the odds scraper's `Browser` (launch, or attach over CDP) and its rich
console and log handler (`log.py`). `.bazaar-tracker/raw_data/scraper/` is in the app's data folder (gitignored): `raw_data/` holds the files prepared for the app to import.

## The rules both keep

- **Read only.** The scrapers only read from Bazaar DB; nothing is ever sent to it. (The app's own
  upload feature stays off: see [upload.md](upload.md).)
- **A person's pace.** Between one ask and the next each waits a random time (`--gap`: 0.5–4 s for
  odds, 1–4 s for runs), and the odds scraper drives the page's own search and buttons the way a
  person would rather than calling the API directly.
- **Carry on where it stopped.** What is already saved is left out of the queue, so Ctrl+C and
  running the same command again loses nothing. `--refresh` asks again for what is saved.
- **No half files.** Every file is written to `<name>.json.tmp` and then renamed over the real one
  (`os.replace`), so a stopped scrape never leaves a broken file.
- **Plain JSON with a `format`.** Every file has `"format": 1` and a `saved_ms` (Unix ms), and an
  `index.json` beside the saved files lists them, rewritten at the end of each scrape.
- **Cloudflare is passed by a person, not dodged.** When a check comes up the scraper waits
  (`--human-wait`, 300 s) for it to be done in the browser window, then carries on; if it is not
  passed it stops with a message, and running it again carries on.

## Where Bazaar DB's data is

Bazaar DB is a Next.js site. Its data reaches a browser in two ways, and that decides how each
scraper reads it:

| Source | What it carries | Who can read it |
|---|---|---|
| The page itself (its React Server Components "flight" payload) | The props the page was rendered with: a profile and its newest 20 runs, a run's whole timeline | Anyone: a plain HTTP `GET` |
| `/api/*` replies (`/api/item-odds`, `/api/run`) | Item odds; every run list past the page's first 20 runs, and the run feed | Only a browser: the API wants a Cloudflare Turnstile token |

So:

- The **odds scraper** works only in a browser. It opens `shop-odds?hero=<hero>`, uses the page's
  search and "Currently owned tier" buttons, and reads the `/api/item-odds` replies the page gets
  (Playwright's response events). It never builds an API request itself.
- The **run scraper** fetches pages over plain HTTP and reads their flight payload. It opens a
  browser only when it must, and from then on fetches every page there (`Pages` in `cli.py`):
  - a run feed (`/run?…`), whose runs all come from `/api/run`;
  - a profile with more than 20 runs, the rest of which the page pages in from `/api/run`;
  - a plain fetch Cloudflare stopped (HTTP 429, or 403/503 with a challenge page);
  - `--in-browser`.

  For an `/api/run` list it opens the list in the tab, scrolls to the bottom, and keeps each page
  of runs the tab is sent (20 a page). It stops when it has enough runs, when a page comes short
  (the list's end), or after five 1.5 s waits with nothing new.

## Reading a page's flight payload

`rsc.py` turns a page into the data its components got, without running its script:

1. **Collect the payload.** The page pushes it in pieces with `self.__next_f.push([1, "…"])`
   scripts. Over HTTP, `flight_from_html` finds those scripts and joins the strings; in a browser
   the same is read from `self.__next_f`.
2. **Split it into rows.** The payload is `<hex id>:<value>` rows. A value is JSON; `T<hex byte
   length>,<text>` is a long text, counted in UTF-8 bytes; a capital letter before the JSON (`I`
   an import, `HL` a hint, `E` an error) is skipped. A row that does not parse is passed over.
3. **Follow references.** A string starting with `$` points elsewhere: `$<id>` is another row,
   `$<id>:a:0:b` a part of it (React sends an object once and points at it after), `$undefined` is
   missing, `$$…` is a string starting with `$`, `$D…` a date, `$n…` a big integer. A React element
   (`["$", type, key, props]`) is followed by field name. A row that points to itself reads as
   null.
4. **Find the props by shape.** `Flight.find` walks every row for an object with the wanted keys,
   so the scraper does not depend on row ids or the page's component tree:
   - a profile page: the object with `initialRuns` and `profile`;
   - a run page: the object with `snapshots`, `combats` and `events` (its timeline).

   A page without it raises `Missing`; a run without a timeline is saved with its summary only.

## Seasons

A run keeps no season of its own; it is placed by when it was saved (`createdAt`). `Seasons` reads
each season's start from the season picker in the site's script (`/_next/static/chunks/*.js`,
`{name:"Season N",startDate:"…"}`), once per scrape. If the script cannot be read it uses the
starts known on 2026-11-08 (Seasons 10–20, in `site.KNOWN_SEASONS`) and logs a warning. A run
before the first known start goes in `season-unknown`.

## The run scraper

`just scraper runs` (`uv run scrape-runs` in `apps/scraper/`). A step-by-step guide for macOS and
Windows is in [scraping-manual.html](scraping-manual.html).

```sh
just scraper runs                      # ask what to scrape
just scraper runs https://bazaardb.gg/run/profile/829739a0-9c10-44fa-b4b7-d59138eb2ed5
just scraper runs 829739a0-9c10-44fa-b4b7-d59138eb2ed5 --list
uv run scrape-runs "https://bazaardb.gg/run?by=wins&heroes=Jules&tracker=1" --max-runs 200 --cdp http://localhost:9222
```

With no arguments in a terminal, or with `-i`, it asks (`bazaar_run_scraper/prompts.py`):

1. Which profiles and feeds saved before to scrape again (from `builds/*/metadata.json` with a
   `stats.json` beside it, and `builds/feeds/*.json`).
2. New links, pasted one or more per line until an empty line. Pasting spares the shell's quoting of
   a feed link's `?` and `&`, which `just` passes to PowerShell unquoted.
3. For a feed, how many runs from its top (`--max-runs`).
4. Only runs not saved yet? (`--refresh`)
5. The browser (below).

Then for each profile or feed, after its runs are listed: Start, Show every run first (a table),
Skip this one, or Cancel the rest. A table at the end gives each one's runs, saved before, saved
now and runs without a timeline.

### Starting a browser

`just scraper browser` (`start-browser`, `bazaar_odds_scraper/browsers.py`) looks for Dia, Chrome,
Edge and Brave in the usual install folders (`/Applications` and `~/Applications` on macOS; Program
Files, Program Files (x86) and Local AppData on Windows; `PATH` elsewhere), starts the first found,
or the one named, with `--remote-debugging-port=9222` and `bazaardb.gg/run`, apart from the
scraper's process, and waits up to 20 s for `/json/version` to answer. Dia is started on its normal
profile and refused while it is running (it would ignore the port); the others are started on a
folder of their own (Chrome's is `.browser-profile`, the one the scrapers launch Chrome with; Edge's
and Brave's are `.browser-profile-edge` and `.browser-profile-brave`). `--print` only prints the
commands, PowerShell on Windows and sh elsewhere.

Both scrapers ask about the browser with `prompts.choose_browser`: a browser answering on the port
is used without asking; otherwise it prints those commands and offers to start each browser found,
to check the port again, or to fall back (launch Chrome with the scraper's profile; for the run
scraper with profiles only, launch it only if a page needs it). Without the questions, a `--cdp`
that does not answer stops the scrape with the same commands.

Each argument is a **profile** (its link or bare id: every run of that player) or a **run feed**
(a link to the `/run` list with its filters, quoted in the shell: the first `--max-runs` runs, in
the feed's order). Starting a browser to attach to is the same as for the odds scraper; see
[odds-scraper.md](odds-scraper.md#start-the-browser).

### What a scrape does

For a profile:

1. Fetch `/run/profile/<id>` and take its props; save `metadata.json` and `stats.json`.
2. Take the runs on the page (`initialRuns`, newest 20). If `profile.runCount` is more, scroll the
   profile in the browser for the rest (only `/api/run` replies with `profileId=<id>` count).
3. Leave out the runs already saved (any season folder), unless `--refresh`; cut to `--limit`.
4. For each run, fetch `/run/<kind>/<run id>`, take its timeline, and save the run file. Wait
   `--gap` before the next.
5. Rewrite the profile's `index.json`, even when stopped.

For a feed: scroll it in the browser for up to `--max-runs` runs, read each run as above into its
own player's folder (a player only seen in feeds gets a `metadata.json` with the id and name from
the runs, and no `stats.json`), rewrite each touched player's `index.json`, then write
`feeds/<filters>.json` with the runs in the feed's order. A run is saved once however it was
found.

### Options

| Option | Default | What it does |
|---|---|---|
| `PROFILE_OR_FEED …` | asked for | One or more profile links or ids, or feed links |
| `-i`, `--interactive` | asked when there are no links | Ask the questions, with the given links and options as defaults |
| `--out DIR` | `.bazaar-tracker/raw_data/scraper/builds` | The folder to save into |
| `--max-runs N` | `100` | How many runs of a feed to take, from its top |
| `--refresh` | off | Read runs already saved again |
| `--limit N` | no limit | Read at most `N` unsaved runs of each profile or feed |
| `--list` | off | Print the runs that would be read, and stop |
| `--gap MIN MAX` | `1 4` | Seconds to wait between pages, picked at random |
| `--timeout S` | `30` | Seconds to wait for a page |
| `--in-browser` | off | Fetch every page in the browser from the start |
| `--human-wait S` | `300` | Seconds to wait for a Cloudflare check done by hand |
| `--browser` | `chrome` | When launching: `chrome` or `chromium` (Playwright's) |
| `--headless` | off | When launching: hide the window (Cloudflare is likely to stop it) |
| `--browser-profile DIR` | `apps/scraper/.browser-profile` | When launching: the profile folder |
| `--cdp URL` | launch a browser | Attach to a running browser and work in a new tab |
| `-v`, `--verbose` | off | Also log each wait between pages |

### What it writes

```text
.bazaar-tracker/raw_data/scraper/builds/
  <profile id>/
    metadata.json        id, name, run count, verified, supporter, current season
    stats.json           the profile's stats and card checklist, as Bazaar DB works them out
    index.json           every saved run, newest first: season, hero, wins, losses, victory tier,
                         rank and rating before and after, unfinished, has_timeline, file
    runs/
      season-20/
        <run id>.json    one run
      season-unknown/
  feeds/
    <filters>.json       the runs a feed listed, in its order (by-wins_heroes-Jules_tracker-1.json)
```

A run file has `run_id`, `url`, `kind`, `profile_id`, `username`, `hero`, `season`, `created_at`,
and:

- `summary`: the run as its list gives it: the end of the run (`statWins`, `statLosses`,
  `victoryTier`, `statLevel`, `statIncome`, `statGold`, `statMaxHealth`, `statPrestige`), the rank
  before and after (`playerRating`, `playerRank`, `playerRankDivision`, `…After`), the final
  `items` and `skills` with their attributes, and `combats` (`d` day, `h` hour, `o` outcome, `p`
  against a player).
- `timeline`: the run page's: `snapshots` (both boards, stash and skills at each moment), `events`
  (`selection` with `options` and `selectedCard`, `sale`, `board` pointing at a snapshot),
  `combats` (opponent's name, hero, level, rating, both sides' health after), `monsterNames` (by
  `day|hour`) and `player`. Null when the page had none.

Card art (`art`, `artBlur`, `urlFg`) and the page's `replayCta` are left out: they are most of a
file's size and of no use for analysis. If a run's season changes (for example when the site's
seasons could first not be read), the old file is removed when it is saved again.

## Code

| File | What it does |
|---|---|
| `bazaar_run_scraper/cli.py` | Options, `Pages` (HTTP until a browser is needed), profile and feed scrapes, the read loop and its progress bar |
| `bazaar_run_scraper/site.py` | Links and ids, `Http` and `InBrowser` (fetch a page, wait out a check, scroll an `/api/run` list), `Seasons`, finding a profile's and a timeline's props |
| `bazaar_run_scraper/rsc.py` | `flight_from_html` and `Flight`: the flight payload's rows and references |
| `bazaar_run_scraper/store.py` | The builds folder: run files, `metadata.json`, `stats.json`, `index.json`, feeds |
| `bazaar_run_scraper/prompts.py` | The run scraper's questions, and the look at each one's runs |
| `bazaar_odds_scraper/browsers.py` | The real browsers installed here, their start commands, starting one and waiting for its port; `start-browser` |
| `bazaar_odds_scraper/…` | The odds scraper; its `browser.Browser`, `log.py` and `prompts.choose_browser` are shared. See [odds-scraper.md](odds-scraper.md#code) |
