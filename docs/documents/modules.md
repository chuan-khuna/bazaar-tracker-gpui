# Modules

What each module does, crate by crate, and how data moves between them. The layout and the
layering rules are in [architecture.md](architecture.md); this document is the tour of what is
inside.

## How it fits together

```
 Player.log ──► log::follow ─► log::parse ─┐
                                           ├─► run::RunMachine ─► Events ─► history.db
 TheBazaar.exe ─► unityspy-rs ─► memory ───┘          │                        │
                                                      ▼                        ▼
                                          tracker::Notice ─► tracking::service ─► ui::TrackerApp
                                                                                   ▲
 bazaardb.gg ─► bazaardb::webview ──────────── shop odds ──────────────────────────┘
```

Log lines are the triggers, game memory gives the data, and the run state machine turns the two
into Events. The tracker service runs that on background threads, saves every Event to
`history.db` and reports what happened as `Notice`s. The window turns notices into view models
(`tracking/`) and draws them (`ui/`).

### One Event, end to end

1. The game writes `State changed from [...] to [ChoiceState]` to `Player.log`.
2. `log::follow` reads the new line; the `tracker-log` thread sends it to the machine thread.
3. `log::parse` makes it a `LogLine::StateChanged`; `RunMachine` starts a 500 ms settle timer.
4. When the timer fires, the machine reads the options and a Snapshot through `GameMemory` and
   produces `EventKind::OptionsPresented`.
5. The machine thread saves it through the `Store` (`History`) and sends `Notice::EventSaved`.
6. `tracking::service` forwards the notice; `TrackerApp::on_notice` updates `Session`, the log
   panel and the status, and the window redraws.

## `crates/unityspy-rs`

Generic reading of a running Unity (Mono) game's memory, ported from UnitySpy. It knows nothing
about The Bazaar and is empty on anything but Windows.

| Module | What it does |
|---|---|
| `error` | The crate's one error type. Every missing process, module, export, assembly, class or field is its own variant and names what was missing. |
| `mem::process` | Finds a process by name and opens it read-only (`PROCESS_VM_READ \| PROCESS_QUERY_LIMITED_INFORMATION`). It never writes and never asks for admin rights. |
| `mem::module` | Finds a loaded module, such as `mono-2.0-bdwgc.dll`, in the process. |
| `mem::reader` | The `Memory` trait: the one door every Mono structure is read through, so tests can use byte buffers. |
| `mono::pe` | Finds an exported function in a loaded PE module from its headers and export directory alone. |
| `mono::offsets` | Where the members of Mono's structures sit for each Unity version (64-bit Windows only). An unknown version is an error, never a guess. |
| `mono::domain` | The root domain and the assemblies loaded in it. |
| `mono::class` | Looks a class up by name and namespace in an image's class cache, lazily, and caches it. |
| `mono::field` | Class fields: name, type, offset; lookups search the parent chain. |
| `mono::statics` | Static fields, read through the class's vtable. |
| `mono::object` | Objects: the header that leads to the runtime class, and their fields. |
| `mono::value` | Primitives at their right widths, strings, arrays, references, structs and enums. Nothing is converted silently. |
| `mono::bcl` | .NET collections (`List<T>`, `Dictionary<K,V>`, `HashSet<T>`, `Nullable<T>`), with field offsets looked up by name. |
| `mono::testing` | Builders for fake Mono memory, for tests only. |

`examples/dump-classes.rs` (`just app spike`) attaches to the game and prints the classes and fields
the tracker needs.

## `crates/bazaar-tracker`

Everything Bazaar-specific with no UI. It must not depend on gpui.

### `log` — reading `Player.log`

| Module | What it does |
|---|---|
| `path` | Where the game keeps `Player.log`, or the `log_path` setting. |
| `follow` | Follows the growing file from its end; waits if it is missing and notices when the game truncates or replaces it. |
| `parse` | Turns a raw line into a `LogLine` (`StateChanged`, `SkillSelected`, `GameSimSummary`, `CardPlaced`, `CommandCompleted`, `RunInitialized`, `CombatPrepared`, `CombatCompleted`). A substring check guards each regex. The game's timestamps are dropped: Events are stamped when they are made. |

### `memory` — reading the game

