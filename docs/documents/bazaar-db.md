# Bazaar DB

How the app reads Bazaar DB (bazaardb.gg): the Shop odds for each day, and one item's odds at each
merchant. The reading is in `crates/bazaardb`; the app side is `src/tracking/shop_odds.rs`,
`src/tracking/item_odds.rs`, `src/ui/shop_odds.rs`, `src/ui/item_odds.rs`, `src/ui/odds_load.rs`,
`src/ui/web_panel.rs` and three overlays. The terms (Run, Event, Shop odds, Widget) are in
[GLOSSARY.md](../../GLOSSARY.md). The module tables are in [modules.md](modules.md); this document
goes into the behaviour.

## Two ways to get a reply

Bazaar DB's API wants a Cloudflare Turnstile token that only a browser can earn. The crate does
not work around that. A reply reaches it in one of two ways:

| Way | Where | What happens |
|---|---|---|
| Plain `GET` | `fetch.rs` | One honest request that names this app. Without a token the API refuses, and the crate says so (`FetchError::Blocked`). |
| Web view | `webview/` (feature `webview`) | Bazaar DB's own page loads in an Edge WebView2 view on the player's machine. Cloudflare's check passes as in any browser, the page signs its own requests, and a script copies the replies. Nothing here sends a request of its own. |

An answer saved from a browser can also be parsed directly (`shop_odds::parse_json`), which is what
Import JSON… and `just app shop-odds <file>` do.

The app depends on `bazaardb` with `features = ["webview"]` only on Windows. Elsewhere, Refresh
falls back to the plain `GET`, and item odds can not load.

## URLs

| Name | Value | Used for |
|---|---|---|
| `SHOP_ODDS_PAGE` | `https://bazaardb.gg/shop-odds` | The page the web view loads, for both shop odds and item odds |
| `SHOP_ODDS_API` | `https://bazaardb.gg/api/shop-odds` | The plain `GET` (`fetch_shop_odds`) |
| `shop_odds::API_PATH` | `/api/shop-odds` | Watched by the page script |
| `item_odds::API_PATH` | `/api/item-odds` | Watched by the page script |
| `runs::API_PATH` | `/api/run` | Watched by the page script |

The page asks for an item's odds as
`/api/item-odds?item=<Bazaar DB id>&hero=<hero>&tier=<owned tier or None>&t=<token>`. A captured
request is in [item-odds-api.md](../rea/bazaardb/item-odds-api.md). `t` is the page's signed,
short-lived token; `query::without_token` drops it from what is kept.

## The honest GET

`fetch_page(url)` sends one `GET` with `User-Agent: BazaarTracker/<version>` and a 20 s timeout
(`TIMEOUT`). `classify` decides what the answer means, in this order:

| Answer | Result |
|---|---|
| A `cf-mitigated` header, a body that is Cloudflare's challenge, or status 403 | `Blocked { status }` |
| Any other status outside 200–299 | `Status(status)` |
| 200–299 | `Ok(body)` |
| Timeout | `Timeout` |
| No network, DNS, refused connection, unreadable body | `Other(message)` |

A body is a challenge when it holds `<title>Just a moment...</title>` or
`challenges.cloudflare.com/cdn-cgi` (`is_challenge`). A challenge is `Blocked` whatever its status,
even 200.

## Shop odds

`/api/shop-odds` answers:

```json
{ "days": [ { "day": 1, "samples": 51234, "tiers": { "Bronze": 88.1, "Silver": 11.9 } } ] }
```

`parse_json` reads it into `ShopOdds { days: Vec<DayOdds> }`. Each `DayOdds` has `day`, `samples`
and `tiers: [f32; 4]` in `TIERS` order: Bronze, Silver, Gold, Diamond (shops never offer
Legendary).

- A tier missing from `tiers` is 0; tier names match ignoring case.
- A day without enough data is missing from `days`; `for_day` returns `None` for it.
- Days are sorted, and a repeated day keeps its first copy.
- `{}` is an empty `ShopOdds`.
- Text that is a challenge, or exactly `Unauthorized`, is `ParseError::Blocked`; any other text that
  is not this JSON is `ParseError::Json`.

