# Workspace, crates and app frame

Status: ready-for-agent

## Problem Statement

The repo is a bare `cargo new` stub. The decisions about how the rewrite is laid out, themed and windowed (see the 2026-10-31 folder-structure-and-ui record and the architecture document) exist only on paper. There is nowhere to put the memory-reader spike, the log and run state machine work, or the UI, and there is no window to look at. A player who opens the app today gets "Hello, world!" on a console.

## Solution

Turn the repo into the agreed workspace and give it a working, empty app. The player launches **Bazaar Tracker** and gets a native window in Rosé Pine with wordle-gpui's frame: a flat title bar with the ☰ menu, a ⌘P command palette that runs the same commands as the menu, a theme picker with ten themes, a status bar, and three island panels (the runs sidebar, the main run panel and the detail panel) that can be shown, hidden and resized, each showing an empty state. The chosen theme and the window layout are remembered between launches in `settings.toml`. Only one copy of the app runs at a time, and it writes a log file. Developers get the two library crates with their module skeletons, a spike entry point for the memory reader, and a `justfile` to run, test and build everything.

Behind the window, the two library crates are built out in the order the feasibility record set, riskiest first. `unityspy-rs` attaches to the running game and reads its Mono objects, proven step by step with the `dump-classes` spike against the live game. `bazaar-tracker` follows `Player.log`, reads Snapshots from game memory, runs the run state machine that turns both into Events, saves Runs and Events to its own `history.db`, looks cards up in the game's card catalog, and can upload runs to Bazaar DB (off by default). A headless `track` example ties it all together: it follows a live game and prints each Event as it is saved.

The app does not show run data yet; the board, stash, skills, events and detail views come in a later spec built on this one.

## User Stories