| Module | What it does |
|---|---|
| `mod` | The `GameMemory` trait. A read is `Ok(None)` while the game has no value yet (normal, never an error), `Ok(Some(..))` when it has one, and `Err` only when reading failed. Also `ConnectionChange` and `AttachDetails`. |
| `game` | `BazaarMemory`, the real reader over `unityspy-rs`. All access is behind one lock. While detached it tries to attach every 500 ms; it detaches when the process dies or after five board reads in a row fail, and reports each change. |
| `snapshot` | `Snapshot`, `BoardState`, `Item`, `PlayerStats`: what the tracker knows of the board at one moment. |
| `fake` | `FakeMemory`, which returns scripted values for tests. |

`examples/snapshot.rs` prints a live Snapshot to check the reader by hand against the screen.

### `run` — the run state machine

| Module | What it does |
|---|---|
| `event` | `Run`, `Event`, `EventKind`, `Outcome`, `PresentedOption`. Events serialise to our own JSON with a `type` tag. |
| `machine` | `RunMachine`. It has no threads or timers of its own: it is fed lines (`on_raw_line`) and time (`tick`) and hands back `Output`s (`take_outputs`). Delays are timers on its `Clock`, each cancelled by a generation counter when a newer line supersedes it. An Event is built when it is sent, so its Snapshot reflects that moment. |
| `lifecycle` | Where a Run starts, ends and changes identity. A Run starts under a `local-` id; when its real id is known the machine emits `Output::Rebound`. A ranked end waits up to 4 s for the rating to change. |
| `options` | Options offered (settled after 500 ms), items and skills selected, items sold, rerolls. |
| `combat` | Captures both boards 350 ms into a combat, sends CombatStart and CombatEnd, and retries deciding the result after 1200 ms. |
| `clock` | `Clock`, `SystemClock` and `ManualClock`. The machine never reads the system clock itself. |
| `test_support` | A machine with a fake memory and a manual clock, and board builders, for tests. |

### `tracker` — the background service

| Module | What it does |
|---|---|
| `mod` | `Tracker`. `start_live` starts four threads: `tracker-log` (follows the file), `tracker-connection` (passes on attach/detach), `tracker-game` (the memory reader's reconnect loop) and `tracker-machine` (runs the machine and saves its outputs in order). A failed save is retried with a doubling delay and never dropped; later saves wait behind it. While attached it reads the live board about once a second and sends `LiveSnapshot` when it changes. Everything it reports is a `Notice`. |
| `store` | The `Store` trait the service saves through. `History` implements it. |

`examples/track.rs` (`just app track`) runs the service headless and prints each saved Event.

### `history` — `history.db`

| Module | What it does |
|---|---|
| `mod` | `History`: the `runs` and `events` tables, Event details as JSON, times in Unix milliseconds. Saving an Event updates its Run's summary in the same transaction. One connection writes and another reads, in WAL mode, so listing never waits on a write. |
| `schema` | The schema and its migrations, versioned with SQLite's `user_version`. Older files are migrated; newer ones are refused. |
| `uploads` | Which Events were uploaded and which the server refused for good (migration 2). |
| `shop_odds` | One saved copy of Bazaar DB's shop odds, for offline use (migration 3). A new file starts with the built-in odds. |
| `item_odds` | Odds for single items the player asked for, by template id, hero and owned tier, with Bazaar DB's answer kept as it came (migration 4). |
| `tracked_items` | The items tracked in each Run (migration 5). They follow the Run when it is rebound and go when it is deleted. |
| `hidden_items` | The items hidden from the odds beside the merchants in each Run (migration 6), kept the same way. |
| `pinned_items` | The items pinned for each hero (migration 7), tracked in each of the hero's Runs. Deleting a Run leaves them. |

### The rest

| Module | What it does |
|---|---|
| `catalog` | Card id → name, starting tier, size, type, description and the heroes that can get it (`Common` for neutral), from the game's `GameData.db` (read from a copy) or `cards.json`. Finding nothing leaves the catalog empty. |
| `enums` | The game's enum numbers (tier, size, hero, rank, …) and their names. Unknown numbers keep their raw value. |
| `art` | Extracts card art from the player's install into `art/<category>/<key>.webp` with a `manifest.json`. `addressables` reads the Addressables catalog (ArtKey → asset and bundle); `unity/` reads UnityFS bundles (`bundle`), serialized files and type trees (`serialized`, `common_strings`) and Texture2D pixels (`texture`). |
| `upload` | Uploading to Bazaar DB, off by default. `uploader` picks sendable Events (uploaded once, Run has a real id, Run ended or idle for six hours) and backs off on failure; `batch` sizes and compresses a batch; `transport` sends it, behind a trait; `secret` keeps the key in the Windows Credential Manager, under a separate entry in debug builds. |

## `crates/bazaardb`

Reading Bazaar DB (bazaardb.gg). Its API wants a Cloudflare Turnstile token only a browser can
earn, so a reply arrives either by a plain `GET` (usually refused) or through a real browser view.

| Module | What it does |
|---|---|
| `lib` | The page and API URLs, `fetch_shop_odds`, and the crate's `Error`. |
| `fetch` | One honest `GET` naming this app. Tells a refusal or a Cloudflare challenge apart (`FetchError::Blocked`); it does not work around them. |
| `query` | Reads the query of a request URL. |
| `shop_odds` | `ShopOdds` and `DayOdds`: per day, the percent of shop items of each tier. Parses `/api/shop-odds` and holds the built-in defaults. |
| `item_odds` | One item's odds from `/api/item-odds`: for each merchant and day, the chance it is in stock after a reroll, plus "Any shop". Every field is optional, as the shape was read from the page's script. |
| `runs` | Replies of `/api/run`, kept as they came until their shape is known. |
| `webview` (feature) | Bazaar DB's own page in an Edge WebView2 view. `script.js` copies the replies of the watched APIs and, when the address ends in `#bazaar-tracker-find=<name>`, types the name into the page's item search and picks the exact match; nothing here sends a request of its own. `view` is the borderless popup above the app's window (a child window would be drawn under gpui's DirectComposition), moved over its panel with `WebView::place`. `shop_odds`, `item_odds`, `runs` read each API's captures; `item_odds::page_for` builds the shop odds page address for one item, hero and tier. |

