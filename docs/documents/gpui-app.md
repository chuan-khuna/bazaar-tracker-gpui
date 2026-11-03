# gpui app

The desktop window in `src/`: how it starts, what each part of the window shows, how the tracker's notices become what is drawn, and the commands, keys, themes and settings around it. The tracking itself is in the `bazaar-tracker` crate (see [modules.md](modules.md)); this document is the app on top of it. Domain words (Run, Live run, Event, Snapshot, Board, Stash, Shop odds, Widget) are as in the [glossary](../../GLOSSARY.md).

## Running it

| Command | What it does |
|---|---|
| `just run` | `cargo run`: a debug build, with its data in `<repo>/.bazaar-tracker/` |
| `just build` (or `just release`) | `cargo build --release` into `target/release/` |
| `just app` | Starts `target/release/bazaar-tracker-gpui.exe` without rebuilding, with `BAZAAR_TRACKER_DATA_DIR` set to `<repo>/.bazaar-tracker` |
| `just test` | `cargo test --workspace`; a plain `cargo test` at the root tests only the app |

The app reads no command-line options: there is no `args.rs`, and `main.rs` never looks at `std::env::args`. `just run *args` passes extra words on to `cargo run`, but the app ignores them. Its only outside input is the `BAZAAR_TRACKER_DATA_DIR` environment variable (see [data-folder.md](data-folder.md)).

Tracking runs on Windows only. On other platforms the window opens, the service reports "Tracking is only available on Windows." and the status bar says "Not tracking".

## Startup

`main.rs` does these steps in order:

1. **Data folder.** `shell::data_dir::locate()` picks `BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker/` in a debug build, else `%APPDATA%\BazaarTracker\`, and creates it. If it cannot, the app prints `bazaar-tracker: cannot create the data folder: …` and exits with code 1.
2. **Single-instance lock.** `shell::instance::acquire` takes a lock per data folder. A second copy signals the first and returns at once; the first copy brings its window forward.
3. **Logging.** `tracking::log::feed::channel()` makes the log panel's `tracing` layer, and `shell::logging::init` adds it beside the file layer. The first line is `data folder: <path>`.
4. **Settings.** `SettingsStore::load` reads `settings.toml`. An unreadable file becomes a startup error: `settings.toml could not be parsed: <why>. Using the defaults.`
5. **Service.** `Service::start(data_dir, log_path)` opens `history.db` and starts the tracker's threads. Its error, if any, joins the startup error.
6. **Window.** gpui starts with gpui-kit's `AllAssets`, calls `gpui_component::init` and `ui::bind_keys`, and opens one window, centred, 1000 × 640 (`DEFAULT_WINDOW_SIZE`), at least 820 × 520 (`MIN_WINDOW_SIZE`), holding `TrackerApp`. Closing the window quits the app.
7. **Shutdown.** When gpui returns, `drop(service)` stops the `tracker-bridge` thread, which drops the `Tracker` and joins its threads.

### The single-instance lock

| Part | Value |
|---|---|
| Name | `BazaarTracker-<hash>`: a 64-bit FNV-1a of the data folder's path, lower-cased, `/` turned into `\`, trailing `\` trimmed |
| Mutex | `Local\BazaarTracker-<hash>`, created owned. `ERROR_ALREADY_EXISTS` means another copy runs |
| Event | `Local\BazaarTracker-<hash>-show`, waited on by a `single-instance` thread in the first copy |
| Second copy | Sets the event and exits; the first copy's window gets `activate_window()` |
| Mutex cannot be made | Treated as the first copy, so the player can always start the app |
| Not Windows | No lock |

So a debug build, a release build and a scratch folder never block each other: each data folder has its own name.

### What `TrackerApp::new` starts

| Work | Where it runs | What it feeds |
|---|---|---|
| `Session::load(history)` | At once | The runs list. A failure adds `Could not list the runs: …` to the startup error |
| `CatalogHandle::start(<profile>\AppData\LocalLow\Tempo Storm\The Bazaar)` | Its own `card-catalog` thread; the window waits up to 120 s (`CATALOG_PATIENCE`) | Card names (see [card-catalog-and-art.md](card-catalog-and-art.md)) |
| Log rows from `tracing` | A gpui task | `push_log` |
| Notices from the service | A gpui task | `on_notice` |
| `ArtInfo::load` and `ArtIndex::load` on `<data>/art` | At once | Card art on tiles, and the Settings overlay |
| Shop odds and item odds from `history.db` | At once | See [bazaar-db.md](bazaar-db.md) |
| Web-view captures (Windows) | A gpui task | See [bazaar-db.md](bazaar-db.md) |

## From notices to the screen

The tracker reports everything as a `Notice` over a blocking `mpsc` channel. `tracking::service` owns the `Tracker` on a `tracker-bridge` thread and copies each notice onto an `async_channel` (it checks its stop flag every 50 ms). The window awaits that channel, so it never blocks.

```
Tracker ─mpsc─► tracker-bridge ─async_channel─► TrackerApp::on_notice
                                                  ├─ Status::apply     → status bar connection, new errors
                                                  ├─ LiveState::apply  → Live island
                                                  ├─ Details::apply    → log panel's pinned facts
                                                  ├─ Narrator::row     → a log row
                                                  ├─ Session::apply    → runs list, selected Run's Events
                                                  └─ scroll the Events list to the bottom when watching the Live run
