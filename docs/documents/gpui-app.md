# gpui app

The desktop window in `apps/tracker/` (its `Cargo.toml`, `src/` and `macos/`): how it starts, what each part of the window shows, how the tracker's notices become what is drawn, and the commands, keys, themes and settings around it. The tracking itself is in the `bazaar-tracker` crate (see [modules.md](modules.md)); this document is the app on top of it. Domain words (Run, Live run, Event, Snapshot, Board, Stash, Shop odds, Widget) are as in the [glossary](../../GLOSSARY.md).

## Running it

| Command | What it does |
|---|---|
| `just tracker run` | `cargo run -p bazaar-tracker-gpui`: a debug build, with its data in `<repo>/.bazaar-tracker/` |
| `just tracker build` (or `just tracker release`) | `cargo build --release -p bazaar-tracker-gpui` into `target/release/` |
| `just tracker start` | Starts `target/release/bazaar-tracker-gpui.exe` without rebuilding, with `BAZAAR_TRACKER_DATA_DIR` set to `<repo>/.bazaar-tracker` |
| `just tracker test` | `cargo test --workspace`; the root is a virtual workspace, so a plain `cargo test` there tests every member too |

The app reads no command-line options: there is no `args.rs`, and `main.rs` never looks at `std::env::args`. `just tracker run *args` passes extra words on to the app, but the app ignores them. Its only outside input is the `BAZAAR_TRACKER_DATA_DIR` environment variable (see [data-folder.md](data-folder.md)).

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
| Advisor updates (`tracking::advisor::client::Advisor`) | A gpui task, fed by short-lived `advisor-client` threads | `on_advisor_update`: the Advisor tab ([below](#advisor-tab)) |

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

`TrackerApp::on_notice` applies a notice to each model in that order, then redraws. After `Session::apply` it tells the Advisor tab about a `RunRebound` (its Conversations move with the Run) and lets it follow the selected Run. Every model in `apps/tracker/src/tracking/` is plain data with no gpui, built from notices and `history.db`, so it has unit tests. `ui/` turns them into elements.

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

Clicking a row selects the Run and loads its Events. A chat icon (Lucide `MessageCircle`) shows on a row while the pointer is over it, and always on the selected row: it opens the Run in the Advisor tab ([below](#advisor-tab)). A trash button shows while the pointer is over a row (not on the Live run). A click opens a strip under the row, inside its highlighted card: "Delete this Run? Its Events and Advisor Conversations are deleted too." on its own line, then a neutral **Cancel** and a red **Delete** (trash icon) against the right edge (`widgets::delete_strip`, `text_button`, `delete_button`). The row keeps its hero, tags and date, and its chat and trash icons hide while the strip is open. Delete deletes the Run, its Events and its Advisor Conversations from `history.db`; Cancel, Escape or a click on another row closes the strip, and only one row asks at a time (`tracking::delete_ask::DeleteAsk`). With no runs the list says "No runs yet — start a run in The Bazaar and it will appear here."

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

Beside the pop-out button, at the top right of the Live section, a chat icon opens the Live run in the Advisor tab. With no Live run it is greyed out and its tooltip says "No run is being played".

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

Rows are 24 px; the table is 220 px tall and draws only the rows in view (`uniform_list`). A selection or a sale is indented under the Event before it, as `↳` and a badge (Sold in `gold`, Selected in `accent`). A line marks where a new hour begins. Clicking a row chooses that Event; clicking it again lets go. While the Live run is selected and no Event is chosen, the list follows the newest Event. A chat icon (Lucide `MessageCircle`, 18 px) sits at the right of a row while the pointer is over it, and always on the selected row: it opens the Advisor tab on a new Conversation with that Event attached ([below](#advisor-tab)), without choosing the row.

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

**Slot marks.** `SocketEffect` cards are not items: each marks one slot in a 13 px row, under the player's grid and over the opponent's. Marks on the same slot split its cell side by side, and its tooltip names them all. A mini Board (`render_mini_grid`: the Advisor's Context row and the recommendation card's source runs) draws them too, in a 9 px row under it.

| Card | Label | Colour |
|---|---|---|
| Stove | Heat | `burn` |
| Cooler | Chill | `diamond` |
| `<letter> Note` | the letter | `legendary` |
| other | its name | `overlay` |

**Skills.** Square icons, eight to a row in two groups of four, across half the width beside a SKILLS label. Rarest first; a skill with no known tier goes last. Each has a tier border, its icon (`ObjectFit::Cover`) or else its name, and a tooltip. No skills reads `none`.

**Header stats.** On the right: `♥ 1240 / 1400` in `loss`, `● 36 (+5) gold` in `gold`, `Lv 7` in `subtle`.

### Detail island

On the **Run** tab ([tabs](#advisor-tab)), `ui/run/detail.rs` shows two parts that scroll together.

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

### Advisor tab

The detail island has two tabs along its top, **Run** (everything above) and **Advisor**. The open tab stays open when another Run is selected, and isn't saved: the window opens on Run. The Advisor tab talks to the Build Advisor's backend (`just advisor serve`, [advisor.md](advisor.md#the-backend)); the app never starts it.

**Chat icons.** One on a sidebar Run's row (on hover, and always on the selected row) and one beside the Live section's pop-out button (the Live run's; greyed out with none). Each selects its Run, opens the detail island on the Advisor tab, shows the Run's newest Conversation (or none, so the first message makes one), and puts the typing in the box. Which Run an icon opens is `tracking::advisor::state::icon_run`.

**Attaching an Event.** The chat icon on a row of the Run tab's Event table opens the Advisor tab on a new Conversation of the selected Run (none shown, even once the list comes; `AdvisorState::open_event`), with that Event attached to the first message. The box shows the attachment as a chip above the text: a paperclip, `Event #7 · D5 H2 · Options: 3 options · Choice` (the Event's number in the Run as the detail numbers it, its Day and Hour as the table places it, its kind and the table's detail; `tracking::advisor::attachment::chip_text`) and a ✕ that removes it, so the message goes with the situation now (Live or last Snapshot, as without one). The message sent with the chip carries the situation at that Event (`situation::for_event`): the Snapshot the Run tab's SNAPSHOT section shows for it, at the Event's Day and Hour (so the new Conversation's `D5 H2` is the Event's), with the Options it offered as the Offer (each with its kind: `skill`; an item is `loot` in a `LootState`, else `merchant_item`; anything else `encounter_choice`) and the situation's `event` field ([advisor.md](advisor.md#the-situation)) saying which Event it is and what was picked, taken or sold. The attachment goes with that one message: the chip goes when it is sent, and later messages use the situation now unless an Event is attached again. A message that never reaches the backend gives its text and its Event back. Its Context row shows the Event.

**Top to bottom** (`ui/advisor.rs`):

| Part | Shows |
|---|---|
| CONVERSATIONS | The Run's Conversations newest first: the title (the first question, cut by the backend to 48 characters) and a `D5 H2` badge (the Day and Hour it began; `D5` without the hour), the whole as a tooltip. `+` starts a new one ("New conversation" shows until the first message). ✕ shows on hover; a click opens the same delete strip as a Run's under the row ("Delete this Conversation? Its messages are deleted too.", Cancel and Delete, the ✕ hidden meanwhile). Delete deletes the Conversation and its messages and shows the newest left; Cancel, Escape or a click on another Conversation closes it. "None yet: the first question starts one." when empty. At most five rows tall (`state::LIST_ROWS`), then it scrolls with the app's thin scroll bar; the shown Conversation is scrolled into view when the list opens or it changes. **Folding:** the header reads `▾ CONVERSATIONS · 7`; a click on it (not on `+`) folds the list away to `▸ CONVERSATIONS · 7 · Hello D14 H6`, the count then the shown Conversation's title and Day and Hour ("New conversation" while one is being started; `AdvisorState::list_summary`), so the messages get the height. Folded or open lasts while the app runs, across Runs and tab switches (`AdvisorState::list_collapsed`), and isn't saved. `+` and the chat icons work while it is folded |
| Status | In `error`: "The Advisor isn't running. Start it with" and then `just advisor serve` as inline code (a monospace chip, `widgets::inline_code`) with a copy button beside it (`widgets::copy_button`), when nothing answers on `127.0.0.1:8420`. The button writes the command (`SERVE_COMMAND`) to the clipboard and shows a tick in the success colour, tooltip "Copied", for 1.5 s (`TrackerApp::copy_text`); the backend's reason when it answers 503 (such as `history.db` at another version); another error's detail (such as 409 "A reply is still running.") |
| Messages | The chosen Conversation, loaded from the API: the player's questions right-aligned on `overlay` in a bubble as wide as the text, up to 85% of the panel, each with its Context row under it (below), and each reply in full (below). A spinner and `Working…` while a reply is on its way and nothing of it shows it at work yet; the list follows a streaming reply down while it is scrolled to its end (within 24 px): scrolled up to read, it stays put, and a round ↓ button at the bottom scrolls back down, after which it follows again. Loading a Conversation and sending a message scroll to the end. A Conversation with a reply running elsewhere takes no message until it ends |
| Box | Over it, a **What should I do?** button (✨) that sends that question with the situation now, as a follow-up button does (muted while a reply runs or a Conversation is being made). In it, the attached Event's chip, when there is one (above), then the question over as many lines as it has (up to 120 px, then it scrolls), a caret while it takes the typing, and a send button that becomes a red Stop while this app's message is on its way. Click it to type; Enter sends, Shift+Enter starts a new line, Backspace deletes, Ctrl+V (Cmd+V) pastes, Escape or a click outside lets go. Up to 4,000 characters |
| Model | Under the box, the model the next message goes to (Lucide `Cpu`, the id, a chevron): the model chosen, else the backend's default once `GET /api/models` has said it, else "Advisor's default model". It needs no backend, so with the backend down it still shows the saved default. A click opens the model picker (below) |

The tab says "Select a run to ask the Advisor about it." with no Run selected.

**The Context row** (`tracking::advisor::context`, drawn by `render_context`) shows what the app sent with a question, so the player can check what the Advisor saw. Under the question, right-aligned, a collapsed row with a paperclip icon: `Context · D14 H6 · 10/10 slots` (the attached Event's number as `Event #7` first when one was, the Day and Hour, and the Board's used slots of 10, each when sent; `Context · none sent` for a question sent without a situation). It opens and closes like a tool line: a click on the header, or a double-click in its body (`render_collapsible`). Opened, in a bordered box:

| Part | Drawn as |
|---|---|
| Numbers | Event (`#7 Options: 3 options · Choice (+12:05)`) then Picked or Sold, when an Event was attached; Hero, Time, Gold, Level, Health (`900/1200`) and State, those that were sent |
| BOARD, STASH | Each a mini Board 280 px wide (`render_mini_grid`, as the recommendation card's source runs): the cards laid out by `tracking::run::board::board_grid` (each on its sent slot) or `stash_grid` (in order), with size, tier border, art, and the enchantment and name in the tooltip; "empty" for an empty list. A card keeps the name it was sent with |
| Skills | `render_skills`, as on the Run tab |
| OFFER | One line per card: a small art square in its tier's border, the name (enchantment first) and its kind (`merchant item`, `loot`, `encounter choice`, `skill`); "nothing" when empty |
| OPPONENT | In a combat: its hero and health, its Board as a mini Board and its skills |
| Slot effects | The marks (`slot_effects`, the player's and the opponent's) under the mini Board, as [slot marks](#board-tiles) are drawn; a Board with marks and no items is still drawn |
| JSON | A toggle (Lucide `Braces`) that shows the situation exactly as sent, pretty-printed in Consolas, with a copy button (`widgets::copy_button`, `TrackerApp::copy_text`) in its corner |

A part that wasn't sent is left out; a question sent with no situation shows "No context sent". The situation comes from the saved message's `situation` ([advisor.md](advisor.md#the-api)) when a Conversation loads; while a reply streams, the question gets the situation the tab sent with it (`AdvisorState::sent_situation`, given to it by `Transcript::set_situation`). Whether a Context row and its JSON are open is kept with the other open blocks.

**A reply** (`tracking::advisor::reply`, drawn by `render_reply`). A reply is every assistant message (one per model turn, a *step*) and tool result after a question, up to the next question; a step's parts are its content blocks by index, as in tau-app's chat (`useTauChat.ts`, `Message.tsx`). It is built the same way from a streaming reply's events and from the saved messages (`Transcript::from_saved`, which replays them as `message_end`s), so a reloaded Conversation looks as it did. Top to bottom:

| Part | Drawn as |
|---|---|
| How it ended | A `Stopped` badge (muted) or a `Failed` badge (`error`) above a reply that didn't finish |
| Thinking | `Thinking… 3s` with a spinner while it streams (counted from its first delta, ended by any other delta or the message's end; the tab is drawn again every 250 ms while a reply runs), then a collapsed `Thought for 3s` (rounded, at least 1 s) that opens to the text. A saved thinking's time isn't kept by the backend, so a reloaded one reads `Thought` |
| Tool lines | One per tool call: a spinner, then ✓ (`success`) or ✕ (`error`, and the name in `error`), and the tool's name (`Tool call` while its arguments stream, as the name only comes with the message's end). Collapsed; a click opens it to **Arguments** (pretty JSON, or the JSON so far) and **Result** or **Error** (the result's `details`, else the text the model read, as JSON when it is JSON), in Consolas. A call left without a result by Stop shows ✕ "Tool call interrupted by user", as the backend saves it. `suggest`, `follow_ups` and `ask_user` are not lines unless they failed |
| Text | Light markdown (`tracking::advisor::markdown`): `**bold**`/`__bold__`, `*italics*`/`_italics_` (`_` only at a word's edge, so `rate_options` stays as it is), `` `inline code` `` (Consolas on `overlay`, in `accent`), and `- `/`* `/`+ ` bullets and `1.`/`1)` numbered lines indented by two-space steps. A marker without its partner (as while text streams) stays as typed; `\*` is a literal star. Anything else (headings, tables, quotes, links, fenced code, whose lines are not parsed) stays plain text. Each line is one `StyledText` with highlight ranges |
| Recommendation card | `suggest`'s card, after the text (`ui/recommendation.rs`, below); `ranking…` and the rows with their reasons only, from the arguments, until the result comes |
| The Advisor's question | `ask_user` (`tracking::advisor::ask_user`, as tau-app's `AskUserCard.tsx`): the question, a Single or Multiple choice badge, one row per choice (radio, or checkbox with `multiple`), a one-line text box when `allowFreeText` (click it to type; Enter sends, Escape lets go), and Send. With one choice a pick and typed words replace each other; with several, picks toggle. The answer is an ordinary message: the label (or the words), or for several a `- <label>` line per pick in the card's order then the words. The question is answered by the player's next message, from the card or the box: the card is then locked, its picks read back from that message (`You answered: …` for other words), which also rebuilds it when history loads |
| Follow-ups | The last `follow_ups` that ran, as 2–3 buttons under the newest finished reply only; each sends its question (with the situation) as if typed, leaving the box's draft alone |
| Error | A failed reply's error in `error`: the model's `errorMessage`, the reply's `error` event, or the stream breaking off |
| Usage line | `model · 4.2 s · 12,244 tokens · $0.0285 · 3 tool calls` (`free` at no cost; no tools part without tool calls) in 11 px Consolas, from the `stats` event or the saved stats. Hover for input, output, cache read and written, the cost (an estimate, provider-reported) and the time including tool calls. Formats are tau-app's (`tracking::advisor::usage`): dollars to three significant digits, `850 ms`/`4.2 s`/`1 min 5 s`, counts with commas |

**The recommendation card** (`tracking::advisor::recommendation`, drawn by `ui/recommendation.rs`), from `suggest`'s result (`details`, the shape in [advisor.md](advisor.md#the-tools)), as in the record's mock-up:

```
┌ ★ SUGGEST · Shop · Day 5 ──────────────────────────────────────────┐
│ ① [art] Laurel's Bouncy Ball  Small · Bronze  ✓ fits                │
│         "Shows up with both your items"                             │
│         with your Board     ████████░░  3/4  (with Pizza Cutter · …)  │
│         taken when offered  █████░░░░░  7/15                        │
│ ② [art] Decanter  Medium · Silver  ✕ doesn't fit …                  │
│ ④ [art] Mystery Box …   no data · from card text                    │
│ ⚠ source: 6 Jules runs · S20 · 1 profile · thin ⚠                   │
└─────────────────────────────────────────────────────────────────────┘
```

| Part | Drawn as |
|---|---|
| Title | ★ `SUGGEST · <title>` in `accent`, the card bordered in `accent` |
| Rows | Best first, sorted by `rank`: the rank in a circle (the first on `accent`), the card's art (`Names::art` of its id) in a box as tall as 36 px and as wide as its size (a slot is half as wide, as on the board) with its tier border, the name, `Small · Bronze` in the tier's colour (sizes and tiers in any case read as the game names them), ✓ `fits` (`success`) or ✕ `doesn't fit` (`error`) from `fits` (nothing when it is `null`: a skill, an encounter, or the Board or size not known), and the reason in italics |
| Bars | Under the reason, each a label, a track (up to 120 px, filled in `accent` by `n`/`of`), the count `n/of` and a note. **with your Board**: the `with_board` pair with the highest lift (then the most runs), noted `with Pizza Cutter · lift 2.81`; with an empty Board, **on final Boards** from `runs_with` instead. **taken when offered**: `picks.taken`/`picks.offered` |
| No data | A row with `no_data` shows "no data · from card text" in place of its bars |
| Hover a count | A popup under the count (anchored, kept in the window, 272 px wide, scrolling past 340 px) headed `taken when offered · 7/15 · 4 source runs`, listing the runs behind the count that the result carries, each once: its result (`S20 · 10–3 · Gold Victory`), a `taken` badge on the runs where the card was taken, and a mini Board 240 px wide (`render_mini_grid` in `ui/run/board.rs`: the Board laid out by `tracking::run::board::board_grid`, each item on its slot with its tier border and art, its name when it has none and in the tooltip). It stays while the pointer is on the count or on the popup (`recommendation::Hover`), so the pointer can move into it |
| Click a run | Expands it under its row, in the card (a second click, or its ✕, closes it): its hero and result, a **Bazaar DB** link that opens its page in the browser (only `https://bazaardb.gg/` links; read-only, nothing is sent), its full Board (`render_grid`, as the Snapshot draws it, with no odds buttons) and its skills (`render_skills`). Cards the catalog doesn't know keep the names the run gives them |
| Source line | Under a rule: `source: 15 Jules runs · S20 · 1 profile` (the result's `source`: runs, hero, seasons and profiles) in `muted`; under 10 runs (`thin`) it ends `· thin ⚠` and is drawn in `gold` with a warning icon. A result without `source` gets one made from `n`, `hero`, `sample` and `profiles` |

Which thinking blocks, tool lines and source runs (by the card's row) are open is kept in `AdvisorState` and forgotten when another Conversation is shown; which count's popup shows is `AdvisorState::hover`.

An open thinking block or tool line also closes on a double-click anywhere in its body (the text, or the arguments and result), as its header would (`AdvisorState::collapse`, which leaves a closed one closed); a single click does nothing. The header's tooltip says so (`Collapse` / `Or double-click the text`; `Expand` while closed). If the header is then above the messages' visible area, they scroll so it shows just under the top (the block's `on_children_prepainted` keeps where the header was last painted, and the scroll offset moves by the gap). The recommendation card, the `ask_user` card and text don't collapse this way.

**Stop.** While this app's message is on its way to the shown Conversation, Send is Stop. Stop marks the reply `Stopped` at once, keeping what arrived (thinking ended, running tools interrupted, a reply shown even if nothing of it had come), and passes over whatever still arrives until the stream ends; nothing more is sent until then. It closes the stream (`Request::Stop`): the client shuts the reply's socket from the window's thread, the backend sees it close and cancels and saves the partial reply as `aborted`. Stopped while the Conversation is still being made, the question isn't sent to it and goes back in the box. A stream that breaks off fails the reply with the error; one that ends without `done` is done.

**Sending.** Enter trims the question and sends it with the **situation** (`tracking::advisor::situation`, the schema in [advisor.md](advisor.md#the-situation)). With no Conversation shown it first makes one (`POST /api/runs/{run_id}/conversations` with the question and the situation, which give its title, Day and Hour), shows it at the top of the list, then sends the question to it (`POST /api/conversations/{id}/messages`). Nothing more is sent while a reply streams. A question that never reached the backend (nothing listening, or an error status) goes back in the box.

| Run | The situation is built from |
|---|---|
| The Live run, with a board read | The latest board read (`LiveState::board`, the one the Live section's `LiveView` is drawn from): hero, day, hour, gold, level, health; the Board with each item's first slot and size (placed as the board draws it, `SocketEffect` cards left out), Stash and skills; the game's state in the Live section's words (`Shop`, `Encounter choice`, `Combat`, `Loot`); the Offer, each card's `kind` a `skill`, a `merchant_item` in a shop, `loot` elsewhere for an item, else an `encounter_choice` (an id the read has no card for is left out); and in a combat the opponent's hero, health, Board and skills |
| Any other Run, or the Live run before a read | The Run's last Snapshot (`moment::at`): the player's numbers, Board, Stash and skills, with the Run's hero and day when the Snapshot lacks them, and `game_state` `Run over` once the Run has ended. No Offer or opponent |
| A Run with no Snapshot | The Run's hero and day only |
| Any Run, with an Event attached | The Snapshot at that Event (`situation::for_event`, `moment::at` with it chosen) at the Event's Day and Hour, the Options' cards as the Offer, the Options' context as `game_state` (`Choice`, `Loot`, …; `Run over` only for Run Ended), and the `event` field ([above](#advisor-tab), "Attaching an Event") |

Every field is the schema's: a day under 1, an hour under 0, or a size other than Small, Medium or Large is left out. Names come from the card catalog (else the card id), ids are the `template_id`.

**Trying again.** The backend's state is remembered (`Backend`: not asked, ready, unreachable, unavailable). Opening the tab, a chat icon, or sending asks again while it wasn't usable, so starting `just advisor serve` later needs no restart of the app.

**Following the Run.** The tab follows the selected Run: selecting another Run (by a click, or when a new Live run starts) lists that Run's Conversations and shows its newest; the draft in the box is kept. When the Live run gets its real id (`RunRebound`) the tab keeps its Conversations, which the schema moved with it. Deleting a Run, or clearing the data, empties the tab.

**The client** (`tracking::advisor::client`). `Advisor::ask` runs each request on a short-lived `advisor-client` thread with a blocking `ureq` call (2 s to connect, so a backend that isn't running is found at once; 15 s for a call; never through a proxy). A reply instead goes over a plain `TcpStream` of its own (2 s to connect, no limit after; a hand-written HTTP/1.1 POST with `Connection: close`, the answer's head read, then the body de-chunked, cut to its `Content-Length` or read to the end), because `ureq` gives no way to close a blocked read from another thread: `Advisor` keeps each reply's `StopHandle` by Conversation, and `Request::Stop` shuts the socket at once, ending the thread without an error. A reply's body is read line by line; `sse::SseParser` turns the lines into frames (lines in pieces, CRLF, several `data:` lines joined, comments passed over, the name from `event:` else the JSON's `type`), and each frame becomes an `AdvisorEvent` (`message_start`, `delta` text/thinking/toolcall, `message_end`, `stats`, `error`, `done`, else unknown and passed over). Each event, and each call's result, goes to the window as an `Update` over an `async_channel`, as the notices do:

```
TrackerApp ─Request─► advisor-client thread ─ureq (a reply: TcpStream, shut by Stop)─► 127.0.0.1:8420
     ▲                       │ SSE lines → SseParser → AdvisorEvent
     └──── async_channel ◄───┘ Update (Conversations, Created, Messages, Event, Ended, Deleted)
           on_advisor_update → AdvisorState → more Requests
```

**The model picker** (`overlays/model_picker.rs`, drawn in `ui/advisor.rs`). A 440 px panel near the top of the window, like the theme picker. Opening it asks `GET /api/models` again (`Request::Models`; the backend caches the list) and the rows fill in when it answers. The rows (`model_picker::rows`): "Advisor's default model (its id)" while nothing is typed (no model is sent then), each listed model whose id or name holds every typed word in any case (the id, its name under it), and, when what is typed is one word that isn't a listed id, `Use "<typed>"`, so a model the list lacks can be typed in. Twelve rows show at a time, following the highlighted one, with `12 of 340` in the footer. The chosen model is in the accent colour and the saved default is tagged `default`. Up and Down move, Enter (or a click) uses the row for the next messages, Shift+Enter (or Shift+click) also saves it as the default, Escape closes. **The rule:** the picker starts on the saved default (`settings.toml` `advisor_model`); a choice changes only the messages sent after it, until the app restarts, unless it is made the default. Every message carries the model chosen when it was sent (`Request::Create` and `Request::Send` hold it, so a choice made while a new Conversation is being made doesn't change that message); with no model chosen none is sent and the backend answers with `ADVISOR_MODEL`.

`tracking::advisor::state::AdvisorState` is plain data: each click or `Update` goes through one of its methods, which returns the requests to run next, so the tab's behaviour is unit-tested without a window or a backend. An answer for a Run or Conversation no longer shown is passed over.

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
| `ImportItemOdds` | Import item odds… | — | Asks for the odds scraper's `odds/` folder (or one of its files) and saves the item odds in it to `history.db`, keeping any saved since; see [odds-scraper.md](odds-scraper.md#import-into-the-app) |
| `ClearData` | Clear data… | — | Opens Settings at its Data section, with Clear data waiting for one click to confirm |
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

Every other key goes to `TrackerApp::key_down`, which hands gpui's key name (`up`, `enter`, `a`), the typed text and whether Shift is held to the open overlay's `press` (only the model picker uses Shift: Shift+Enter also sets the default). A key with Ctrl, Alt or the platform key held is not passed on. With nothing open a key goes to the odds search, the Advisor's box or the text box of the Advisor's question (`ask_user`) if one was clicked into, else nowhere. The Advisor's boxes are handed Shift too (Shift+Enter is a new line in the box) and Ctrl+V (Cmd+V) pastes into them ([Advisor tab](#advisor-tab)).

At most one overlay is open (`overlays::Overlay`): the menu, the palette, the theme picker, the Settings overlay, or the Shop odds, item odds or Tracked items pop-up. Opening one closes the others. While one is open a transparent backdrop covers the window, and a click on it closes the overlay. Each overlay is a plain struct that takes a key and returns an outcome, so its keys are tested without a window.

| Overlay | Keys |
|---|---|
| Menu, Settings, Shop odds, item odds | Escape closes |
| Palette | Type, Backspace, Up/Down (wrap), Enter runs, Escape closes |
| Theme picker | Type, Backspace, Up/Down preview (wrap), Enter chooses, Escape restores |
| Tracked items | Type, Backspace, Up/Down (wrap), Enter tracks or untracks the row, Escape closes |

## Themes

Ten themes, one file each in `apps/tracker/src/theme/`. The theme picker (360 px, 56 px from the top) lists them in `THEMES` order under DARK and LIGHT headings, with a "Select a theme…" field.

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
| DEFAULT ADVISOR MODEL | The saved default model, or "The Advisor's own (…)" when none. Change… opens the model picker, where Enter or a click saves the default (and uses it) and Settings opens again; the "Advisor's default model" row clears it. Reset (shown while one is saved) clears it. Saves `advisor_model` |
| CARD ART | `3310 cards have art · extracted 2026-10-31`, or "Not extracted yet.", or the progress (`Reading the game's card data…`, `Extracting… 3 of 40 bundles`, `Finishing…`). The button reads Extract art, Update art, or a greyed Extracting… while one runs. See [card-catalog-and-art.md](card-catalog-and-art.md) |
| Item odds | Picks heroes (and Neutral) and loads their items' odds; see [bazaar-db.md](bazaar-db.md) |

## `settings.toml`

`apps/tracker/src/shell/settings.rs`. Every key has a default (`#[serde(default)]`), so a partial file keeps what it gives and an empty file means all defaults. Unknown keys are ignored.

| Key | Default | Meaning |
|---|---|---|
| `theme` | `"Rosé Pine"` | Theme name; unknown gives Rosé Pine |
| `log_path` | `""` | `Player.log`, if not in the usual place under `LocalLow` |
| `game_dir` | `""` | The game's install folder, for card art; empty finds it ([card-catalog-and-art.md](card-catalog-and-art.md)) |
| `run_sync` | `false` | Upload Runs to Bazaar DB; nothing is sent without a secret key either way ([upload.md](upload.md)) |
| `show_run_duration` | `false` | Saved; not shown anywhere yet |
| `card_size` | `"medium"` | `small`, `medium` or `large` |
| `advisor_model` | `""` | The Advisor tab's default model, which its picker starts on; empty is the backend's own `ADVISOR_MODEL` (no model sent). Trimmed on load |
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

`apps/tracker/src/shell/logging.rs` writes `<data>/logs/tracker.log` through `tracing`, info level and above, with no colour codes. The first line of each launch is `Bazaar Tracker 0.1.0 started (process <pid>)`. A panic is logged as `panic: …` before the previous panic hook runs.

| Rule | Value |
|---|---|
| Rotation | When a write would take a non-empty file past 1 MiB (`MAX_BYTES`, 1,048,576 bytes) |
| Backups | 3 (`BACKUPS`): `tracker.log.1` is the newest, `.3` the oldest; the oldest is dropped |
| Across launches | The file is opened to append, so it keeps growing until it rotates |
| Rotation fails | Lines keep going to the same file rather than being lost |
| File cannot be opened | `bazaar-tracker: cannot write the log: …` on stderr; the log panel still gets every record |

The file keeps `tracing`'s own UTC timestamps.

## Times

Every time the window shows comes from `apps/tracker/src/tracking/when.rs`, in one style ([ADR 0001](../adr/0001-one-date-and-time-style.md)): a 24-hour clock, the day before the month's short English name, local time, no system locale.

| Function | Example | Used for |
|---|---|---|
| `time_s` | `20:25:07` | Log rows, Last event, an Event's detail title |
| `date_time` | `1 Nov 20:25`; `30 Oct 2025 20:25` in another year | Runs list, Run header |
| `stamp` | `1 Nov 2026 20:25` | When shop odds were saved |
| `offset` | `+1:05`; `+1:02:03` past an hour | An Event's time into its Run; before the start counts as `+0:00` |

## gpui and its dependencies

From `apps/tracker/Cargo.toml`:

| Crate | Version | Why |
|---|---|---|
| `gpui` = `gpui-pre` | `=0.3.9`, features `font-kit`, `windows-manifest` | The copy of gpui that gpui-kit is built on, a different crate from `gpui` on crates.io; `package = "gpui-pre"` keeps `use gpui::…` working |
| `gpui-pre-platform` | `=0.3.9`, features `font-kit`, `runtime_shaders` | Opens the window (`gpui_platform::application()`); `runtime_shaders` compiles the Metal shaders at start, so macOS builds need only the Command Line Tools |
| `gpui-base` | `=0.7.3` | gpui-kit's unstyled `Button`, and the scroll bar |
| `gpui-kit-assets` | `0.7.3` | Lucide icons (`PanelLeft`, `PanelRight`, `SquareTerminal`, `Globe`, `Trash`, `MessageCircle`, `Plus`, `X`, `SendHorizontal`) and the asset source `AllAssets` |
| `gpui-component` | `0.7.3`, no default features | `Kbd` for shortcut hints, the `LineChart` of the Economy, and the theme its scroll bar and tooltips read |
| `chrono` | `0.4`, feature `clock` | Local time in `when.rs` |
| `tracing`, `tracing-subscriber` | `0.1`, `0.3` | Logging |
| `async-channel` | `2` | Notices, log rows and reports into the window |
| `ureq` | `3` | The Advisor tab's HTTP client for `just advisor serve`, its replies read as SSE |
| `serde`, `toml`, `serde_json` | | `settings.toml`, the art `manifest.json` |
| `bazaar-tracker`, `bazaardb` | path | The tracker; Bazaar DB, with the `webview` feature on Windows |

`[profile.dev.package."*"] opt-level = 2`, in the root `Cargo.toml` (a profile belongs to the workspace), builds every dependency optimised in debug, so `just tracker run` does not lag while scrolling; the app's own code stays at debug settings, quick to build. `tempfile` is a dev-dependency for tests.

## Tests

Tests sit beside the code as `<name>_tests.rs`. Everything with state has them; the drawing files do not.

| Area | Tests |
|---|---|
| Startup plumbing | `shell/data_dir_tests.rs`, `instance_tests.rs` (a second launch signals the first; different names do not clash; the lock frees), `logging_tests.rs` (append, rotate, keep 3 backups, banner), `settings_tests.rs` (defaults, partial and unknown keys, unparsable file left alone, round trip, clamping, the default Advisor model saved and reloaded), `art_tests.rs` |
| Overlays | `overlays/*_tests.rs`: the eleven commands in menu order, menu equals palette, loose search, wrap-around, theme preview, choose and restore; the model picker's filtering (every word in the id or name, the backend's row, a typed id not in the list), Enter and Shift+Enter (and from Settings), and the rows shown following the highlight |
| Themes | `theme/theme_tests.rs`: ten themes, Rosé Pine first, dark before light, every role a colour, search by name and appearance |
| Advisor tab | `tracking/advisor/*_tests.rs`: the SSE parser (lines in pieces, CRLF, several `data:` lines, comments, unknown events), the situation from a board read, from a Snapshot and at an attached Event (Options as the Offer with what was picked, loot, Selected, Sold), the Board's Stoves, Coolers and Notes sent as `slot_effects` and not as items, the attached Event's chip text, Conversation titles (`D5 H2`) and newest first, the Context row from a saved situation (the summary line, the Board, Stash, skills, Offer and opponent laid out with their sent names, the JSON as sent, missing fields left out, no situation, slot effects marked on the Board) and a question keeping its situation when saved and when streamed, the reply model from event sequences and from saved messages (a normal reply's order, cards, buttons and usage; thinking timing; tool states and results; stopped, failed, broken-off and tool-error replies; `ask_user` waiting and rebuilt from the next message), the markdown splitter, the `ask_user` answer format and reading it back, the usage line's formats, which Run and Conversation each chat icon opens (an Event's: a new one, with the Event attached to one message and given back if it never arrived), sending to a new or the shown Conversation, Stop (also while a Conversation is made), follow-up buttons, answering and locking a card, the not-running message and trying again, deleting, a rebound or deleted Run, every message carrying the model chosen when it was sent (also across a new Conversation being made), the picker showing the chosen model without the backend and keeping it and the list across Runs, the Conversation list folding and staying folded across Runs, its folded header (the count and the shown Conversation) and the row it scrolls to; and the client against a local socket (listing, an error's detail, the models list, a message's body with and without a model, a streamed and a chunked reply, a refused message, Stop closing a waiting reply and Stop before it opens, nothing listening), the response head and the chunked reader |
| View models | `tracking/session_tests.rs`, `delete_ask_tests.rs` (one row asking at a time; Cancel, Escape and another row closing the strip; Delete only on the row that asks), `status_tests.rs`, `service_tests.rs`, `names_tests.rs`, `when_tests.rs`, `live_tests.rs`, `run/*_tests.rs` (header, moment, board, economy, events, detail), `log/*_tests.rs` (feed, narrate, rows: the 2,000-row buffer, details) |
| Drawing helpers | The gold ticks in `ui/run/economy.rs`; `ui/web_panel_tests.rs` |

Run them all with `just tracker test`; `cargo test -p bazaar-tracker-gpui` runs only the app's.