`examples/shop_odds.rs` (`just app shop-odds`) prints the odds, from the API or a saved answer.

## `src/` — the app

### `main.rs`

Finds the data folder, takes the single-instance lock (a second launch focuses the first window
and exits), starts logging, loads `settings.toml`, starts the `Service`, then opens the window with
`TrackerApp`. Dropping the service on exit stops every background thread.

### `shell/` — plumbing with no drawing

| Module | What it does |
|---|---|
| `data_dir` | Picks the data folder: `BAZAAR_TRACKER_DATA_DIR`, else the repo's `.bazaar-tracker/` in debug, else `%APPDATA%\BazaarTracker\`. See [data-folder.md](data-folder.md). |
| `instance` | One copy per data folder: a named mutex, and a named event the second copy sets to bring the first forward. |
| `settings` | `settings.toml`, with defaults for anything missing, saved as soon as a setting changes. An unreadable file means defaults in memory, and the file is left alone. |
| `logging` | `logs/tracker.log` through `tracing`, rotated by size. |
| `art` | Runs `bazaar_tracker::art` on a background thread and reads what it left. |
| `elevate`, `update` | Placeholders. |

### `overlays/` — what floats above the window, as state and keys

Each overlay is a plain struct that takes a key name and returns an outcome for the app to carry
out, so all of it is testable without a window.

| Module | What it does |
|---|---|
| `commands` | The commands behind the ☰ menu and the command palette (Ctrl+P). |
| `search` | The loose matching used by the palette and the theme picker. |
| `theme_picker` | Type to filter, Up/Down to preview, Enter to choose, Escape to restore. |
| `settings_overlay`, `shop_odds` | The settings overlay and the Shop odds pop-up. |
| `item_odds` | The pop-up of one item's saved odds. |
| `tracked_items` | The Tracked items pop-up: typing filters, Up/Down move, Enter tracks or untracks (or shows a hidden item again). |

### `theme/`

`mod.rs` defines `Theme`, its colour roles, and `THEMES`. Each other file maps one palette
(Rosé Pine, Catppuccin, Nord, …) onto the same roles. To add a theme, add a file and list it.

### `tracking/` — what the app knows of the tracker, with no drawing

