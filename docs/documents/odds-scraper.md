# Odds scraper

How to save Bazaar DB's item odds for many items at once with the Python scraper in
`app/scraper/`, and how to start the browser it works in. The files it writes have the fields of
the tracker's saved item odds, so the tracker can import them. How the app itself reads Bazaar DB
is in [bazaar-db.md](bazaar-db.md).

## What it needs

| Need | Why | How |
|---|---|---|
| `uv` and `just` | Runs the scraper and its recipes | Installed by `mise` (see `mise.toml`) |
| Python dependencies | Playwright, questionary, rich | `just scraper sync`, once |
| The tracker's `items.json` | The list of items, keyed by the template ids the tracker uses | Run the app once (`just app run`); it writes `.bazaar-tracker/items.json` when it loads the card catalog. Or pass `--items <path>`. |
| A Chromium browser | Bazaar DB's API wants a Cloudflare Turnstile token only a real browser earns | Google Chrome, Dia, or Playwright's Chromium (below) |

## Start the browser

The scraper works in a browser in one of two ways:

| Way | Option | Profile used | Cloudflare |
|---|---|---|---|
| Attach to a browser you started | `--cdp http://localhost:9222` | That browser's own | Uses the browser's existing clearance; a check, if any, is done in its window |
| Let the scraper launch one | none (default), or `--browser chromium` | `app/scraper/.browser-profile`, kept between runs | Often stops a browser Playwright launched; do the check in the window when it asks |

Attaching is the dependable way. Start the browser with a remote-debugging port, then check the
port answers before scraping.

### Dia (macOS)

Quit Dia fully (⌘Q), then start it from a terminal:

```sh
/Applications/Dia.app/Contents/MacOS/Dia --remote-debugging-port=9222
```

Dia 1.56 (Chromium 157) opens the port on its normal profile, so the scrape has your Bazaar DB
cookies, including `cf_clearance`.

### Google Chrome

Chrome 136 and later ignore `--remote-debugging-port` on the default profile, so give it another
profile folder. The scraper's own folder is a good choice: a check passed there also serves the
launched mode later. Quit any Chrome that already uses that folder first.

macOS:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --remote-debugging-port=9222 --user-data-dir="$PWD/app/scraper/.browser-profile"
```

Windows (PowerShell, from the repo root):

```powershell
& "C:\Program Files\Google\Chrome\Application\chrome.exe" `
  --remote-debugging-port=9222 --user-data-dir="$PWD\app\scraper\.browser-profile"
```

A new profile starts with no Cloudflare clearance: open <https://bazaardb.gg/shop-odds> in that
window once and pass any check before scraping.

### Check the port

```sh
curl -s http://localhost:9222/json/version
```

An answer names the browser (`"Browser": "Chrome/157…"`). No answer means the browser was already
running when the flag was given, or (Chrome) it was started on its default profile.

While the port is open, any program on the machine can drive the browser, with every logged-in
session in it. Quit and start it normally when the scrape is done.

## Scrape

From the repo root:

```sh
just scraper scrape
```

With no arguments in a terminal the scraper picks the browser itself (it attaches to the one on
`localhost:9222` when that answers, else launches Chrome with its own profile, and logs which) and
asks only:

1. Which characters, with each one's item count.
2. Only items not scraped yet? No asks again for odds already saved (`--refresh`).
3. Scrape the skipped list too? Only when `odds/skipped.json` has entries (`--retry-skipped`).
4. Retries per failed item (`--retries`, 1 by default).

Then it shows how many items each character will ask for, with Start, Show every item first, or
Cancel. Every owned tier is scraped; the other options (such as `--item`, `--limit`, `--tier`)
come from the flags, so `just scraper scrape -i --item Feast` asks the questions for Feast alone.
Ctrl+C or Esc on any question stops before anything is asked for.

With arguments it runs straight away:

```sh
just scraper scrape --cdp http://localhost:9222 --hero Jules
just scraper scrape --cdp http://localhost:9222 --hero Jules --no-neutral --limit 20 --retries 2
just scraper scrape --cdp http://localhost:9222 --hero Jules --tier Gold   # only the Gold-owned odds
just scraper scrape -i --hero Jules        # ask the questions, with Jules ticked
```

| Option | Default | What it does |
|---|---|---|
| `-i`, `--interactive` | asked when there are no arguments | Ask the questions, with the other options as the defaults |
| `--cdp URL` | launch a browser | Attach to the browser at `URL` and scrape in a new tab of it; `--browser`, `--headless` and `--profile` are then not used |
| `--hero NAME` | every hero | A hero, as the catalog names it; repeat for more |
| `--tier TIER` | every owned tier the item has | Only this owned tier: `None`, `Bronze`, `Silver`, `Gold` or `Diamond`; repeat for more. Each is a full pass over the items, with one page load per tier, and asks for it even for items that do not have it. |
| `--no-neutral` | neutral items included | Leave out the Common items |
| `--item TEXT` | every item | Only items whose name contains `TEXT`; repeat for more |
| `--limit N` | no limit | Stop after `N` asks |
| `--refresh` | off | Ask again for odds already saved and for items skipped before |
| `--retry-skipped` | off | Ask again for the items in `odds/skipped.json` |
| `--retries N` | `1` | How many times to try a failed item again (no reply, not in the search, blocked), reloading the page each time, before keeping it in `skipped.json` (or, when blocked, stopping) |
| `--gap MIN MAX` | `0.5 4` | Seconds to wait between asks, picked at random |
| `--timeout S` | `20` | Seconds to wait for the page, a suggestion or a reply |
| `--human-wait S` | `300` | Seconds to wait for a Cloudflare check done by hand |
| `--browser` | `chrome` | When launching: `chrome` (installed Google Chrome) or `chromium` (Playwright's, `just scraper chromium` installs it) |
| `--headless` | off | When launching: hide the window (Cloudflare is likely to stop it) |
| `--profile DIR` | `app/scraper/.browser-profile` | When launching: the profile folder |
| `--out DIR` | `data/scraper/odds` | The odds folder |
| `--items FILE` | the newest `items.json` in the data folders | The tracker's item list |
| `--list` | off | Print the queue as tables, and stop |
| `--index-only` | off | Rewrite `odds/index.json`, and stop |
| `-v`, `--verbose` | off | Also log each wait between asks |

Other recipes: `just scraper list` (`--list`), `just scraper index` (`--index-only`),
`just scraper help` (every option), `just scraper chromium` (install Playwright's Chromium).

### What a scrape does

For each hero the scraper loads `https://bazaardb.gg/shop-odds?hero=<hero>` once, then for each
item, as a person would:

1. Fill the page's item search ("Search for an item…") with the item's name.
2. Wait for the suggestion with exactly that name (case, spaces and punctuation ignored) and
   click it.
3. Read the `/api/item-odds` reply the page then gets and keep it.
4. Read the buttons of the page's **Currently owned tier** group: Not owned, then every tier the
   item comes in but its highest (Feast, Gold and Diamond: Not owned and Gold; 3D Printer, only
   Diamond: Not owned alone). Click each one whose odds are not saved yet, waiting 0.5–4 s before
   each click; each click makes the page ask `/api/item-odds?…&tier=<tier>` and update its address,
   without loading again. The button already pressed sends nothing, so it is clicked last.
5. Check each reply is for that item and save it under the tier Bazaar DB answered for
   (`ownedTier`), with the buttons' tiers as the file's `owned_tiers`.
6. Remove the picked item's chip (its ⨉, "Remove <name>"), which brings the search back.
7. Wait 0.5–4 s and go on with the next item.

The page keeps the tier last clicked, so the next item's first reply can be for any tier; it is
kept if that item has that tier. An item counts as done when every tier in its `owned_tiers` is
saved for the hero (for a file from before `owned_tiers`, `None` plus every tier but the highest
of Bazaar DB's `itemTiers`).

With `--tier`, it loads the page once per hero and tier and saves only the reply to the pick.

The page is loaded again when the hero changes, for each retry of a failed item, after
Cloudflare blocks a reply, when an item is not in the search but its Bazaar DB id is known from an
earlier save, or when the search does not come back after removing a chip.

Some names have several template ids (each copy of a package); Bazaar DB knows them as one item,
so it is asked once and the answer saved for each.

The log has one coloured line per item (`saved`, `skipped`, `blocked`) above a progress bar.
A scrape of one hero with its neutral items is about 380 items. With every owned tier that is up
to four asks an item (about 3–4 s each), so expect an hour or more for a whole hero.

### Stopping and carrying on

Ctrl+C stops the scrape; the same command later carries on, because odds already saved for that
hero and every owned tier, and items skipped before, are left out of the queue. A failed item is
tried again `--retries` times; a Cloudflare block on the last try stops the scrape with a message;
run it again once the check is passed in the window.

Leave the scraper's tab alone while it runs. When attached, only that tab is closed at the end;
the browser stays open.

## What it writes

Everything goes to `data/scraper/odds/` (gitignored):

| File | What it holds |
|---|---|
| `<template_id>.json` | One item's odds: `format` 1, `template_id`, `item_name`, `url_id`, `owned_tiers` (the item's owned-tier buttons, `None` for Not owned; the app ignores it), and `odds`, one entry per hero and tier with exactly the fields of the tracker's `SavedItemOdds` (`body` is the text Bazaar DB sent). A new answer for the same hero and tier replaces the old one. |
| `index.json` | Every odds file with its template id, name, Bazaar DB id and the heroes and tiers it has. Rewritten at the end of each scrape, or by `just scraper index`. |
| `skipped.json` | Every ask that got no odds after its retries: the name, its template ids, hero, tier (`All` when the item itself could not be picked), the reason, the names the page's search offered instead, and when. Not an odds file: the index leaves it out and the tracker never reads it. An entry is removed when a later ask for it is saved. |

Files are written through a temporary file, so a stopped scrape never leaves half a file.

## Import into the app

1. In the app, open the ☰ menu or the command palette (Ctrl+P) and run **Import item odds…**.
2. Pick the `data/scraper/odds/` folder (or one `<template_id>.json` in it) and press Import.

The app reads every odds file in the folder, leaving out `index.json` and `skipped.json`
(`src/tracking/odds_import.rs`), checks each answer with `bazaardb::item_odds::parse_json`, and
saves them to `history.db` in one transaction (`History::import_item_odds`). An imported answer
replaces a saved one for the same item, hero and tier only if it is newer (a later `saved_ms`), so
importing twice changes nothing and never undoes odds saved since. The odds show at once, wherever
saved item odds are shown (the odds button on a hovered item, the ITEM ODDS block, Tracked items).
This is also the way to have item odds on macOS, where the app can not load them itself.

The status bar says `Imported N item odds from F files`, with how many were already up to date. A
file or answer that can not be read is left out, logged in `logs/tracker.log`, and counted in the
message. The app can be running while the scraper writes; import again after a later scrape to
add what is new.

## Code

| File | What it does |
|---|---|
| `cli.py` | The options, the queue, the scrape loop and its log, `scrape-odds`'s entry point |
| `prompts.py` | The interactive questions, and the check of the debugging port |
| `browser.py` | `Browser` (launch, or attach and open a tab) and `OddsPage` (search, pick, read the reply, remove the chip) |
| `catalog.py` | Reading `items.json` and picking each hero's items |
| `store.py` | The odds files, `index.json` and `skipped.json` |
| `log.py` | The rich console, its colours and the log handler |
