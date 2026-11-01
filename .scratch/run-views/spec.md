# Run views: the app tracks and shows real data

Status: ready-for-agent

## Problem Statement

The app opens a window with three empty islands. The tracker that follows the game, runs the run state machine and saves Runs and Events to `history.db` exists (`bazaar-tracker`'s tracker service, run headlessly by `just track`), but the app does not depend on it and nothing in `src/` starts it. A player who runs the app while playing sees "Not tracking", no runs, and no board, whatever the game does. The previous spec (workspace-and-app-frame) left this out on purpose: "Connecting the app to the tracker service: the connection indicator, the runs list and the run views reflecting real data come in the run-views spec." This is that spec.

## Solution

The app starts the tracker service on a background thread when it opens, and the three islands show what it finds. The status bar says whether the game is attached. The sidebar lists every Run in `history.db`, newest first, and a new Run (or a changed one) appears there as its Events are saved. Selecting a Run fills the main island with a header (when, hero, rank, outcome, record, a per-day timeline), the **board, stash and skills at the selected moment**, and the Run's Events. Selecting an Event moves that moment and fills the detail island with what is specific to the Event: the options on offer and which was picked, or the opponent's board.

The moment is the live board for a live Run (the Snapshot of its latest Event), the last Snapshot for a finished Run, or the nearest Snapshot at or before the selected Event. Items are drawn on a 10-slot grid (Small 1, Medium 2, Large 3) with a tier border, badges on top, the sell price bottom-left and the cooldown bottom-right, with everything else in a tooltip. There is no card art.

## User Stories

1. As a player, I want the app to start tracking by itself when it opens, so that I never have to start a second program.
2. As a player, I want the status bar to say "Tracking" when the game is attached and "Waiting for the game" when it is not, so that I know whether my run is being recorded.
3. As a player, I want the status bar to say "Not tracking" when the tracker could not start, together with the reason, so that I know why nothing is recorded.
4. As a player, I want a failed save to appear in the status bar and stay until I dismiss it, so that I never lose an Event without knowing.
5. As a player, I want the same error not to reappear every half second after I dismiss it, so that a stuck problem does not make the app unusable.
6. As a player, I want the tracker to stop cleanly when I quit, so that no save is cut in half.
7. As a player, I want the sidebar to list my past Runs, newest first, as soon as the app opens, so that my history is there without doing anything.
8. As a player, I want each sidebar row to show when the Run started, its hero, its record (for example 7W-2L) and the day it reached, so that I can tell Runs apart.
9. As a player, I want a record to be drawn in the win or loss colour once the Run has an outcome, so that I see how each Run went.
10. As a player, I want the Run I am playing now to carry a LIVE chip, so that I can find it.
11. As a player, I want a new Run to appear at the top of the sidebar when I start it, and its row to update as the Run goes on, so that the list never needs refreshing.
12. As a player, I want a Run that began with a `local-` id and was then given its real id to stay one row, so that a Run never shows up twice.
13. As a player, I want to click a Run to select it, so that I can look at it.
14. As a player, I want a Run I start to be selected for me, so that I see it without a click.
15. As a player, I want the main island to show the selected Run's header: start time, hero, rank and rating, the rating change when it ended, LIVE chip, outcome, day and record, so that I see how the Run stands.
16. As a player, I want a timeline of days in the header, each marked won or lost by that day's combat, so that I see the shape of the Run at a glance.
17. As a player, I want my board on a grid of ten slots, with a Medium item two slots wide and a Large three, so that it looks like the board in the game.
18. As a player, I want each item bordered in its tier's colour, so that I can tell tiers apart without reading.
19. As a player, I want badges on each item for damage, shield, heal, burn, poison, regen and ammo, and `x2` for multicast, so that I see what it does.
20. As a player, I want the sell price in the bottom-left corner and the cooldown in the bottom-right, so that I can read them without opening anything.
21. As a player, I want everything else about an item (tier, size, enchantment, tags, every non-zero attribute) in a tooltip, so that nothing is hidden.
22. As a player, I want an enchantment shown with the item's name, so that I know which items are enchanted.
23. As a player, I want cards shown by name rather than id, so that the board is readable.
24. As a player, I want my stash shown under the board and my skills in a row under that, so that I see all I have.
25. As a player, I want the stash section to fold with a click, and the folded state remembered, so that I can give the Events more room.
26. As a player, I want the board header to say which moment I am looking at (live, the end of the Run, or a numbered Event) with the health, gold and level of that moment, so that I know what I am seeing.
27. As a player, I want the board of a live Run to follow it as Events arrive, so that I can watch my Run.
28. As a player, I want a finished Run to show its last board, so that I can see what I ended with.
29. As a player, I want the list of the selected Run's Events in the main island, each with its time into the Run, its day and hour, what kind it is and a short description, so that I can review the Run's decisions.
30. As a player, I want a picked option or a sale to sit under the Event it belongs to, so that decisions read as one.
31. As a player, I want a combat's outcome drawn in the win or loss colour, so that I find fights quickly.
32. As a player, I want new Events to be added to the list as they are saved, and the list to stay scrolled to the newest while I watch a live Run, so that I can follow along.
33. As a player, I want to click an Event to move the board, stash and skills to that moment, so that I can see what I had when I chose.
34. As a player, I want to click the selected Event again to go back to the latest moment, so that I need not hunt for a "live" button.
35. As a player, I want an Event that has no Snapshot of its own (a combat ending, say) to show the nearest one before it, so that the board never goes blank in the middle of a Run.
36. As a player, I want the detail island to show the options offered by an Options Event, with the one I picked marked, so that I can see what I passed over.
37. As a player, I want a Selected Event to show the options it was chosen from, so that I get the same view from either Event.
38. As a player, I want a combat's detail to show the opponent's board and mine as they were before the fight, with the opponent's name, hero, rating and level, so that I can see what I was up against.
39. As a player, I want a combat ending to show its result and the opponent's board from its start, so that I see the fight it ended.
40. As a player, I want Events with nothing more to say (Run started, Run ended, rerolled) to show their facts in the detail island, so that it is never a blank page.
41. As a player, I want the detail island to say so when the selected Event has gone (its Run was deleted), so that it does not show stale data.
42. As a developer, I want the view models (connection state, runs list, selection, moment lookup, board layout, badges, Event rows, detail) in plain code with no gpui, so that they are tested without a window.
43. As a developer, I want the `ui/` files to only draw those view models, so that the layer split of the previous spec holds.

## Implementation Decisions

- **Dependency and layer.** The root package depends on `bazaar-tracker`. A new plain layer `src/tracking/` (no gpui, may use `bazaar-tracker`) holds the service bridge and every view model; `ui/` draws them. `shell/`, `overlays/` and `theme/` are unchanged apart from what a ticket says.
- **Starting the service.** `main` opens `history.db` in the data folder, resolves `Player.log` from the `log_path` setting (`log::path::resolve`) and starts `Tracker::start_live` (Windows only; elsewhere the connection says "Not tracking"). If `history.db` cannot be opened the app still opens, with the reason as a sticky error and "Not tracking"; nothing is saved. A changed `log_path` takes effect on the next launch.
- **Bridge.** One extra thread owns the `Tracker`, reads its `std::sync::mpsc` notices and forwards them into an `async_channel` the app already depends on; `TrackerApp` awaits that channel in a `cx.spawn` loop and calls `cx.notify()`. Dropping the service sets a stop flag; the thread ends and drops the `Tracker`, which joins its own threads. `main` drops the service when `run` returns.
- **Connection state.** Three states: `NotTracking` (service not running), `Waiting` (running, game not attached; the initial state of a running service) and `Tracking` (game attached). `Connected`/`Disconnected` notices switch between the last two. Dots: muted, muted and success.
- **Errors.** `SaveFailed`, `LogError` and a failed start become sticky status-bar errors (the existing slot). The text omits the retry delay so that the same error is recognised; an error equal to the last one reported is not shown again until an Event has been saved. `SaveFailed` and `LogError` repeat on the tracker's schedule, so without this a dismissed error would come back every poll.
- **Reading history.** The app and the tracker share one `Arc<History>`. The app reads on the UI thread: `History` keeps a separate read connection in WAL mode, and a Run's Events are small. A list is read at start-up and on selecting a Run; after that, saved Events are applied from the notices (and the Run's row is re-read from `history.db`, so the summary logic stays in one place, the history store).
- **Runs list.** `RunList` holds Runs newest first plus the **live Run id**. The live Run is the Run of the latest saved Event, until a `RunEnded` Event for it is saved or the game is lost; a rebinding renames it. (A Run without `ended` is also what a Run the tracker lost looks like, so the flag alone does not mean live.) Delete and refresh buttons from the mockup are not built.
- **Selection.** `Selection { run_id, event_id }`. Nothing is selected at start-up. When a Run becomes live and is not the selected one, it is selected (this includes a Run starting while another is open: the records are silent, the simplest rule is chosen). Selecting a Run loads its Events; a rebinding of the selected Run keeps it selected under the new id.
- **The moment.** `moment(events, selected)` gives the Snapshot shown: with no Event selected, the Snapshot of the latest Event that has one; with one selected, the nearest at or before it, searching backwards. Events with a Snapshot: Options, Selected, Sold (their `snapshot`) and CombatStart (its `player`). A Run with none shows an empty board. The live board starts with Events and is not polled (open question 2 of the folder-structure record).
- **Stash capacity.** Drawn as ten slots like the board (open question 1 of that record, still unanswered).
- **Board layout.** An item sits at its `Socket_N` slot and covers its size's slots. An item with no socket, or whose slots are taken or off the end, goes in the first gap it fits; one that fits nowhere is counted as "N not shown". `SocketEffect` cards are not items: each marks one slot (Jules's Stove = Heat, Cooler = Chill, and the dragon's A–G Notes), drawn as a label under the player's slot and over the opponent's. The stash has no sockets, so its items are laid out in order.
- **Badges.** From attributes, only when above zero: `DamageAmount` (damage), `ShieldApplyAmount` (shield), `HealAmount` (heal), `BurnApplyAmount` (burn), `PoisonApplyAmount` (poison), `RegenApplyAmount` (regen), `Ammo` (ammo, when `AmmoMax` is above zero), and `Multicast` above one as `x2`. Sell price is `SellPrice`; the cooldown is `CooldownMax` in seconds (milliseconds in the game). Colours are the theme's badge roles. The unnamed attribute `100` is shown under that number in the tooltip (open in record 2026-10-31.002).
- **Names.** Card names come from the card catalog (`CatalogHandle::start`, loaded once off the UI thread from the game folder under the user profile); a card not found, or a catalog not loaded yet, shows its template id, then its instance id. The view redraws when the catalog arrives.
- **Header.** Start time in local time; hero; `Rank Division · rating` from the `RunStarted` Event; the rating change from `RunEnded.rating_after`; a LIVE chip; the outcome; `Day N`; the record. The per-day timeline marks each day from 1 to the Run's day won or lost by that day's last `CombatEnd` (a day without a combat is unmarked). "See on Bazaar DB" and the synced indicator wait for uploads in the UI.
- **Event rows.** Time into the Run as `m:ss` from the first Event; `D<day> H<hour>` (the day shown only when it changes); the kind's name; a description (`Vanessa · Ranked`, `4 options · Choice`, `Katana: Item (Gold)`, `PvP vs Bob (Mak)`, `Victory`). Selected and Sold Events are sub-rows (`↳`) under the Event before them. The mockup's hour separators and encounter banners are not built (records silent).
- **Detail.** For Options: the options as cards with the pick marked; for Selected: its options Event's cards; for CombatStart: opponent and player boards plus the opponent's facts; for CombatEnd: result facts plus the opponent board of its CombatStart; for the rest: facts. Cards show name, `Kind · Tier` and a tier border.
- **Scrolling.** The Events list scrolls; while no Event is selected and the Run is live, it stays at the bottom as Events arrive.
- **Settings.** The `stash_open` key already exists and now has an effect; no new keys.