| Module | What it does |
|---|---|
| `service` | Starts `Tracker` and bridges its blocking `mpsc` channel to an `async_channel` the window awaits, on a `tracker-bridge` thread. `Link` is what the window holds: the notices and the shared `History`. |
| `status` | The connection state and the errors the status bar shows, from the notices. |
| `session` | The Runs in the sidebar, which is live and which is selected, the selected Run's Events, and the chosen Event, day and filter. Loaded from `history.db` once; then each saved Event re-reads its own Run's row. |
| `names` | Card names from the catalog once it has loaded, and card art from the data folder. |
| `when` | Every time the app shows, in one local 24-hour style (ADR 0001). |
| `live` | `LiveState`: the latest board read, for the Live island. Never saved. |
| `shop_odds` | Which days' odds show beside the Economy (the Run's day and the next two) and in the widget. |
| `item_odds` | The saved item odds (`ItemOddsBook`), whether a reply is for the item asked about (`accept`), the pop-up's table, the chances beside each merchant at an encounter choice, grouped by merchant with "Any shop" last (`choice_groups`, over owned and tracked items from `watched`, less hidden ones), the Tracked items pop-up's rows (`candidates`: tracked, hidden, then matches), and the queue of a load of many items (`load_queue`, by hero, with the neutral items). |
| `run/header` | The facts above the board: when, who, the record, and each day. |
| `run/moment` | Which Snapshot the board shows: the chosen Event's own, or the nearest one before it. |
| `run/board` | The board, stash and skills as tiles: slot, tier, badges, prices, tooltip lines. |
| `run/economy` | Gold at the start of each hour, for the chart. |
| `run/events` | The rows of the Events list, and the day, hour and kind filters. |
| `run/detail` | What the detail island shows for one Event: the options offered, or a combat's boards. |
| `log/feed` | A `tracing` layer that hands each record to the log panel, beside the file layer. |
| `log/narrate` | Turns notices into plain sentences, naming cards from the catalog. Remembers just enough context (the shop, a sold item, a combat's opponent). |
| `log/rows` | The log panel's rows and the buffer that keeps the latest. |
| `log/details` | The facts pinned at the top of the log panel: the game, the paths, when it last saw something. |

### `ui/` — drawing

Nothing outside `ui/` draws. Its folders mirror `tracking/`.

| Module | What it draws |
|---|---|
| `mod` | `TrackerApp`, the root view. It holds the state, routes keys, and spawns the loops that take in notices, web-view captures and the loaded catalog. |
| `frame/title_bar` | The app-drawn title bar with the ☰ button and, on Windows, its own caption buttons. |
| `frame/menu`, `frame/palette` | The ☰ menu and the command palette, sharing `command_row`. |
| `frame/theme_picker`, `frame/settings_overlay` | The two pickers that float above the window. |
| `frame/sidebar` | The runs list, the widgets along its bottom, and its resize strip. |
| `frame/status_bar` | Connection state, error and notice, how far a load of item odds has got, and the detail and log buttons. |
| `run/main_panel` | The middle column: one island, the Live section then the Economy. |
| `run/header`, `run/economy`, `run/events` | The Run's header and day buttons, gold chart, and Events list, in the detail island (the chart also in the main island). |
| `run/board` | Items on the ten-slot grid with art, tier border, badges and tooltips; skills as icons. |
| `run/detail` | The detail island: the Snapshot of the chosen moment and what is specific to the Event. |
| `live` | The Live island: the shop, the options, or the opponent's board over the player's. |
| `log_panel` | The log panel, following the newest row unless scrolled up. |
| `shop_odds` | The odds column beside the Economy and the Shop odds pop-up. |
| `item_odds` | The odds button shown on a hovered item: it loads the item's odds in the hidden web view (the page script types the name into the page's search and picks it), opening the Bazaar DB panel if that fails or takes over 20 s, or opens the saved ones. Also the pop-up, the ITEM ODDS block at an encounter choice (it folds; each badge's ✕ hides the item in the Run), and the Tracked items pop-up, where items are tracked in the Run being played (else the one selected) and loaded when they have no odds. |
| `odds_load` | Loading many items' odds from the Settings overlay: the picked heroes' items (and the neutral ones) without saved odds, one at a time through the hidden web view, skipping any that fail or take over 20 s; the status bar shows the item loading. |
| `web_panel` | A debug panel with Bazaar DB's page in the web view and the replies it got. Shop odds are saved only after Refresh. Item odds are saved only for the item a tile asked for. |
| `widgets` | Small shared pieces. |

## Tests

Tests sit beside the code as `<name>_tests.rs`. The memory seams make them possible without the
game: `mono::testing` feeds byte buffers to `unityspy-rs`, and `FakeMemory` with a `ManualClock`
drives the run state machine. `crates/bazaar-tracker/tests/` replays recorded `Player.log` files
through the whole machine. Run everything with `just app test`.