1. As a player, I want the app to open a window titled "Bazaar Tracker", so that I can tell it apart from other tools.
2. As a player, I want the app to start in the Rosé Pine theme the first time I open it, so that it looks good without any setup.
3. As a player, I want a flat title bar with only the app's name and a ☰ button, so that the window looks uncluttered.
4. As a player, I want the window's minimise, maximise and close buttons to work in the custom title bar, so that it behaves like any other Windows app.
5. As a player, I want to drag the window by its title bar, so that I can move it.
6. As a player, I want clicking ☰ to open a menu of commands, so that I can find everything the app can do.
7. As a player, I want each menu row to show its shortcut, current value or an on-dot, so that I can see state and learn shortcuts at a glance.
8. As a player, I want Escape or a click outside to close the menu, so that I can back out without choosing anything.
9. As a player, I want typing to be ignored by the rest of the window while the menu is open, so that I don't trigger things by accident.
10. As a player, I want Ctrl+P to open a command palette in the middle of the window, so that I can run any command from the keyboard.
11. As a player, I want the palette to list the same commands as the menu, so that I only have to learn one set.
12. As a player, I want to filter the palette by typing loose fragments of a command's name, so that I can find commands quickly.
13. As a player, I want Up and Down to move the highlight and Enter to run the highlighted command, so that I never need the mouse.
14. As a player, I want pressing Ctrl+P again, Escape or clicking outside to close the palette, so that I can dismiss it any way I like.
15. As a player, I want only one of the menu, the palette and the theme picker open at a time, so that overlays never stack up.
16. As a player, I want a **Change theme…** command and Ctrl+K Ctrl+T to open a theme picker, so that I can change how the app looks.
17. As a player, I want the theme picker to list ten themes, dark ones first and light ones after under headings, so that I can find the kind I want.
18. As a player, I want to type in the theme picker to filter by name or by "dark"/"light", so that I can jump to a theme.
19. As a player, I want the window to restyle as I move through the theme picker, so that I can preview each theme before choosing.
20. As a player, I want Escape in the theme picker to restore the theme I had, so that previewing is safe.
21. As a player, I want the theme I choose to still be there the next time I open the app, so that I set it only once.
22. As a player, I want the app to fall back to Rosé Pine if my saved theme no longer exists, so that a bad setting never stops it from opening.
23. As a player, I want the **Change theme…** menu row to show the current theme's name, so that I know what is selected.
24. As a player, I want a runs sidebar on the left that I can show and hide with Ctrl+B, a menu row and a status-bar button, so that I can give the run more room.
25. As a player, I want to drag the edge between the sidebar and the main panel to resize the sidebar within sensible limits, so that I can fit long run rows.
26. As a player, I want the sidebar to say "No runs yet — start a run in The Bazaar and it will appear here." while there are none, so that I know the app is working and what to do next.
27. As a player, I want a main run panel in the middle that tells me to select or start a run while none is selected, so that the empty window still explains itself.
28. As a player, I want a detail panel on the right that I can show and hide with Ctrl+Alt+B, a menu row and a status-bar button, so that I can open it when I need event details.
29. As a player, I want the panels drawn as rounded islands on the window colour with the title bar and status bar flat, so that the layout reads clearly in every theme.
30. As a player, I want a status bar along the bottom with the sidebar button on the left, so that the most common toggle is always in reach.
31. As a player, I want the status bar to show the tracker's connection state as a coloured dot and a word, so that I can see whether it is following the game.
32. As a player, I want errors to appear in the status bar in the error colour and stay until I dismiss them with ✕, so that I don't miss them.
33. As a player, I want short notices (like "Copied") to appear in the status bar and disappear by themselves, so that they don't need dismissing.
34. As a player, I want the sidebar's open state and width, and the detail panel's and stash section's open states, to be remembered between launches, so that the window comes back the way I left it.
35. As a player, I want a **Reset layout** command that restores the default panel layout and window size, so that I can recover from a layout I don't like.
36. As a player, I want a **Settings…** command that opens settings as an overlay in the window, so that I can find where options will live.
37. As a player, I want the settings overlay to close with Escape, ✕ or a click outside, so that it behaves like the other overlays.
38. As a player, I want Ctrl+Q to quit and closing the window to quit, so that the app never lingers in the background unexpectedly.
39. As a player, I want opening the app a second time to bring the existing window forward rather than start a second copy, so that two trackers never fight over the same data.
40. As a player, I want my settings and history kept in `%APPDATA%\BazaarTracker\`, so that they live in the normal place for Windows apps and are separate from any other tracker.
41. As a player, I want the app to keep a log file in its data folder with rotation, so that I can attach it to a bug report without it growing forever.
42. As a player, I want a missing or partly written `settings.toml` to be filled in with defaults rather than refused, so that upgrading or hand-editing never breaks start-up.
43. As a player, I want the shortcuts shown in the menu and palette as keys in Windows notation (Ctrl+B), so that they match my keyboard.
44. As a developer, I want a debug build to keep its settings, history and logs in `.bazaar-tracker/` at the repo root, so that development never touches my real data.
45. As a developer, I want `BAZAAR_TRACKER_DATA_DIR` to override the data folder in any build, so that I can point tests or a release build at a scratch folder.
46. As a developer, I want the root package to be the app and the libraries to live in `crates/<crate-name>/` with matching kebab-case names, so that the layout is predictable.
47. As a developer, I want a `unityspy-rs` crate with its `mem` and `mono` module skeletons, Windows-only, so that the memory-reader spike has a home.
48. As a developer, I want a `dump-classes` example in `unityspy-rs` that builds and runs, so that build step 1 can start immediately.
49. As a developer, I want a `bazaar-tracker` crate with its `log`, `memory`, `run`, `enums`, `history`, `catalog` and `upload` module skeletons and a fixtures folder for recorded logs, so that build steps 2–3 have a home.
50. As a developer, I want `just run`, `just test`, `just spike` and `just release`, so that I don't have to remember which cargo flags cover the whole workspace.
51. As a developer, I want `just test` to test every crate in the workspace, so that a plain root `cargo test` covering only the app never hides a failure.
52. As a developer, I want the app split into `shell`, `overlays`, `theme` and `ui` layers that only use the layers above them, so that state and key handling can be tested without a window.
53. As a developer, I want each theme in its own file mapping its palette onto shared roles, so that adding a theme means adding one file and one list entry.
54. As a developer, I want the theme roles to include outcome, tier and badge colours now, so that the board and event views can be built later without touching every theme.
55. As a developer, I want tests beside the code they test as `<name>_tests.rs` files, so that the convention matches wordle-gpui and is easy to follow.
56. As a developer, I want `just spike` to find The Bazaar's process and list its loaded assemblies, so that I know the Mono root domain and offsets are right before building anything on them.
57. As a developer, I want the spike to print the fields of `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache` with offsets and type codes, so that I can check them against the field paths in the notes.
58. As a developer, I want the spike to read one known static and one collection count, so that I can cross-check the value readers against what the game shows.
59. As a developer, I want the memory reader to fail with a clear error when the game, the Mono module or a class is missing, so that I am not left with a null-pointer read later.
60. As a developer, I want every Mono structure read through one `Memory` trait, so that tests can feed the reader byte buffers instead of a live game.
61. As a developer, I want the memory reader to avoid UnitySpy's known bugs (wrong primitive widths, swapped signedness, 32-bit native ints, unbounded strings, ordinal-blind export lookup, unchecked root-domain bytes), so that values read are correct.
62. As a player, I want the tracker to start following `Player.log` from its end, so that an old session is never replayed as new Events.
63. As a player, I want the tracker to notice when the game truncates or replaces `Player.log`, so that tracking keeps working across game restarts.
64. As a player, I want a Run to start when I start a run in the game and end when it ends, so that my history matches what I played.
65. As a player, I want Events for options offered, items and skills selected, items sold, combats started and combats ended, so that I can review the decisions of a run.
66. As a player, I want each combat Event to carry the Snapshot of both boards from just before the fight, so that I can see what I was up against.
67. As a player, I want a Run that began before the tracker knew its real id to be merged into its real id once it is known, so that one run never shows up twice.
68. As a player, I want my Runs and Events saved to `history.db` as they happen, so that nothing is lost if the app or the game closes mid-run.
69. As a player, I want to delete a Run from my history, so that I can remove runs I don't want kept.
70. As a player, I want card ids turned into names, tiers, sizes and descriptions from the game's own card data, so that Events are readable.
71. As a player, I want uploading to Bazaar DB to be off until I turn it on and give a secret key, so that nothing leaves my machine without my say.
72. As a player, I want uploads to retry with back-off and never block tracking, so that a server problem doesn't lose or delay my history.
73. As a player, I want my secret key kept in the Windows credential store, so that it is not lying in a plain file.
74. As a developer, I want a debug build to use a different credential name for the secret key, so that development never overwrites my real key.
75. As a developer, I want the run state machine driven by recorded `Player.log` files and a fake game memory in tests, so that its subtle timing rules are checked without the game.
76. As a developer, I want a `track` example that follows the live game headlessly and prints Events as they are saved, so that the whole tracker can be tried before the UI shows runs.

## Implementation Decisions

- **Workspace.** The root manifest is both the `bazaar-tracker-gpui` package (the app) and the workspace, with members `crates/*`. Folder names equal package names, in kebab-case. Rust edition 2024.
- **`unityspy-rs`.** Windows-only at the crate level. Two module groups: `mem` (process, reader with the `Memory` trait for raw reads, module) and `mono` (pe, domain, offsets, class, field, statics, object, value, bcl), following the UnitySpy porting notes' component list. It knows nothing about The Bazaar. The first ticket creates them as documented skeletons and a `dump-classes` example that prints that it is not implemented; later tickets fill them in.
- **Attaching.** Find the process by name and open it with `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION` only. Find `mono-2.0-bdwgc.dll` case-insensitively. Look up `mono_get_root_domain` by reading only the PE headers and export directory, resolving through `AddressOfNameOrdinals`. Check the `48 8B 05 … C3` bytes before trusting the `rel32`. Choose a `MonoOffsets` table by the game exe's `FileVersion`, starting with the 2021/2022 x64 values and the fork's unverified Unity 6 values. Walk the domain's assembly list to `TheBazaarRuntime`.
- **Classes and fields.** Class lookup is lazy: walk the image's class cache comparing name and namespace, stop once found, cache by full name. The class model reads the whole `MonoClass` in one call. Field lookup searches the parent chain and can filter by declaring type.
- **Values.** Statics are read through `vtable + VTable + vtable_size*8`; a null vtable is a distinct "not initialised" error. Objects resolve fields on their runtime class. Primitives are read at their correct widths (`R4` 4 bytes, `R8` 8, `I1` signed, `U1` unsigned, `I`/`U` 8 bytes on x64). Strings are length-capped (4,194,304 UTF-16 units) and reject negative lengths. Inline structs use offset − 16. A failed value-type read is an error, never a silent int. Collections (`List`, `Dictionary`, `HashSet`, `Nullable`) resolve their field offsets by name from the class, not hard-coded.
- **Errors.** Every missing process, module, export, assembly, class or field is a typed error naming what was missing.
- **`bazaar-tracker`.** Modules `log`, `memory`, `run`, `enums`, `history`, `catalog`, `upload`, plus a recorded-logs fixtures folder. It must not depend on gpui.
- **Log following.** Start at the end of `Player.log`; wait for the file if it doesn't exist; poll every 500 ms. Reset to the start when the file shrinks or its tail signature no longer matches. Read at most 16 MiB per pass and decode only up to the last newline. Strip the timestamp prefix and parse lines with the six line patterns recorded in the log-and-events notes (state changed, skill selected, game-sim summary, card placed, command completed, combat prepared), seven patterns counting the prefix, each guarded by a plain substring check first, into typed log lines.
- **Domain types.** Run, Event and Snapshot types as in the glossary. A Snapshot holds the board, stash and skills (each item with instance id, template id, type, tier, size, socket, enchantment, attributes and tags) and the player stats (health, max health, gold, prestige, level, income, day, hour, victories, losses, hero). Event kinds follow the original's event types, including run started, options presented, selected, sold, combat start, combat end, run ended. Enum values (tiers, sizes, enchantments, attributes, tags, hero, run state and the rest listed in the memory-reading notes) are built in; reading them from the game's DLL is out of scope.
- **Game memory.** A trait with one method per read the run state machine needs (board Snapshot, run init with the real run id, combat result, combat frames, PvP opponent, selected hero, player rank). A fake implementation returns scripted values for tests. The real implementation uses `unityspy-rs` and the field paths in the memory-reading notes, serialises all access behind one lock, reconnects every 500 ms while disconnected, and disconnects on process death or after five failed board reads in a row.
- **Run state machine.** Follows the original's `GameTracker` as described in the log-and-events notes: log lines are triggers, memory reads give the data; Events are built when they are sent, not when the line arrives, so Snapshots reflect the moment of sending; options settle after 500 ms, combat boards are captured 350 ms after entering combat, an undecided combat end retries after 1200 ms, a ranked run end waits 4000 ms for the rating; each timer is cancelled by a generation counter. A Run without a known real id starts with a `local-` id and is rebound once the real id is read. Time comes through an injectable clock so tests can drive the timers.
- **History.** Our own schema, not the original's: `runs` and `events` tables, event details as JSON, timestamps as integer milliseconds since the Unix epoch, foreign keys on, schema version in `user_version` with forward-only migrations. Operations: insert an Event (updating its Run's summary in the same transaction), list Runs newest first, list a Run's Events, load one Event, delete a Run, mark a Run ended or not, and rebind a Run id in one transaction that stays valid with foreign keys on. Writes are serialised; reads may run concurrently.
- **Tracker service.** A background service in `bazaar-tracker` that owns the log follower, the game memory, the run state machine and the history store, and reports connection changes and saved Events over a channel. The app will subscribe to it in a later spec; here it is exercised by a `track` example.
- **Card catalog.** Find the newest of `GameData.db` and `cards.json` under the game's `LocalLow\Tempo Storm\The Bazaar` folder (with or without `prod\`). Read `GameData.db` from a temporary copy so the running game is never locked out; fall back to `cards.json`. Each card gives name, starting tier, size, type and description, looked up case-insensitively by id.
- **Uploading.** Pick sendable Events in batches of up to 200 and about 2,000,000 payload bytes (always at least one), gzip the JSON batch and `POST` it with `Content-Encoding: gzip`, `Authorization: Bearer <key>` and `User-Agent: BazaarTracker/<version>`. Back off on failure, mark sent Events, set aside Events the server rejects for good, and never block tracking. Run sync is off by default and does nothing without a secret key. The endpoint is configurable; what the server accepts is still unknown (feasibility record, open question 3), so behaviour against the real server is not part of the acceptance criteria.
- **Secret key.** Stored in the Windows credential store through `keyring`, under `BazaarTracker` in release builds and `bazaar-tracker-dev` in debug builds.
- **App layers.** `shell` (single-instance lock, settings, data folder, logging, and placeholders for admin relaunch and updates), `overlays` (plain state structs with no gpui: commands, search, theme picker, settings form), `theme`, and `ui` (the `TrackerApp` view plus one drawing file per part: title bar, menu, palette, theme picker, status bar, sidebar, run header, board, events, detail, settings, shared widgets). Files whose views are out of scope may be left out until their spec; the structure above is the target.
- **gpui stack.** Pinned exactly as in wordle-gpui: `gpui-pre =0.3.9` renamed to `gpui`, `gpui-pre-platform` with `font-kit` and `runtime_shaders`, `gpui-base =0.7.3`, `gpui-kit-assets 0.7.3`, `gpui-component 0.7.3` with default features off. The title bar, menu, palette, search, theme picker, `Kbd` shortcuts and island styling are copied from wordle-gpui and adapted, not shared through a crate.
- **Overlays.** Each overlay is a struct that takes a key name and returns an outcome (run a command, close, keep open, preview or choose a theme). `TrackerApp` holds at most one open overlay and routes real key presses to it. The menu and palette share one list of commands: Settings…, Show sidebar, Show detail, Show run duration (a toggle that is saved but has no visible effect yet), Change theme…, Reset layout, Quit.
- **Search.** The palette and theme picker use wordle-gpui's loose matching (fragments in order, case- and accent-insensitive, so "rose" finds Rosé Pine).
- **Themes.** Ten themes: Rosé Pine, Catppuccin Mocha, Flexoki Dark, Tokyo Night, Everforest, Kanagawa Wave, Nord, Rosé Pine Dawn, Catppuccin Latte, Flexoki Light, with dark ones first. Rosé Pine is first and is the default. The `Theme` struct keeps wordle's generic roles (name, dark, background, surface, overlay, three highlights, text, subtle, muted, heading, error, success, accent), drops the Wordle-specific ones, and adds `win`, `loss`, `live`, the tiers `bronze`, `silver`, `gold`, `diamond`, `legendary`, and the badges `damage`, `shield`, `heal`, `burn`, `poison`, `regen`, `ammo`, `value`. Rosé Pine's mapping: win/live/heal/diamond = foam, loss/damage = love, shield/value/gold = gold, burn/bronze = rose, poison = pine, regen/legendary = iris, ammo/silver = subtle. Other themes use their own orange and green for burn, heal and poison where they have them.
- **Theme lookup.** By exact name, falling back to Rosé Pine for an unknown or missing name.
- **Data folder.** Resolved in this order: `BAZAAR_TRACKER_DATA_DIR` if set; otherwise in a debug build the repo's `.bazaar-tracker/` (fixed at compile time from the manifest directory); otherwise `%APPDATA%\BazaarTracker\`. Created if missing. It holds `settings.toml`, later `history.db`, and `logs/`.
- **Settings.** A TOML file with snake_case keys, every key optional with a default:

  ```toml
  theme = "Rosé Pine"
  log_path = ""
  run_sync = true
  show_run_duration = false

  [window]
  sidebar_open = true
  sidebar_width = 248
  detail_open = true
  stash_open = true
  ```

  Unknown keys are ignored. A file that fails to parse is logged and replaced by defaults in memory, never overwritten silently until the player changes a setting. Settings are saved as soon as they change. Sidebar width is clamped to the same range as wordle-gpui's sidebar.
- **Single instance.** A named Windows mutex; a second launch signals the first to show and focus its window, then exits.
- **Logging.** `tracing` to `logs/tracker.log` in the data folder, with size-based rotation keeping the current file and a few backups, and a start-up banner line with the version and process id.
- **Status bar.** Shows the sidebar toggle, a connection indicator (in this spec always "Not tracking"), a sticky error slot dismissed with ✕, transient notices that hide after 2 seconds, and the detail toggle.
- **Empty states.** The sidebar shows the "No runs yet" text; the main panel asks the player to start or select a run; the detail panel asks the player to select an event.
- **Window.** Opens centred at 1000×640 with a minimum of 820×520; the custom title bar must occlude its buttons so clicks on them are not taken as drags.
- **Naming.** The user-facing name is "Bazaar Tracker" everywhere; nothing reads, writes or names BazaarDBTracker's or Bazaar DB's files.

## Testing Decisions

- Good tests drive a module through its public interface and check what comes out (an outcome, a value, a file's contents), never private fields or how it got there. They run with no window and no game.
- **Overlays seam.** Type key sequences into the command palette, the menu's command list and the theme picker and check the outcomes: filtering, wrap-around movement, Enter running the highlighted command, Escape closing, the theme picker previewing on movement and restoring on Escape, and the menu and palette offering the same commands. Prior art: wordle-gpui's commands, theme picker and search tests.
- **Settings and data-folder seam.** Data-folder resolution is tested as a function of the env var, the build kind and the app-data path. Settings are loaded and saved against a temporary folder: an empty file gives every default, a partial file keeps given keys and defaults the rest, unknown keys are ignored, an unknown theme name resolves to Rosé Pine, an unparsable file gives defaults without being overwritten, and save-then-load round-trips. The sidebar width clamp is tested at both ends.
- **Themes seam.** Every theme in the list is reachable by its name; lookup of an unknown name gives Rosé Pine; Rosé Pine is first; dark themes come before light ones; search finds "Catppuccin Mocha" from "cmo" and "Rosé Pine" from "rose". Prior art: wordle-gpui's theme tests.
- **Memory seam (`unityspy-rs`).** A fake `Memory` serves byte buffers laid out like the real structures: a PE export directory (including a name whose ordinal differs from its index), root-domain instruction bytes (good and bad), an assembly list, a class cache, class and field structs, a vtable with statics, objects, strings (normal, empty, negative and over-cap lengths), arrays, and `List`/`Dictionary`/`HashSet`/`Nullable` layouts. Tests check the values and errors that come out, with one test for each UnitySpy bug listed in the porting notes. Prior art: none in this repo; the porting notes' bug list is the checklist.
- **Live checks.** The UnitySpy porting notes' verification steps 1–3 are run by hand with `just spike` against the game, and the real game-memory implementation is checked by hand with an example that prints a live Snapshot.
- **Log seam.** Fixture text for every pattern (matching and near-miss lines) and the parsed lines that come out; a temporary file that grows, gets a partial last line, is truncated and is replaced, checked through the follower's output.
- **Run seam.** Recorded `Player.log` files plus the fake game memory and a fake clock go into the run state machine; the Events it sends come out and are compared with expected Events. Cover a full run, options and selection, selling, a PvE and a PvP combat with Snapshots, a run whose real id arrives late and is rebound, and timers cancelled by newer lines. The tracker service is tested the same way, ending in rows in a temporary `history.db`.
- **History seam.** Against a temporary database: a fresh file gets the current schema; insert-then-list round-trips; Run summaries update as Events arrive; deleting a Run removes its Events; rebinding moves a Run and its Events under foreign keys; migrations run once.
- **Catalog seam.** Small fixture `GameData.db` and `cards.json` files: lookup by id ignores case, the newer file wins, a broken `GameData.db` falls back to `cards.json`, nothing found gives an empty catalog rather than an error.
- **Upload seam.** A fake HTTP server: batches respect the count and size caps, the body is gzip JSON with the right headers, failures back off, accepted Events are marked sent, nothing is sent while sync is off or the key is missing.
- Drawing code in `ui` has no tests, as in wordle-gpui. Single-instance and logging are checked by hand.
- `just test` must pass with `cargo test --workspace`; the two library crates' skeletons compile with no warnings.

## Out of Scope

- The board, stash and skills tiles, the run header and combat timeline, the event list and the detail panel's contents. They need run data and get their own spec.
- Real settings fields (secret key, run sync, log path); the settings overlay is a shell here.
- Connecting the app to the tracker service: the connection indicator, the runs list and the run views reflecting real data come in the run-views spec.
- Reading the game's enum values from `BazaarGameShared.dll`; the built-in values are used.
- The combat-frame serialiser (`ManagedJson`); combat frames are read only as far as the run state machine needs them to decide outcomes.
- Behaviour against the real Bazaar DB server, until the Bazaar DB team answers what it accepts.
- Admin relaunch and auto-update (Velopack).
- macOS and Linux builds.
- Card art.
- Any import from, or compatibility with, the original BazaarDBTracker.

## Further Notes

- Decisions and their reasons are in the 2026-10-31 folder-structure-and-ui record; the agreed folder tree and layering rules are in the architecture document. The window is drawn in the 2026-10-31 UI mockup (HTML) next to that record; it shows the later run views too, which this spec does not build.
- wordle-gpui lives at `D:\code\wordle\app\wordle-gpui`. Its gpui-app document explains the layering, the overlay pattern and the theme files this spec copies.
- wordle-gpui needs Rust 1.89 or newer for one of its dependencies; the same applies here.