```

`TrackerApp::on_notice` applies a notice to each model in that order, then redraws. Every model in `src/tracking/` is plain data with no gpui, built from notices and `history.db`, so it has unit tests. `ui/` turns them into elements.

| Notice | `Status` | `Session` | `LiveState` | `Details` |
|---|---|---|---|---|
| `Connection(Connected)` | Tracking | — | — | attached: pid and details |
| `Connection(Disconnected)` | Waiting | no Run is live | board forgotten | not attached |
| `EventSaved` | clears the last error | re-reads the Event's Run; a non-ending Event makes its Run live and selects it; `RunEnded` ends the live Run | — | Last event time |
| `RunRebound` | clears the last error | swaps the old id for the new one in the list, live and selected; reloads the Events | — | — |
| `SaveFailed`, `LogError` | an error, unless it repeats the last one | — | — | — |
| `LiveSnapshot` | — | — | replaces the board | — |

`Status` keeps the last error it handed out, so the tracker repeating a failure does not bring back an error the player dismissed. `Connection` reads `Not tracking` (the service did not start), `Waiting for the game` or `Tracking`.

`Session` reads the runs from `history.db` once. After that each saved Event re-reads only its own Run's row (`History::get_run`), so the summary rules live only in the history store. Runs stay newest first. The live Run cannot be deleted, since the tracker is still saving to it.

## The window

```
┌ Bazaar Tracker ──────────────────────────────────────────── ☰ ─ □ ✕ ┐  title bar, 34 px
│ ┌ RUNS ────┐ ┌ LIVE ─────────────────────┐ ┌ SNAPSHOT ──┐ ┌ LOG ───┐ │
│ │ Vanessa  │ │ shop / choices / opponent │ │ board      │ │▾DETAILS│ │
│ │ 7W–2L    │ │ board · stash · skills    │ │ stash      │ │ TIME…  │ │
│ │ Day7 LIVE│ └───────────────────────────┘ │ skills     │ │        │ │
│ │ …        │ ┌ Run header + day buttons ─┐ │ DETAIL     │ │        │ │
│ │──────────│ │ ECONOMY        │SHOP ODDS │ │ options or │ │        │ │
│ │ WIDGETS  │ │ Events: filters + table   │ │ boards     │ │        │ │
│ └──────────┘ └───────────────────────────┘ └────────────┘ └────────┘ │
│ ▣ ● Tracking  error ✕  notice                            ▣  ▤  🌐    │  status bar, 28 px
└──────────────────────────────────────────────────────────────────────┘
```

The panels are islands: rounded, on the `surface` colour, with an 8 px gap (`GAP`) between them on the `background` colour. Each gap beside a resizable island is its resize strip: it shows `overlay` on hover and an `accent` line while dragged, and the new size is saved when the mouse button is released.

| Island | Opened by | Width or height | Saved as |
|---|---|---|---|
| Runs sidebar | Ctrl+B, status bar ▣, menu | 220–520 px wide, 248 at first (includes the gap) | `sidebar_open`, `sidebar_width` |
| Main island | always | the rest: the Live section, then the selected Run's Economy, scrolling together | — |
| Detail island | Ctrl+Alt+B, status bar, menu | 240–1200 px wide, 560 at first | `detail_width` (whether it is open is not saved: the window opens without it) |
| Log panel | Ctrl+J, status bar, menu | 260–900 px wide, 420 at first | `log_open`, `log_width` |
| Bazaar DB panel (Windows) | status bar 🌐 | — | not saved; see [bazaar-db.md](bazaar-db.md) |

### Title bar

`ui/frame/title_bar.rs` draws a flat 34 px bar in the `background` colour with "Bazaar Tracker" on the left and the ☰ button on the right. The whole bar drags the window (`WindowControlArea::Drag`); a double click maximises (or calls `titlebar_double_click` on macOS). The ☰ button `occlude()`s, so a click on it is not a drag.

| Platform | Caption |
|---|---|
| Windows | Its own minimise, maximise/restore and close buttons, 46 px wide, drawn with Segoe Fluent Icons glyphs (`\u{e921}`, `\u{e922}`/`\u{e923}`, `\u{e8bb}`); hover is `highlight_med`, close hovers in `error` |
| macOS | The system bar is transparent; the traffic lights sit at (12, 11) inside an 88 px left padding |
| Linux | Keeps its system title bar |

### Runs sidebar

`ui/frame/sidebar.rs` lists `Session::rows()`, newest first, under RUNS. Each row shows:

- the hero, or `Unknown hero`;
- the record as a badge, `7W–2L`, in `win` or `loss` once the Run has an outcome, else `subtle`;
- `Day 7` once the Run has a day;
- a solid LIVE badge in the `live` colour on the Live run;
- under them, when it started: `1 Nov 20:25` (`when::date_time`).

Clicking a row selects the Run and loads its Events. A trash button shows while the pointer is over a row (not on the Live run). The first click turns it into a red `Delete?`; the second deletes the Run and its Events from `history.db`. With no runs the list says "No runs yet — start a run in The Bazaar and it will appear here."

WIDGETS sits along the bottom of the sidebar: **Shop odds** and **Tracked items** (with the number of items tracked in the Run being played, or else the one selected). Each opens a pop-up; both are described in [bazaar-db.md](bazaar-db.md).

### Live section

`tracking::live::LiveState` keeps the latest `LiveSnapshot` board, read about once a second while the game is attached. Nothing in it is saved.

| Situation | The island shows |
|---|---|
| Game not attached | LIVE, then "Waiting for the game. What it shows appears here while it is attached." |
| Attached, no Live run or no board yet | "No run is being played right now." |
| Attached, Live run | A board section with a border in the `live` colour |

The live board section has the LIVE badge, the game's state in its own colour, and the time in the Run:

| Game state (`run_state`) | Words | Colour | Offers titled |
|---|---|---|---|
| `Encounter` | Shop | `gold` | SHOP |
| `Choice` | Encounter choice | `accent` | CHOICES |
| `Combat`, `PVPCombat` | Combat | `loss` | ON OFFER |
| `Loot` | Loot | `heal` | ON OFFER |
| anything else | the state's own name | `subtle` | ON OFFER |

For example `Shop · Day 2 Hour 1`. Under the header come the offers (items on a ten-slot row, each taking its size; an id the read has no card for is listed by its id), at an encounter choice the ITEM ODDS block ([bazaar-db.md](bazaar-db.md)), in a combat an OPPONENT box (skills over board), a rule, and then the player's own board, stash and skills. In a combat the stash is left out.

### The Run (detail island)

With no Run selected the main island says "Start a run in The Bazaar, or select one from the sidebar." under the Live section. Otherwise the detail island shows the Run, top to bottom: the header, the Events, SNAPSHOT, the selected Event and the Economy. The main island shows the same Economy under the Live section.

**Header** (`tracking::run::header`). The hero in the `heading` colour, when the Run started, then badges: the starting rank and rating (`Gold 3 · 812`), the rating change once the Run ended (`+24` in `win`, `-12` in `loss`), LIVE, Victory or Defeat, and the record. The Events' filters (Day, Hour, Kind) have a **Reset · Go live** button, which shows every Event again and the latest moment. The pop-out button sits at the top right of the Live section.

**Economy** (`tracking::run::economy`, `ui/run/economy.rs`). A line of the gold at the start of each hour: one point per hour in which a Snapshot with gold was saved. It shows once the Run has at least two such hours. The row is 70% Economy and 30% the Shop odds of three days, from the chosen day (else the Run's day) ([bazaar-db.md](bazaar-db.md)).

| Part | Rule |
|---|---|
| Gold scale | Five ticks from 0, stepping by the first of 5, 10, 25, 50 whose four gaps reach the peak. Above 200 the step stays 50 and ticks are added: a peak of 128 gives 0–200, 340 gives 0–350 |
| Days | Alternate bands, every other day shaded from the second, each reaching half an hour past its first and last hour; `D1`, `D2` … under them |
| Hours | A tick under each point |
| Chart | 106 px tall, the line in `gold`, gpui-component's `LineChart` |
| Hover | `Day 3 · Hour 2`, Gold, and Income (`+5`) when it was read |

**Events** (`tracking::run::events`, `ui/run/events.rs`). Three rows of filter buttons, then a table.

| Filter | Buttons | Keeps |
|---|---|---|
| Day | All, D1 … the last day | Events of that day |
| Hour | All, H1 … H6, and H0 first when an Event happened in hour 0 | Events of that hour |
| Kind | All, Combat, PvP, PvE | All, or only combat starts and ends (of any kind, against a player, against a monster) |

An Event with no day or hour of its own counts in the last one before it. A combat end takes PvP or PvE from its start (by `combat_start_event_id`, else the last start before it).

| Column | Width | Shows |
|---|---|---|
| TIME | 44 px | Offset into the Run: `+1:05`, past an hour `+1:02:03` |
| DAY, HOUR | 36 px each | `D2`, `H4`, bright on the first row of an hour and faint after |
| GOLD | 40 px | The gold at the start of the hour, on its first row |
| EVENT | 130 px | `Run Started`, `Options`, `Selected`, `Sold`, `Rerolled`, `Combat Start` (with a PvP badge in `burn` or PvE in `subtle`), `Combat End`, `Run Ended` |
| DETAIL | the rest | `Vanessa · Ranked`, `4 options · Choice`, `Pasta Maker (Bronze): Item`, `vs Mak (Mak)`, `Victory` in `win`, `Defeat · 7W–3L` in `loss` |

Rows are 24 px; the table is 220 px tall and draws only the rows in view (`uniform_list`). A selection or a sale is indented under the Event before it, as `↳` and a badge (Sold in `gold`, Selected in `accent`). A line marks where a new hour begins. Clicking a row chooses that Event; clicking it again lets go. While the Live run is selected and no Event is chosen, the list follows the newest Event.

### Board tiles

The same board section draws the Live island, the detail island's SNAPSHOT, and a combat's two boards. `tracking::run::board` lays the tiles out; `ui/run/board.rs` draws them.

**Placement.** The board has ten slots (`SLOTS`). A Small item covers 1, Medium 2, Large 3. An item whose `Socket_N` slots are free gets them; the rest take the first gap they fit, in order. Items that fit nowhere are counted as "N not shown". The stash has no sockets, so its items go left to right; its summary reads `4 items · 5 of 10 slots`.

**Size.** A slot is half as wide as it is tall (`SLOT_ASPECT` 0.5), so a Small tile is 0.5 h wide, Medium 1 h, Large 1.5 h. A row is never wider than ten slots of the Card size setting and scrolls sideways (with its own scroll bar) below two thirds of that:

| `card_size` | Slot width | Most row width | Least before scrolling |
|---|---|---|---|
| `small` | 48 px | 480 px | 320 px |
| `medium` (default) | 72 px | 720 px | 480 px |
| `large` | 96 px | 960 px | 640 px |

The Economy chart, the Events table and the section headers stop at the same width.

**A tile.**

| Part | From |
|---|---|
| Border | 2 px in the tier's colour: `bronze`, `silver`, `gold`, `diamond`, `legendary`; `highlight_med` if unknown |
| Art | The card's square WebP, stretched to the tile (`ObjectFit::Fill`). Without art, the name (and enchantment in `accent`) is written in the middle |
| Badges, top | Damage (`DamageAmount`), Shield (`ShieldApplyAmount`), Heal (`HealAmount`), Burn (`BurnApplyAmount`), Poison (`PoisonApplyAmount`), Regen (`RegenApplyAmount`), each when above 0; Ammo (`Ammo`, when `AmmoMax` > 0); Multicast (`x2`, when above 1). Each in its role colour, Multicast on `overlay` |
| Footer, 15 px | Sell price in `value`, the enchantment (when there is art), the cooldown from `CooldownMax`: `3s`, `4.5s`. Over art the band is black at 0xb0 alpha |
| Tooltip | The name (with its enchantment), `Bronze · Medium · Item`, `Enchantment: …`, `Tags: …`, then every other non-zero attribute as `Name: value` |
| Odds button | On a hovered item tile; see [bazaar-db.md](bazaar-db.md) |

**Slot marks.** `SocketEffect` cards are not items: each marks one slot in a 13 px row, under the player's grid and over the opponent's.

| Card | Label | Colour |
|---|---|---|
| Stove | Heat | `burn` |
| Cooler | Chill | `diamond` |
| `<letter> Note` | the letter | `legendary` |
| other | its name | `overlay` |

**Skills.** Square icons, eight to a row in two groups of four, across half the width beside a SKILLS label. Rarest first; a skill with no known tier goes last. Each has a tier border, its icon (`ObjectFit::Cover`) or else its name, and a tooltip. No skills reads `none`.

**Header stats.** On the right: `♥ 1240 / 1400` in `loss`, `● 36 (+5) gold` in `gold`, `Lv 7` in `subtle`.

### Detail island

`ui/run/detail.rs` shows two parts that scroll together.

**SNAPSHOT** of the selected Run at the chosen moment (`tracking::run::moment`). Only Options, Selected, Sold and Combat Start (the player's side) carry a Snapshot; any other Event shows the nearest one before it, so the board is never blank mid-Run.

| Choice | Snapshot | Moment badge |
|---|---|---|
| Nothing chosen, Live run | The latest | `live` |
| Nothing chosen, ended Run | The latest | `end of run` |
| An Event | The nearest at or before it | `Event #12` |
| A day | The last one on or before that day | `end of day 3` (and `Hour 6` as the time) |
| No Snapshot yet | Empty | `no board yet` |