## Testing Decisions

- Good tests drive plain state through its public interface and check what comes out. No window, no game. Drawing code in `ui/` has no tests, as before.
- **Connection and notices.** Feeding `Notice`s to the status reducer gives the connection and the errors to show, including the repeated-error rule.
- **Bridge.** Notices sent into the forwarding function arrive in order on the async channel; the stop flag ends it.
- **Runs list.** Built from a temporary `History` with Events inserted: newest first, upserting a changed Run keeps order, a new Run goes on top, a rebinding leaves one row, the live Run follows the notices.
- **Selection and moment.** Pure functions over `Vec<Event>`: latest Snapshot, nearest at or before, an Event with none uses an earlier one, an empty Run gives none, CombatStart contributes its player side.
- **Board.** Layout by socket, sizes, collisions, overflow; badges and tooltip lines from attributes; name fallbacks.
- **Events and detail.** One row per kind, sub-rows, the day shown only on change, `m:ss`, options with the pick marked, combat detail with both boards, an Event that is gone.
- `cargo build`, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean after every ticket.
- What can only be seen with the game running is listed in each ticket's Comments as a by-hand check.

## Out of Scope

- The uploads UI (sync pill, "See on Bazaar DB", the settings fields for the key and sync) and anything the previous spec put out of scope: card art, auto-update, admin relaunch, macOS and Linux.
- Deleting Runs from the sidebar, a refresh button, hour separators and encounter banners in the Events list.
- Polling the live board between Events.
- Changes to the memory reader. Combat `frame_count` and attribute `100` are open in the records; the app shows what the tracker saves.
- Changing `log_path` without restarting.

## Further Notes

- The records that decide the look are `docs/records/2026-10-31-folder-structure-and-ui.md` and the drawn window `docs/records/2026-10-31-ui-mockup.html`. The live checks are `docs/records/2026-10-31.00{1,2,3}-*.md`.
- The tracker service, its `Notice` type and the `track` example are the prior art for starting and reading it.
- Nothing here can be seen without the game, so the tickets finish as `ready-for-human` with their by-hand checks.