### The built-in copy

`shop_odds::defaults()` holds the odds as the page showed them on 2026-11-01 (patch 19.4), days 1
to 16. A new `history.db` starts with them. Day 6's 1% Diamond is moved to Gold, as shops do not
offer Diamond on day 6 yet.

```
Day  Bronze Silver Gold Diamond     Day  Bronze Silver Gold Diamond
  1    97     3     0     0           9     0     35    60     5
  2    88    12     0     0          10     0     21    69    10
  3    69    31     0     0          11–15  0     21    69    10
  4    49    51     0     0          16     0     19    71    10
  5    25    75     0     0
  6     1    91     8     0
  7     1    79    20     0
  8     0    59    40     0
```

## Item odds

`/api/item-odds` answers one item's chances. Its shape was read from the page's script, so every
field is optional:

```json
{ "itemName": "Pasta Maker", "itemUrlId": "qnwx…", "hero": "Jules", "tierOffered": "Bronze",
  "days": ["1", "2", "10+"], "ownedDataByDay": [true, true, false],
  "pAnyRerollByDay": [0.15, 0.3, 0.0],
  "merchants": [ { "name": "Chris", "alwaysTier": null,
                   "days": [ { "pOfferReroll": 0.05, "pSeen": 0.4 } ] } ] }
```

`ItemOdds` keeps the request's `item`, `hero` and `tier` with the body unparsed; `table()` reads it
into an `ItemOddsTable`:

| Field | From | Meaning |
|---|---|---|
| `item_name`, `item_url_id`, `hero` | `itemName`, `itemUrlId`, `hero` | The item, its Bazaar DB id (the page's `?item=`), and the hero the odds are for |
| `tier_offered` | `tierOffered` | The tier an owned item is offered at |
| `days` | `days` | Column labels: `1`, `2`, … and a last one such as `10+`; numbers or text |
| `measured` | `ownedDataByDay` | Whether each column has enough tracked runs, truthy as JavaScript reads it |
| `any_reroll_by_day` | `pAnyRerollByDay` | "Any shop": every merchant offered that day, each visited and rerolled |
| `merchants[].days[].offer_reroll` | `pOfferReroll` | In stock at that merchant after its one reroll, 0 to 1 |
| `merchants[].days[].seen` | `pSeen` | How often the merchant is presented that day |

The table answers questions by column:

- `column_of(day)` is the column labelled `day`, else the last column when it is `N+` and
  `day >= N`. With `["1", "2", "3+"]`, day 9 is column 2 and day 0 is none.
- `is_measured(column)` is the flag; a table without flags counts every column as measured.
- `merchant_chance(name, column)` and `any_chance(column)` return `None` for an unmeasured column.
- Merchant names match through `name_key`, which keeps only letters and digits, lower-cased:
  `Chris's Cookware`, `chris's cookware` and `Chriss Cookware` are the same merchant.

`tracking::item_odds::percent` prints a chance as the page does: one decimal from 10% up, two
below, trailing zeros dropped. `0.125` is `12.5%`, `0.3` is `30%`, `0.0525` is `5.25%`.

## Runs

`/api/run` replies (runs that used a card, from a card's page) are kept as `Runs { query, body }`
until their shape is known. `query` is the request's query without `t`. Nothing in the app uses them
beyond listing them in the Bazaar DB panel.

## The web view

`webview/` holds the page script, the view, and one module per API. Reading the replies builds on
every platform; the view (`WebView`, `Rect`) is Windows only.

### `script.js`

`webview::script()` fills in `__PATHS__` (the three API paths) and `__FIND__`
(`#bazaar-tracker-find=`). The view adds the script to each page's main frame. It does two things.

**It copies replies.** It wraps `window.fetch` and `XMLHttpRequest.prototype.open`. When a request's
path is watched, the reply's text goes to the app through `window.ipc.postMessage` as
`{ url, status, body }`. The page's own request goes out and comes back unchanged.

**It finds an item.** When the address ends in `#bazaar-tracker-find=<name>`, every 250 ms it:

1. Looks for `input[placeholder^="Search for an item"]`, sets its value to the name with the native
   setter and fires an `input` event, as a person typing would.
2. Then looks for a `div[data-type="item"]` whose text matches the name (letters and digits only,
   lower-cased), clicks it and reports `picked`. The page then asks for the item's odds itself.
3. After 15 s it gives up and reports `no-search-box` (no search found) or `no-match` (no suggestion
   with that name).

The report is `{ find, outcome }`.

### Decoding

`decode(message)` turns one message into a `Capture`:

| Message | Capture |
|---|---|
| `{ find, outcome }` | `Find { name, outcome }`: `Picked`, `NoMatch`, or `NoSearchBox` for anything else |
| A reply whose path is not watched, or not JSON | `None` (dropped) |
| A reply whose body is a challenge | `Failed { api, status, why: "Bazaar DB's bot check answered instead" }` |
| A reply outside 200–299 | `Failed { api, status, why: "Bazaar DB answered HTTP <status>" }` |
| `/api/shop-odds` | `ShopOdds`, or `Failed` if it does not parse |
| `/api/item-odds` | `ItemOdds` (unparsed) |
| `/api/run` | `Runs` (unparsed) |

### Page addresses

`item_odds::page_for(item, hero, tier)` builds the shop odds page set to one item, hero and tier.
An empty item or hero is left out, and so is a tier that is empty or `None`. Values are
percent-encoded except letters, digits and `-._~`. `page_to_find(name, hero, tier)` is the same page
without the item, plus the find fragment:

```
page_for(Some("qnwx879f"), "Jules", "Silver")
  https://bazaardb.gg/shop-odds?item=qnwx879f&hero=Jules&tier=Silver
page_for(None, "Jules", "None")
  https://bazaardb.gg/shop-odds?hero=Jules
page_to_find("Pasta Maker", "Jules", "Bronze")
  https://bazaardb.gg/shop-odds?hero=Jules&tier=Bronze#bazaar-tracker-find=Pasta%20Maker
```

### The popup

gpui draws with DirectComposition, which draws above any child window. So the view can not be a
child of the app's window. `WebView::open` makes a borderless `WS_POPUP` with `WS_EX_TOOLWINDOW`
(class `BazaarDbWeb`), owned by the app's window: it stays above its owner, hides when the owner is
minimised, and has no taskbar button. It starts hidden at 1×1.

| Method | What it does |
|---|---|
| `open(owner, data_dir, url, on_capture)` | Builds the view on `url` with the script, devtools on. Each decoded message goes to `on_capture`. Browser data (cookies, Cloudflare's pass) is kept in `data_dir`, which the app sets to `<data folder>/webview`. |
| `place(rect)` | Moves the popup over `rect` of the owner's client area, in physical pixels, and shows it. |
| `hide`, `reload`, `load_url`, `open_devtools` | As named. |

Dropping a `WebView` drops the view first, then destroys its popup.

## Shop odds in the app

### What the player sees

```
┌ ECONOMY ─────────────────────────┬ SHOP ODDS ──────────┐
│ gold chart                       │ Day 4  ▓▓▓▓▓░░░░░   │
│                                  │        ●49% ●51% …  │
│                                  │ Day 5  ▓▓░░░░░░░░   │
│                                  │ Day 6  ░░░░░░░░▒    │
└──────────────────────────────────┴─────────────────────┘
```

- **Beside the Economy**: the SHOP ODDS column takes 30% of the row (`COLUMN_SHARE`), at least 180 px
  wide. It shows three days (`DAYS_AHEAD`): the chosen day, else the Run's day, else day 1, and the
  two after. Each day has a bar split by tier colour and each tier's share under it. Today's name is
  bold. A click opens the Shop odds pop-up.
- **Shop odds Widget**: listed under WIDGETS at the bottom of the sidebar. Its pop-up (460 px wide)
  lists every day, where the odds came from, and two buttons: Refresh and Import JSON…. It closes
  with Escape, its ✕ or a click outside (`overlays/shop_odds.rs`).

### Rules

`tracking::shop_odds`:

- `upcoming(odds, today)` makes one row per day. A day with data shows it. A day past the last day
  with data borrows the last day's odds (`borrowed_from`, drawn as `as D16`) and a note says
  `Shop odds only have 16 days; later days show day 16.` A gap inside the data shows "Not enough
  data".
- `percent` rounds to a whole percent and writes `<1%` for a share above 0 and under 0.5.
- `source` says `No odds saved`, `Built-in copy (11/1/2026)` when the saved odds equal the defaults,
  or `Saved <time>`.

### Refresh and import

| Action | On Windows | Elsewhere |
|---|---|---|
| Refresh | Loads `SHOP_ODDS_PAGE` in the web view (made hidden if needed) and waits 30 s (`REFRESH_PATIENCE`) for the page's `/api/shop-odds` reply. No reply in time says to open the Bazaar DB panel and tick its check. | `fetch_shop_odds` off the UI thread; usually refused, and the message says to use Import JSON…. |
| Import JSON… | Picks a file and parses it with `parse_json`. | The same. |

`store_shop_odds` refuses odds with no days and keeps the old ones. Otherwise it replaces the saved
odds in one transaction and says `Saved odds for N days.` The web view saves shop odds only while a
Refresh waits for them; odds the page loads at other times are listed as `not saved`.

## Item odds in the app

### The odds button

Every item tile has a small button at its top right, seen while the tile is hovered:

| Icon | State | A click |
|---|---|---|
| Search | No odds saved | Loads them from Bazaar DB |
| ChartPie (accent) | Odds saved | Opens the item odds pop-up |
| Loader (always shown) | Waiting for this item | As above: asks again, or opens odds saved meanwhile |

Which odds a tile asks for (`OddsContext`) depends on whose item it is:

| Where | Hero | Tier asked |
|---|---|---|
| The board, and the player's side in the detail island | The Snapshot's hero, else the Run's | The tile's tier (owned) |
| The shop or offer in the Live island, the opponent in the detail island | The same | `None` (not owned) |

A tile with no hero known, no card id, or a card that is not an item has no button.

### Loading one item

`ask_item_odds(wanted)`:

1. Stops a load of many items, if one runs: the player's own ask comes first.
2. Builds the page: `page_for(url_id, hero, tier)` if the item's Bazaar DB id is known from earlier
   odds (`ItemOddsBook::url_id`), else `page_to_find(name, hero, tier)`.
3. Loads it in the web view without showing it, sets `item_odds_pending`, and starts a 20 s timer
   (`BACKGROUND_PATIENCE`). Each ask has a number, so an old timer can not end a newer ask.
4. Shows `Loading the odds for <name>…`.

The ask fails when the find reports `no-match` or `no-search-box` for that name, when an
`/api/item-odds` reply is `Failed`, or when the 20 s pass. A failed ask opens the Bazaar DB panel
(closing any pop-up, as the view steps aside for pop-ups) and says
`Bazaar DB needs you: pick <name> on its page`. The panel shows the item being waited for.

### Which reply is saved

Every `/api/item-odds` reply goes to `take_item_odds`, which saves it only if
`tracking::item_odds::accept` says it is for the pending item:

- The reply's name matches the wanted name by `name_key`, **or** its Bazaar DB id equals the one
  already known for the item. Otherwise it is refused (`Bazaar DB sent the odds for X, not Y`) and
  the ask keeps waiting: the player may be looking at other items on the page.
- The odds are saved under the hero and tier they are for, which the player may have changed on the
  page. If either differs from what was asked, a note says so: `Saved the odds for Pasta Maker, but
  they are for Vanessa, not Jules and they are for not owned, not Bronze.`
- The template id is lower-cased; the Bazaar DB id is `itemUrlId`, else the request's `item`.

A saved reply ends the ask, so its timer does not open the panel.

### The item odds pop-up

`overlays/item_odds.rs` closes on Escape. `table_view` builds it, 680 px wide:

```
Item odds · Pasta Maker                                       ✕
Jules · owned Bronze · offered at Silver                      ⟳
Saved 1 Nov 2026 20:40
Merchant          Day 1     2     3+
Any shop          15%     30%     --
  presented as an option that day
Chris             5%     12.5%    --
  always Silver
```

"Any shop" comes first when the reply has it, then each merchant with `always <tier>` under it when
it has one. Unmeasured cells are `--`. The column of the day being looked at is highlighted. ⟳ loads
the odds again.

### ITEM ODDS at an encounter choice

At an encounter choice (run state `Choice`), the Live island lists the cards on offer. Those names
are the merchants (`LiveView::merchants`). Under them, the ITEM ODDS table shows the chance of each
watched item (a row) at each merchant (a column) today, `—` where the merchant does not carry it:

```
▾ ITEM ODDS  BEST SHOPS  Day 5
ITEM            Chris    Laurel   Kina
Pasta Maker ✕   12.5%      —      3%
Wok (B) ✕          —       —      4%
```

- `watched(owned, tracked, hidden)`: each item owned, once per tier owned, then each tracked item not
  owned, as `None`, less the hidden ones. An owned item that is also tracked is watched only at its
  owned tier.
- What the odds are for: an owned item (board or stash) at its tier is the chance to find it to
  **upgrade**; a tracked item not owned is the chance to find it to **buy**.
- Odds the watched items need and do not have are loaded on their own (`load_watched_odds`, on each
  board read): one by one in the background, with the same waits as a load from Settings, each item
  asked for once while the app runs. Nothing starts while another load or ask is under way; items
  a stopped load did not reach are asked for again later.
- `choice_groups(book, hero, watched, merchants, day, any_shop)`: for each merchant Bazaar DB knows by that
  name, each watched item with saved odds for that hero and tier and a measured chance for the
  day's column. "Any shop" comes last, only when `show_any_shop_odds` is on in `settings.toml`
  (Settings → ANY SHOP ODDS; off by default). A merchant on offer that Bazaar DB lists none of the
  watched items at keeps its column, all `—`; when no merchant carries any, the table says
  "None of your items at …". With no saved odds at all, the tab says so.
- `choice_grid(groups)` turns the merchants' groups into the table: a column for each merchant, a
  row for each item any of them carries, in the order first met.
- In a merchant's shop (run state `Encounter`), the tab shows that one merchant:
  `current_merchant(events)`, the encounter chosen last in the live Run.
- The tables (`odds_table`) are drawn like the Events table, no wider than a row of cards. In the
  Live island five rows show and the rest scroll under the header; the pop-out shows them all.
- An item watched at more than one tier has a row for each, its name marked with the tier's first
  letter (`Wok (B)`).
- A click on ▾ folds the block. The state is `odds_open` in `settings.toml`, open by default.
- A row's ✕ hides the item in the tracking Run and says
  `Hid <name> · show it again from Tracked items`.

### BEST SHOPS

BEST SHOPS names for each watched item the merchants likeliest to have it in stock today, from
every merchant Bazaar DB measured that day, in the same kind of table as ITEM ODDS (without ✕), a
column for each place (`best_grid`). Each cell names the merchant over its chance, so a narrow
column (in the pop-out) cuts neither short:

```
▾ ITEM ODDS  BEST SHOPS  Day 5
ITEM            1st          2nd          3rd
                Kina         Aila         Chris
Pasta Maker     30%          20%          12.5%
                Chris        Vel
Wok (B)         4%           3%           —
```

- `best_shops(book, hero, watched, day, 3)`: each watched item with saved odds for that hero and
  tier, its merchants with a measured chance for the day's column, best first, three at most. An
  item with none is left out.
- In the Live island, ITEM ODDS and BEST SHOPS are two tabs under one heading, shown whenever a Run
  is played. The chosen tab is `odds_tab` in `settings.toml` (`item_odds` by default); ▾ folds both
  (`odds_open`). A tab with nothing to show says so.

### The pop-out

The ↗ button in the Run's header opens a blurred window above the game (`ui/pop_out.rs`). A chip
for each of Economy, Shop odds, Item odds and Best shops shows or hides it; the choice is
`[pop_out]` in `settings.toml`, all on by default. Settings → WIDGET sets how opaque its background
is (`opacity`, 20–100%, 60 by default; at least 10 when edited by hand) and whether it is frosted
glass (`frost`: blurred behind, with a pale rim; on by default).

### The Tracked items pop-up

The Tracked items Widget in the sidebar shows how many items are tracked. Its pop-up (420 px)
works on the **tracking Run**: the Run being played, else the one selected.

```
Find an item to track…
Tracked in this run, as Jules. Odds are for an item not owned.
TRACKED
  Pasta Wok                          ◔   ✕
HIDDEN
  Anvil                                 Show
ITEMS
  Pasta Maker                        + Track
```

- `candidates(items, tracked, hidden, query, 40)`: tracked items first, in the order added; then the
  hidden ones (a tracked item that is hidden is listed here); then up to 40 catalog items whose name
  contains the query by `name_key`. Without a query, only tracked and hidden items are listed.
- Keys (`overlays/tracked_items.rs`): typing filters and goes back to the first row, Backspace
  removes a letter, Up and Down wrap around, Enter picks the highlighted row, Escape closes. Control
  characters are ignored.
- Picking a hidden row shows the item again. Picking any other row tracks or untracks it.
- A newly tracked item with no not-owned odds saved for the Run's hero has them loaded at once.
- A tracked row has an odds button, as on a tile.

The hero is the game's while the Run is live, else the Run's saved hero. Without one, the subtitle
says odds can not load.

## Loading many items

Settings → ITEM ODDS loads the odds of many items, one at a time, through the hidden web view.

```
ITEM ODDS
[Dooley] [Jules] [Mak] [Pygmalien] [Stelle] [Vanessa] [Neutral]
Item odds 12/340 · Pasta Maker (Jules) · 3 skipped              [Stop]
```

- The hero buttons are the heroes the catalog's items name, sorted, without `Common` (`heroes`).
  Neutral is on by default. The choice lasts while the app runs.
- `load_queue(items, heroes, neutral, book)`: for each picked hero in turn, the hero's own items,
  then (with Neutral) the `Common` items the hero does not already have. Every item is asked as not
  owned. Items with not-owned odds saved for that hero are left out. Nothing left says
  `Every picked item already has its odds saved`.
- Each item goes through the same ask as one item. An item that fails or takes over 20 s is skipped
  and logged; the panel does not open.
- Between items the load waits a random 0.5 to 4 s (`GAP_MIN`, `GAP_MAX`), picked anew each time, so
  Bazaar DB is asked at a pace a person could keep. The last item ends the load at once.
- The status bar shows `Item odds <n>/<total> · <name> (<hero>) · <k> skipped` with a ✕ that stops
  it. Stopping keeps what is saved: `Stopped loading item odds: 12 saved, 3 skipped`.
- The end says `Loaded item odds: N saved, K skipped`. If the web view itself can not load a page,
  the load ends with an error, as every item would fail the same way.

## The Bazaar DB panel

`ui/web_panel.rs` is a debug panel, 560 px wide, opened with the globe in the status bar. It shows
Bazaar DB's page in the web view and the replies the page gets.

- The web view is made the first time it is wanted (the panel opens, a Refresh, or an item ask) and
  kept while the panel is shut, so the page and its Cloudflare pass are not lost. If it can not be
  made, the panel shows why and every ask fails with that reason.
- Each frame, a canvas places the view over the panel's body with `WebView::place`, in physical
  pixels. While any overlay is open, the view hides, as it would draw over it.
- It keeps the newest 20 replies (`KEPT`) and lists 6: `ok` or `failed`, what was asked
  (`item-odds · Jules · None · qnwx…`, `find · Pasta Maker`), the size or why it failed, and the
  start of an unparsed body (160 characters). Saved replies end in ` · saved`, others in
  ` · not saved`.
- While item odds are waited for, a line names the item, hero and tier to pick on the page.
- The header has Reload and DevTools buttons.

## Where it is saved

All in `history.db` (see [history-db.md](history-db.md)):

| Table | Migration | Key | Holds |
|---|---|---|---|
| `shop_odds` | 3 | `day` | `samples`, `bronze`, `silver`, `gold`, `diamond`, `saved_ms`. One copy: a save deletes every row and inserts the new days in one transaction. `seed_shop_odds` saves the defaults only into an empty table. |
| `item_odds` | 4 | `(template_id, hero, tier)` | `url_id`, `item_name`, Bazaar DB's `body` as it came, `saved_ms`. Saving the same key replaces it. `tier` is `None` for not owned. |
| `tracked_items` | 5 | `(run_id, template_id)` | `added_ms`. Tracking twice changes nothing. |
| `hidden_items` | 6 | `(run_id, template_id)` | `hidden_ms`. Hiding twice changes nothing. |

Template ids are lower-cased. `tracked_items` and `hidden_items` reference `runs(id)` with
`ON DELETE CASCADE ON UPDATE CASCADE`: they follow a Run when it is rebound and go when it is
deleted. At start the app loads the shop odds (seeding the defaults first) and every item's odds
into an `ItemOddsBook`; a saved body that can not be read is left out and logged. The tracked and
hidden lists are read once per tracking Run and cached until it changes or is edited.

Without a history, the app shows the built-in shop odds and no item odds.

## `just app shop-odds`

`crates/bazaardb/examples/shop_odds.rs` prints the Shop odds as a table:

```
just app shop-odds                     # ask the API (refused without a browser token)
just app shop-odds shop-odds.json      # read an answer saved from a browser
just app shop-odds shop-odds.json --json
```

```
 Day  Samples   Bronze   Silver     Gold  Diamond
   1   120884    97.0%     3.0%     0.0%     0.0%
```

A refusal prints the reason and how to save the answer: open the page, open the developer tools
(F12) on the Network tab, reload, click the `shop-odds` request and save its Response as a `.json`
file.

## Tests

| Test file | Covers |
|---|---|
| `crates/bazaardb/src/fetch_tests.rs` | Challenge, `cf-mitigated` and 403 are `Blocked`; other statuses kept; success returns the body |
| `crates/bazaardb/src/query_tests.rs` | Paths of absolute and relative URLs; reading params; dropping `t` |
| `crates/bazaardb/src/shop_odds_tests.rs` | Day order, missing tiers and days, refusals, the defaults (days 1–16, each adds to 100 ± 1) |
| `crates/bazaardb/src/item_odds_tests.rs` | Reading the answer, `N+` columns, unmeasured columns, name matching, empty answers |
| `crates/bazaardb/src/webview/mod_tests.rs` | Each API's URL, decoding replies, refusals and challenges, find reports, the script's paths and fragment |
| `crates/bazaardb/src/webview/item_odds_tests.rs` | `page_for` and `page_to_find` addresses and encoding |
| `crates/bazaar-tracker/src/history/{shop_odds,item_odds,tracked_items,hidden_items}_tests.rs` | Saving and reading each table, replacing, seeding, following a rebound or deleted Run |
| `src/tracking/shop_odds_tests.rs` | Three days ahead, borrowed days and the note, gaps, `percent` |
| `src/tracking/item_odds_tests.rs` | The book, `accept`, `percent`, the pop-up table, `watched`, `choice_groups` (tier marks), `candidates`, `heroes`, `load_queue` |
| `src/overlays/{shop_odds,item_odds,tracked_items}_tests.rs` | The pop-ups' keys |
| `src/ui/web_panel_tests.rs` | How each reply is listed, newest first, physical pixels |

None of them touch the network or open a web view. Run them with `just app test`. On Windows the app
turns on `bazaardb`'s `webview` feature, so `just app test` also runs the `webview` tests; elsewhere run
`cargo test -p bazaardb --features webview`.