While looking back, a **Go live** (Live run) or **Go to latest** button returns to the latest. Clicking the header folds the section (`snapshot_open`); ▾ STASH folds the stash (`stash_open`).

**DETAIL** of the chosen Event (`tracking::run::detail`), titled `Options · #12 · 21:20:11`. With none chosen: "Select an event to see its details."

| Event | Shows |
|---|---|
| Run Started | Hero, Mode, Build, Environment, Rank, Rating |
| Options | `Picked 1 of 4` or `4 offered, none picked`; OPTIONS as cards with the pick ticked ✓ |
| Selected | The Options it came from with this pick ticked, or SELECTED alone; Section, Target, Merged into |
| Sold | SOLD and the card |
| Rerolled | "The offers were refreshed." |
| Combat Start | Mode, Opponent, Hero, Rating, Level; OPPONENT board (skills above) and PLAYER board (skills below) |
| Combat End | Result, Your health, Opponent health, Record, Note; the boards of its Combat Start |
| Run Ended | Result, Record, Gold, Max health, Prestige, Level, Income, Rank, Rating; "The result of the run was not recorded." if unknown |

A card is a row with a 4 px left border in its tier colour, its name, and `Item · Bronze · <enchantment>`. An Event no longer in the Run says "This event is no longer in the run."

### Log panel

`ui/log_panel.rs` shows LOG, then a pinned block that folds (▾ DETAILS, `log_details_open`), then a table. Both use Consolas.

| Pinned fact | Before attaching | Attached |
|---|---|---|
| State | `Waiting for the game` (`The tracker is not running` if the service failed) | `Attached` |
| Process | `TheBazaar` | `TheBazaar (pid 11776)` |
| Unity | `—` | `6000.3.11` |
| Offsets | `—` | `Unity 6 x64 (unverified) · not verified` |
| Mono module | `—` | `mono-2.0-bdwgc.dll at 0x7ffac8530000` |
| Root domain | `—` | `0x2181a732d20` |
| Player.log, history.db | the paths | the paths |
| Last event | `none yet` | `21:20:11` |

Rows come from two places: every `tracing` record at info level and above (the same records as `logs/tracker.log`, through `FeedLayer`), and `Narrator`, which turns notices into sentences. A record's fields follow its message: `lost the game (pid: 42, reason: gone)`.

| Kind | Colour of the word | Example |
|---|---|---|
| `info` | `subtle` | `Connected to The Bazaar (pid 11776, Unity 6000.3.11)` |
| `warn` | `burn` | `Lost the game: the game process exited` |
| `error` | `error` | `Could not save event e1 (Sold): disk full; trying again in 4s` |
| `event` | `accent` | `Bought Pasta Maker (Bronze, Medium) for 4 gold`, `Combat ended: won against Mak (PvP)` |
| `now` | `success` | `Shop: Pasta Maker (Bronze, Medium), Fang (Bronze, Medium)` |

A `now` row is logged only when a shop, encounter choice or loot offer changes, not on every read. `Narrator` remembers up to 200 Options contexts and combats, to tell a purchase from a free pick and name a combat's opponent at its end.

`LogBuffer` keeps the latest 2,000 rows (`CAPACITY`); older rows drop off the front. The table (TIME 56 px, KIND 40 px, MESSAGE wraps) lays out only the rows in view with gpui's `list`, and follows the newest row unless the player scrolls up (`FollowMode::Tail`). New rows redraw the window only while the panel is open.

### Status bar

`ui/frame/status_bar.rs`, 28 px, flat on the `background` colour.

| Left | Right |
|---|---|
| Sidebar button; a dot (`success` when tracking, else `muted`) and the connection label; an error in `error` with ✕, kept until dismissed; how far a load of item odds has got; a notice that hides after 2 s | Detail button, log button, and on Windows the Bazaar DB panel's globe |

Notices include `Layout reset` and `Card art ready for 3310 cards`. A newer notice replaces an older one, and the older one's timer does not hide it.

## Menu and command palette

The ☰ menu and the palette (Ctrl+P) offer the same `Command`s, in the same order (`overlays/commands.rs`). Each row draws the label and, on the right, a dim value, the shortcut as gpui-kit `Kbd` keys, or a dot for a toggle that is on.

| Command | Label | Shortcut | What it does |
|---|---|---|---|
| `Settings` | Settings… | — | Opens the Settings overlay |
| `ExtractArt` | Extract card art | — | Opens the Settings overlay and starts extracting card art |
| `ToggleSidebar` | Show sidebar | Ctrl+B | Toggles the runs sidebar (dot when open) |
| `ToggleDetail` | Show detail | Ctrl+Alt+B | Toggles the detail island (dot when open) |
| `ToggleLog` | Show log | Ctrl+J | Toggles the log panel (dot when open) |
| `ToggleRunDuration` | Show run duration | — | Flips `show_run_duration`; saved, but nothing shows it yet |
| `ChangeTheme` | Change theme… | Ctrl+K Ctrl+T | Opens the theme picker; the row shows the current theme |
| `ResetLayout` | Reset layout | — | Puts `[window]` back to its defaults, closes the detail island, resizes the window to 1000 × 640 unless maximised or full screen, notes `Layout reset` |
| `Quit` | Quit | Ctrl+Q | Quits |

The menu (340 px) draws a line between groups (Settings and art; the four toggles; theme and layout; Quit) and ends with "Command palette" and its shortcut. It hangs from the ☰ button with `deferred(anchored(...))`.

The palette (360 px, centred) has a "Type a command…" field. Typing filters with `overlays::search::matches`: the query's letters must appear in the label in order, ignoring case, `é` and spaces, so `rdur` finds Show run duration and `show dt` finds Show detail and Show run duration. Up and Down move and wrap, Enter or a click runs, Backspace widens the filter, Escape closes. With no match it says "No matching commands".

## Keyboard

`ui::bind_keys` binds gpui actions. `secondary` is Ctrl on Windows and Linux and Cmd on macOS.

| Keys | Action | While an overlay is open |
|---|---|---|
| Ctrl+Q | Quit | quits |
| Ctrl+P | Open or close the command palette | closes the open overlay, opens the palette (or closes it) |
| Ctrl+K Ctrl+T | Open or close the theme picker | the same, for the picker |
| Ctrl+B | Sidebar | ignored |
| Ctrl+Alt+B | Detail island | ignored |
| Ctrl+J | Log panel | ignored |

Every other key goes to `TrackerApp::key_down`, which hands gpui's key name (`up`, `enter`, `a`) and the typed text to the open overlay's `press`. A key with Ctrl, Alt or the platform key held is not passed on. With nothing open a key goes nowhere: the window has nothing to type into.

At most one overlay is open (`overlays::Overlay`): the menu, the palette, the theme picker, the Settings overlay, or the Shop odds, item odds or Tracked items pop-up. Opening one closes the others. While one is open a transparent backdrop covers the window, and a click on it closes the overlay. Each overlay is a plain struct that takes a key and returns an outcome, so its keys are tested without a window.

| Overlay | Keys |
|---|---|
| Menu, Settings, Shop odds, item odds | Escape closes |
| Palette | Type, Backspace, Up/Down (wrap), Enter runs, Escape closes |
| Theme picker | Type, Backspace, Up/Down preview (wrap), Enter chooses, Escape restores |
| Tracked items | Type, Backspace, Up/Down (wrap), Enter tracks or untracks the row, Escape closes |

## Themes

Ten themes, one file each in `src/theme/`. The theme picker (360 px, 56 px from the top) lists them in `THEMES` order under DARK and LIGHT headings, with a "Select a theme…" field.

| Theme | Appearance | Palette | `win` | `loss` | `live` | `accent` |
|---|---|---|---|---|---|---|
| Rosé Pine (default) | Dark | rosepinetheme.com | foam | love | foam | iris |
| Catppuccin Mocha | Dark | catppuccin.com | green | red | teal | lavender |
| Flexoki Dark | Dark | stephango.com/flexoki | green | red | cyan | cyan 400 |
| Tokyo Night | Dark | folke/tokyonight.nvim | green | red | teal | purple |
| Everforest | Dark (medium) | sainnhe/everforest | green | red | aqua | blue |
| Kanagawa Wave | Dark | rebelot/kanagawa.nvim | green | red | waveAqua2 | oniViolet |
| Nord | Dark | nordtheme.com | green | red | nord7 | nord8 |
| Rosé Pine Dawn | Light | rosepinetheme.com | foam | love | foam | iris |
| Catppuccin Latte | Light | catppuccin.com | green | red | teal | lavender |
| Flexoki Light | Light | stephango.com/flexoki | green | red | cyan | cyan 600 |

The picker opens on the current theme. Typing filters by name or by `dark`/`light` (`light` finds the three light themes); each move previews that theme at once. Enter keeps it and saves `theme`; Escape or a click outside puts back the theme from before. An unknown `theme` name gives Rosé Pine.

Each `Theme` fills the same 29 roles, as `0xRRGGBB`; the UI uses only roles:

| Roles | Used for |
|---|---|
| `background` | The window, the title bar and status bar |
| `surface` | Islands, menus, pickers, tooltips |
| `overlay` | Buttons, hovered and selected rows, separators, table borders |
| `highlight_low`, `highlight_med`, `highlight_high` | Empty slots and striped rows; quiet borders and an unknown tier; `highlight_high` is not drawn yet |
| `text`, `subtle`, `muted`, `heading` | Text by weight; `heading` for the hero's name in the Run header, pop-up titles and today's odds |
| `error`, `success`, `accent` | Errors and the close button; the tracking dot, toggle dots and ✓; the caret, a chosen filter, the resize line |
| `win`, `loss`, `live` | Records, outcomes and day dots; the LIVE badge and the Live island's border |
| `bronze`, `silver`, `gold`, `diamond`, `legendary` | Tier borders; `gold` also draws the Economy line and the GOLD column |
| `damage`, `shield`, `heal`, `burn`, `poison`, `regen`, `ammo`, `value` | Tile badges and the sell price; `burn` also marks warnings, PvP and Heat |

`sync_kit_theme` copies `surface`, `text`, `muted`, `highlight_med` and `overlay` into gpui-component's theme whenever the theme changes, so its chart, tooltips and scroll bars follow. To add a theme, add a file with one `Theme`, declare it in `theme/mod.rs` and list it in `THEMES`.

## Settings overlay

Settings… opens a 480 px panel, closed by Escape, its ✕ or a click outside.

| Section | What it does |
|---|---|
| CARD SIZE | Small, Medium, Large: how big item cards and skill icons are drawn. A click saves `card_size` |
| CARD ART | `3310 cards have art · extracted 2026-10-31`, or "Not extracted yet.", or the progress (`Reading the game's card data…`, `Extracting… 3 of 40 bundles`, `Finishing…`). The button reads Extract art, Update art, or a greyed Extracting… while one runs. See [card-catalog-and-art.md](card-catalog-and-art.md) |
| Item odds | Picks heroes (and Neutral) and loads their items' odds; see [bazaar-db.md](bazaar-db.md) |

## `settings.toml`

`src/shell/settings.rs`. Every key has a default (`#[serde(default)]`), so a partial file keeps what it gives and an empty file means all defaults. Unknown keys are ignored.

| Key | Default | Meaning |
|---|---|---|
| `theme` | `"Rosé Pine"` | Theme name; unknown gives Rosé Pine |
| `log_path` | `""` | `Player.log`, if not in the usual place under `LocalLow` |
| `game_dir` | `""` | The game's install folder, for card art; empty finds it ([card-catalog-and-art.md](card-catalog-and-art.md)) |
| `run_sync` | `false` | Upload Runs to Bazaar DB; nothing is sent without a secret key either way ([upload.md](upload.md)) |
| `show_run_duration` | `false` | Saved; not shown anywhere yet |
| `card_size` | `"medium"` | `small`, `medium` or `large` |
| `[window] sidebar_open` | `true` | |
| `[window] sidebar_width` | `248` | 220–520, including the gap |
| `[window] detail_width` | `560` | 240–1200 |
| `[window] snapshot_open` | `true` | SNAPSHOT unfolded |
| `[window] stash_open` | `true` | STASH unfolded |
| `[window] odds_open` | `true` | ITEM ODDS at an encounter choice unfolded |
| `[window] log_open` | `false` | |
| `[window] log_details_open` | `true` | The log panel's pinned facts unfolded |
| `[window] log_width` | `420` | 260–900 |

| Case | What happens |
|---|---|
| No file | Defaults, and the file is written (`Load::Created`) |
| File reads | Hand-edited widths and heights out of range are clamped (`sidebar_width = 5` becomes 220) |
| File cannot be read or parsed | Defaults in memory, the status bar says why, and the file is left alone until a setting changes |
| A setting changes | Saved at once, only if something changed, by writing `settings.toml.tmp` and renaming it over the file |
| Save fails | `Could not save settings: …` in the status bar (logged only, for a dragged width) |

## Logging

`src/shell/logging.rs` writes `<data>/logs/tracker.log` through `tracing`, info level and above, with no colour codes. The first line of each launch is `Bazaar Tracker 0.1.0 started (process <pid>)`. A panic is logged as `panic: …` before the previous panic hook runs.

| Rule | Value |
|---|---|
| Rotation | When a write would take a non-empty file past 1 MiB (`MAX_BYTES`, 1,048,576 bytes) |
| Backups | 3 (`BACKUPS`): `tracker.log.1` is the newest, `.3` the oldest; the oldest is dropped |
| Across launches | The file is opened to append, so it keeps growing until it rotates |
| Rotation fails | Lines keep going to the same file rather than being lost |
| File cannot be opened | `bazaar-tracker: cannot write the log: …` on stderr; the log panel still gets every record |

The file keeps `tracing`'s own UTC timestamps.

## Times

Every time the window shows comes from `src/tracking/when.rs`, in one style ([ADR 0001](../adr/0001-one-date-and-time-style.md)): a 24-hour clock, the day before the month's short English name, local time, no system locale.

| Function | Example | Used for |
|---|---|---|
| `time_s` | `20:25:07` | Log rows, Last event, an Event's detail title |
| `date_time` | `1 Nov 20:25`; `30 Oct 2025 20:25` in another year | Runs list, Run header |
| `stamp` | `1 Nov 2026 20:25` | When shop odds were saved |
| `offset` | `+1:05`; `+1:02:03` past an hour | An Event's time into its Run; before the start counts as `+0:00` |

## gpui and its dependencies

From `Cargo.toml`:

| Crate | Version | Why |
|---|---|---|
| `gpui` = `gpui-pre` | `=0.3.9`, features `font-kit`, `windows-manifest` | The copy of gpui that gpui-kit is built on, a different crate from `gpui` on crates.io; `package = "gpui-pre"` keeps `use gpui::…` working |
| `gpui-pre-platform` | `=0.3.9`, features `font-kit`, `runtime_shaders` | Opens the window (`gpui_platform::application()`); `runtime_shaders` compiles the Metal shaders at start, so macOS builds need only the Command Line Tools |
| `gpui-base` | `=0.7.3` | gpui-kit's unstyled `Button`, and the scroll bar |
| `gpui-kit-assets` | `0.7.3` | Lucide icons (`PanelLeft`, `PanelRight`, `SquareTerminal`, `Globe`, `Trash`) and the asset source `AllAssets` |
| `gpui-component` | `0.7.3`, no default features | `Kbd` for shortcut hints, the `LineChart` of the Economy, and the theme its scroll bar and tooltips read |
| `chrono` | `0.4`, feature `clock` | Local time in `when.rs` |
| `tracing`, `tracing-subscriber` | `0.1`, `0.3` | Logging |
| `async-channel` | `2` | Notices, log rows and reports into the window |
| `serde`, `toml`, `serde_json` | | `settings.toml`, the art `manifest.json` |
| `bazaar-tracker`, `bazaardb` | path | The tracker; Bazaar DB, with the `webview` feature on Windows |

`[profile.dev.package."*"] opt-level = 2` builds every dependency optimised in debug, so `just run` does not lag while scrolling; the app's own code stays at debug settings, quick to build. `tempfile` is a dev-dependency for tests.

## Tests

Tests sit beside the code as `<name>_tests.rs`. Everything with state has them; the drawing files do not.

| Area | Tests |
|---|---|
| Startup plumbing | `shell/data_dir_tests.rs`, `instance_tests.rs` (a second launch signals the first; different names do not clash; the lock frees), `logging_tests.rs` (append, rotate, keep 3 backups, banner), `settings_tests.rs` (defaults, partial and unknown keys, unparsable file left alone, round trip, clamping), `art_tests.rs` |
| Overlays | `overlays/*_tests.rs`: the nine commands in menu order, menu equals palette, loose search, wrap-around, theme preview, choose and restore |
| Themes | `theme/theme_tests.rs`: ten themes, Rosé Pine first, dark before light, every role a colour, search by name and appearance |
| View models | `tracking/session_tests.rs`, `status_tests.rs`, `service_tests.rs`, `names_tests.rs`, `when_tests.rs`, `live_tests.rs`, `run/*_tests.rs` (header, moment, board, economy, events, detail), `log/*_tests.rs` (feed, narrate, rows: the 2,000-row buffer, details) |
| Drawing helpers | The gold ticks in `ui/run/economy.rs`; `ui/web_panel_tests.rs` |

Run them all with `just test`. `cargo test` at the root runs only the app's.
