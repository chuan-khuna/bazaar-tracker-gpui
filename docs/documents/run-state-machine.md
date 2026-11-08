# Run state machine

How `crates/bazaar-tracker/src/run/` turns `Player.log` lines and game memory reads into Events,
and how the `Tracker` service in `crates/bazaar-tracker/src/tracker/` runs it in the background and
saves every Event to `history.db`. The terms (Run, Live run, Event, Snapshot, Board, Stash) are
defined in [GLOSSARY.md](../../GLOSSARY.md). The log lines come from [player-log.md](player-log.md),
the memory reads from [memory-reading.md](memory-reading.md), and the database is
[history-db.md](history-db.md).

## Files

| File | What it does |
|---|---|
| `run/event.rs` | `Run`, `Event`, `EventKind`, `Outcome`, `PresentedOption`, `is_local_run_id` |
| `run/machine.rs` | `RunMachine`: line dispatch, timers, the emit queue, memory reads; `Output`; the delay constants |
| `run/lifecycle.rs` | Where a Run starts, ends and is rebound; RunStarted and RunEnded |
| `run/options.rs` | Options offered, selections, merges, sales, rerolls |
| `run/combat.rs` | Board capture, CombatStart, CombatEnd and how the outcome is decided |
| `run/clock.rs` | `Clock`, `SystemClock`, `ManualClock` |
| `run/test_support.rs` | The test `Harness`: a machine on `FakeMemory` and a `ManualClock`, ids `e1`, `e2`, ... |
| `tracker/mod.rs` | `Tracker`, `TrackerConfig`, `Notice`; its threads and the save queue |
| `tracker/store.rs` | `Store`, the trait the service saves through; `Arc<History>` implements it |

## The idea

Log lines are triggers and keys; memory gives the data. A line says *that* something happened (a
card was placed, the state changed, a command succeeded). The machine queues the work and builds
the Event when the work reaches the front of the queue, reading memory at that moment. So an
Event's Snapshot is the Board as it was when the Event was built, not when the line arrived.

```
Player.log line ──► on_line ──► dispatch ──► queue of Emit ──► pump ──► build_* (memory reads) ──► Output
                                    │                             ▲
                                    └── timers (Clock) ── tick ───┘
```

## Run, Event and Outcome

`Event` is one thing that happened in a Run:

| Field | What it holds |
|---|---|
| `id` | A unique id, made when the Event is made (`random_id()`: 32 hex digits) |
| `run_id` | The game's run id, or a `local-` id until that is known |
| `timestamp_ms` | When the Event was made, from the machine's `Clock`, in Unix ms. The log's own `[hh:mm:ss.fff]` is never used |
| `day`, `hour` | From the Board read that built it; filled from the last Board read that had one when missing |
| `kind` | An `EventKind`, flattened into the same JSON object |

`Run` is the summary the history lists: `id`, `started_ms`, `ended_ms`, `ended`, `hero`, `day`,
`victories`, `losses`, `outcome`. A Run whose `ended` is false is the Live run, or one the tracker
lost. `is_local_run_id(id)` is true when `id` starts with `local-`.

`Outcome` is `Unknown` (the default), `Win` or `Loss`; `as_str()` gives the same names, which is
what the database stores.

## Event kinds

The JSON is our own, not the original tracker's: snake_case keys, a `type` tag naming the kind, and
`Option` fields that are `None` left out (not written as `null`). Inside a Snapshot, though, every
`Item` and `PlayerStats` field is written, `null` included.

| `type` | Triggered by | Holds |
|---|---|---|
| `RunStarted` | A new Run: the run-init line, a different run id in memory, or the first activity with no Run | `play_mode`, `build_id`, `environment` (from memory's run init), `hero`, `rating`, `rank`, `rank_division` (from the player's rank) |
| `OptionsPresented` | Options that settled 500 ms after a deal, or the current deal just before a choice is reported | `context` (the option state, such as `ChoiceState`), `options` (a list of `PresentedOption`), `snapshot` |
| `Selected` | `Card Purchased` (`CardPlaced`) for an offered card, `Selected skill`, or a merge | `card_kind`, `instance_id`, `template_id`, `tier`, `enchantment`, `section`, `target`, `merged_into`, `options_event_id`, `snapshot` |
| `Sold` | A successful command whose name contains `sellcard` | `instance_id`, `template_id`, `tier`, `enchantment`, `snapshot` (one Event for each item that left the player) |
| `Rerolled` | A successful command whose name contains `reroll` | Nothing of its own |
| `CombatStart` | `Combat prepared` | `player` and `opponent` Snapshots, `is_pvp`, `opponent_name`, `opponent_hero`, `opponent_rating`, `opponent_level` |
| `CombatEnd` | Leaving a combat state | `outcome`, `combat_start_event_id`, `player_hp_after`, `opponent_hp_after`, `victories_after`, `losses_after`, `sim_winner`, `sim_loser`, `sim_frames`, `expected_sim_frames`, `sim_diag` |
| `RunEnded` | `EndRunVictoryState`, `EndRunDefeatState`, `AbandonRunCommand`, or a different run id | `outcome`, `victories`, `losses`, `gold`, `health_max`, `prestige`, `level`, `income`, `rating_after`, `rank_after`, `rank_division_after` |

`PresentedOption` has `instance_id`, `kind` (`Item`, `Encounter`, `EncounterStep`, `Skill`,
`Combat` or `Unknown`, from the id's prefix `itm_`, `enc_`, `ste_`, `skl_`, `com_`/`pvp_`), `size`
(`Unknown` when the card was never seen in a Board read), and optional `template_id`, `tier` and
`enchantment`. `EventKind::name()` returns the `type` tag.

A Rerolled Event, exactly as `event_tests.rs` expects it:

```json
{"id": "e1", "run_id": "run-1", "timestamp_ms": 1761900000123, "day": 2, "hour": 3, "type": "Rerolled"}
```

A Selected Event (Snapshot shortened):

```json
{
  "id": "e1", "run_id": "run-1", "timestamp_ms": 1761900000123, "day": 2, "hour": 3,
  "type": "Selected",
  "card_kind": "Item", "instance_id": "itm_a", "template_id": "t", "tier": "Bronze",
  "section": "Player", "target": "PlayerSocket_3", "options_event_id": "e0",
  "snapshot": {
    "board": [{"instance_id": "itm_EuUtMwz", "template_id": "638a6b50-...", "card_type": "Item",
               "tier": "Silver", "size": "Medium", "socket": "Socket_2", "enchantment": "Fiery",
               "attributes": {"Cooldown": 4000, "DamageAmount": 30}, "tags": ["Weapon", "Tool"]}],
    "stash": [], "skills": [...],
    "stats": {"health": 300, "health_max": 300, "gold": 12, "prestige": null, "level": null,
              "income": null, "day": 2, "hour": 3, "victories": 1, "losses": 0, "hero": "Jules"}
  }
}
```

A CombatEnd Event:

```json
{"id": "e7", "run_id": "run-1", "timestamp_ms": 1761900000123, "day": 2, "hour": 3,
 "type": "CombatEnd", "outcome": "Win", "combat_start_event_id": "e5",
 "player_hp_after": 210, "opponent_hp_after": 0, "victories_after": 3, "losses_after": 1,
 "sim_winner": 0, "sim_loser": 1, "sim_frames": 659, "expected_sim_frames": 659}
```

## The machine

`RunMachine` has no threads and no timers of its own. It is fed and drained:

| Call | What it does |
|---|---|
| `RunMachine::new(memory, clock)` | A machine on a `GameMemory` and a `Clock`, with random ids |
| `RunMachine::with_ids(memory, clock, new_id)` | The same with a chosen id source (tests use a counter) |
| `on_raw_line(raw)` | Parses one raw `Player.log` line (lines it does not know are skipped), handles it, then `tick()` |
| `on_line(&LogLine)` | Fires due timers, learns the run id if it can, dispatches the line, pumps the queue |
| `tick()` | Lets time pass: fires due timers and pumps the queue |
| `next_due_ms()` | The earliest timer or the end of a hero-poll block, so the caller knows when to `tick` |
| `take_outputs()` | Everything produced since the last call, in order |
| `run_id()`, `run_has_ended()` | The Run being followed |
| `read_live()` | A Board read for the live view, through the machine's own memory; nothing is kept from it |
| `reset()` | Forgets the Run and drops pending timers and queued work; outputs not yet taken are kept |

`Output` is one of:

| Variant | Meaning |
|---|---|
| `Output::Event(Event)` | An Event to save |
| `Output::Rebound { old_run_id, new_run_id }` | A Run that began under a `local-` id has this real id: its Events should move |

### Clock and timers

The machine never reads the system clock. `Clock::now_ms()` gives Unix ms; `SystemClock` is real
time, and `ManualClock` only moves on `advance(ms)` or `set(ms)` (clones share one time), so a test
drives every delay by hand.

A delay is a `Timer { due_ms, generation, kind }`. `schedule(delay_ms, generation, kind)` sets
`due_ms` to now plus the delay. Due timers fire in `due_ms` order on the next `on_line` or `tick`.

| `TimerKind` | Cancelled by | When it fires |
|---|---|---|
| `Settle` | `settle_generation` moving on | Queues the settled options, if the generation is current and the state is still an option state |
| `CombatCapture` | `combat_capture_generation` moving on | Reads both Boards, if still current, still in combat, and the fight is not being played |
| `CombatEndRetry` | Nothing (generation 0) | Queues the CombatEnd again for its second and last try |
| `RankSettle` | Nothing (generation 0) | Queues the RunEnded again to look at the rating |

A newer line cancels an older timer by bumping its counter: the old timer still fires, sees its
generation is stale, and does nothing. `reset()` drops every timer.

### The emit queue

Work waits in a queue of `Emit` (`RunStarted`, `Options`, `Selected`, `Merge`, `Sold`, `Rerolled`,
`RunEnded`, `CombatStart`, `CombatEnd`). `pump()` builds them front to back. A RunStarted still
waiting for the hero puts itself back at the front and sets `blocked_until_ms` to now + 500 ms;
everything behind it waits, so no Event of a Run comes out before its RunStarted.

### Delays

| Constant | Value | Use |
|---|---|---|
| `SETTLE_DELAY_MS` | 500 ms | Options settle after the game deals them |
| `COMBAT_CAPTURE_DELAY_MS` | 350 ms | Boards are captured after a combat state is entered, or after cards spawn before the fight |
| `COMBAT_END_RETRY_MS` | 1200 ms | Second try at a combat outcome nothing decided |
| `RANK_SETTLE_MS` | 4000 ms | How long a ranked run end waits for the rating |
| `RANK_POLL_MS` | 500 ms | How often the rating is looked at meanwhile |
| `HERO_POLL_MS` × `HERO_POLLS` | 500 ms × 6 (3 s) | How long a run start waits for the hero |
| `DEALT_PREVIOUS_CAP` | 40 | How many retired option ids are remembered |

## Lifecycle

### Learning the run id

`on_line` first looks for the run's identity. On the run-init line (`NetMessageRunInitialized`) it
reads memory's `RunInit` (`run_id`, `play_mode`, `build_id`, `environment`). On any other line it
does so too while there is no Run, a run boundary is pending, or the Run has a `local-` id, and only
if a Board read succeeds.

- **Run-init line, memory already knows:** `resolve_run_init`.
- **Run-init line, memory does not know yet:** `run_boundary_pending` is set; the next line that
  finds a run id resolves it.
- **`resolve_run_init(info)`:** no id in `info` starts a Run only if there is none. The same id as
  the followed Run changes nothing. A `local-` Run that has not ended is rebound. Anything else ends
  the current Run with `Outcome::Unknown` and starts the new one.

### Starting a Run

`start_run` takes the real id from `RunInit`, or makes `local-` + a random id. It clears everything
about the old Run (selected, sold and reported ids, the deal, the card index, the last stats) and
queues a RunStarted.

`ensure_run` is called by every activity (a placed card, a skill, a merge, a reroll, a sale, a
fight). With no Run it starts one from the run init it has, or reads one if a Board read succeeds.
A run id that already ended is never started again: the Run gets a fresh `local-` id instead. Once
a Run is set, even an ended one, `ensure_run` never starts another; only a run init with a
different id does. It marks the Run as having activity.

Building RunStarted reads the hero from the Board, else from `read_selected_hero`. With no hero it
tries again every 500 ms, six times (3 s), then sends the Event without one. It also reads the
player's rank and keeps the rating as `pre_run_rating`.

### Rebound

A Run that started under a `local-` id and has not ended is rebound when memory gives its real id:
`run_id` changes, `play_mode` is taken if given, and `Output::Rebound { old_run_id, new_run_id }`
is produced. A real id is never rebound, and neither is an ended `local-` Run (a new run id then
starts a new Run). The service applies the rebinding to the history (see
[history-db.md](history-db.md#rebinding-a-runs-id)).

### Ending a Run

| Trigger | Outcome |
|---|---|
| State `EndRunVictoryState` | `Win` |
| State `EndRunDefeatState` | `Loss` |
| `AbandonRunCommand` succeeded | `Loss` |
| Memory names a different run id | `Unknown` |

`end_run` does nothing unless there is a Run that has not ended and has had activity. It marks the
Run ended, remembers its id in `ended_run_ids`, and queues a `PendingRunEnd` with the last stats and
`pre_run_rating`.

A Run is ranked when its `play_mode` is `Ranked` (any case). A ranked run end waits for the rating
to change: while `pre_run_rating` is known and the rating read now is the same or missing, it waits
500 ms and looks again, for up to 4000 ms in all, then sends whatever it finds. An unranked end is
sent at once. RunEnded takes its stats from a Board read at that moment, falling back to the last
stats for `day`, `hour`, `victories` and `losses`, and its `rating_after`, `rank_after` and
`rank_division_after` from the rank read.

## Options, selections, sales and rerolls

The option states are `ChoiceState`, `EncounterState`, `LootState`, `LevelUpState` and
`PedestalState`.

| Line | What happens |
|---|---|
| State changes to the same option state | The settle restarts |
| State changes to a different state | The current deal is retired into the previous ids (at most 40, oldest dropped). Into an option state: a settle starts. Out of the option states: a waiting settle is cancelled |
| `GameSim cards: dealt=N` with N > 0 | The options are marked dirty; in an option state the settle restarts |
| `GameSim cards: spawned=N` with N > 0 | In combat before the fight is played, the board capture restarts |

**Settle.** When the 500 ms settle fires, the machine reads the Board (only if there is a Run and
cards were dealt since the last report; a failed read keeps them dirty), takes the Board's
`selection_set` as the current deal, drops ids already reported, and emits OptionsPresented with
the player's Snapshot. Each reported id is mapped to that Event's id.

**Selection.** A `Card Purchased` line is a selection when the card was not selected before and is
in the current deal, in the previous ids, or not one of the player's known items. If it is in the
current deal, the current deal is reported first (an unsettled OptionsPresented), so the options
always come before the choice. A `pvp_` id marks the coming fight as PvP. The Selected Event takes
`template_id`, `section` and `target` from the line, `tier` and `enchantment` from the card index,
`options_event_id` from the option map (absent when the choice came before the options settled),
and a Snapshot read when it is built. `Selected skill skl_… to socket …` does the same for a skill,
with the socket as `target` and no template id.

**Merge.** Buying a copy of an item the player already has upgrades it, and the offered copy
disappears. A `Card Purchased` for a card the player has, in an option state, for an `Item`, makes
the candidates the offered items not yet selected. After a Board read, if exactly one candidate is
gone from the game, it gets a Selected Event with `merged_into` set to the placed item's id.

**Commands.** Only `result=success` counts. `AbandonRunCommand` ends the Run as a loss. A command
whose name contains `reroll` (any case) emits Rerolled. One that contains `sellcard` remembers the
player's items; when built, each of those items no longer on the Board, Stash or skills and not
sold before gets its own Sold Event.

## Combat

The combat states are `CombatState` and `PVPCombatState`.

1. **Entering combat** sets `is_pvp` for `PVPCombatState`, forgets the last fight, and schedules a
   board capture in 350 ms. Cards spawning before the fight is played push the capture back by
   another 350 ms.
2. **The capture** reads both Boards and keeps them as `pre_board`, if its generation is current,
   the state is still a combat state and the fight has not started. It checks again after the read,
   since the fight may start meanwhile.
3. **`Combat prepared: frames=N`** keeps N as the expected frame count. It is ignored in an
   `EndRun…` state, or after the Run ended outside combat. Otherwise it ensures a Run, cancels any
   settle and capture, marks the fight as playing, picks the CombatStart's id now (so CombatEnd can
   refer to it), keeps the victories, losses, day and hour from `pre_board`, and queues CombatStart.
   CombatStart uses `pre_board`, or a fresh Board read if there was no capture. For a PvP fight it
   reads the opponent's name, hero, rating and level; `opponent_hero` falls back to the opponent
   Board's hero.
4. **Leaving combat** for a non-combat state queues CombatEnd. Into `EndRunVictoryState` or
   `EndRunDefeatState` the outcome is that state's, and the CombatEnd comes before the RunEnded. A
   fight with no CombatStart (the tracker joined late) has nothing to end.

The outcome is decided in this order, first match wins:

| Source | Win | Loss |
|---|---|---|
| The end-of-run state left into | `EndRunVictoryState` | `EndRunDefeatState` |
| The simulation | winner 0, loser 1 | winner 1, loser 0 |
| The counters against `pre_board` | victories grew | losses grew |
| The Board's run state | end-run victory | end-run defeat |
| HP | opponent HP ≤ 0 | player HP ≤ 0 |

The simulation's verdict is ignored when its frame count is not the expected one; `sim_diag` is
then `stale (frame mismatch)`. If nothing decides, the CombatEnd is tried again after 1200 ms, and
the second try is sent whatever it finds, `Unknown` included.

A PvP result can reach the counters after the state changes, so for a PvP win `victories_after` is
at least the victories before + 1 (and `losses_after` likewise for a loss). CombatEnd's `day` and
`hour` are the ones from before the fight.

## The Tracker service

`Tracker` follows `Player.log`, runs the machine and saves its outputs, on its own threads. It never
blocks the caller.

| Thread | When | What it does |
|---|---|---|
| `tracker-log` | Always | `LogFollower::run_every(poll_interval)`: sends each new line to the machine thread; a read error becomes `Notice::LogError` |
| `tracker-connection` | When given a connection receiver | Passes attach and detach changes to the machine thread, in order with the lines (checks every 25 ms) |
| `tracker-machine` | Always | Runs the `RunMachine` and applies its outputs to the `Store` |
| `tracker-game` | `Tracker::start_live` only | `BazaarMemory::run_connection_loop`: keeps the game attached |

`Tracker::start(config, memory, store, clock, connection)` takes any `GameMemory`, `Store` and
`Clock`, which is how the tests run it on `FakeMemory`. `Tracker::start_live(config, history)`
(Windows) builds a `BazaarMemory`, reports its connection changes, and saves to the `History`.
`notices()` gives the receiver; `stop()` or dropping the `Tracker` sets the stop flag, sends `Stop`
and joins every thread.

`TrackerConfig::new(log_path)` sets:

| Field | Default | Meaning |
|---|---|---|
| `poll_interval` | 500 ms | How often the log is looked at |
| `retry_delay` | 1 s | Delay before the first retry of a failed save |
| `max_retry_delay` | 30 s | The retry delay doubles up to this |
| `live_interval` | `Some(1 s)` | How often the Board is read for `Notice::LiveSnapshot`; `None` turns it off |

### The machine thread

Each pass waits for input for at most 25 ms, less if a machine timer or a save retry is due sooner.

- A line goes to `on_raw_line`.
- A connection change is reported as `Notice::Connection`. A `Disconnected` change also calls
  `machine.reset()`, so the Run is followed afresh when the game is found again.
- A timeout calls `tick()`.
- `Stop`, or the input channel closing, ends the loop.

Then the machine's outputs join a queue of `Op::Save(Event)` and `Op::Rebind { old, new }`, which
`apply_pending` applies in order.

### Retrying failed saves

A save or rebind that fails stops the queue at that item: the service sends
`Notice::SaveFailed { what, error, retry_in }` and tries the same item again after `retry_in`. The
delay starts at `retry_delay` and doubles after each failure up to `max_retry_delay`; a success
resets it. Later items wait behind the failed one, so nothing is dropped and the history keeps the
order. `what` names the item, such as `event e1 (RunStarted)` or `run local-… as 3f2a…`.

### Live Snapshot polling

While the game is attached and `live_interval` is set, the machine thread reads the Board through
`machine.read_live()` once per interval, in turn with the machine's own reads. It sends
`Notice::LiveSnapshot` only when the read differs from the last one sent. Attaching or detaching
resets the timer and the last value. A failed read sends nothing. Live reads are never saved.

### Notices

| `Notice` | Sent when |
|---|---|
| `Connection(ConnectionChange)` | The game was found (`Connected { pid, unity_version, details }`) or lost (`Disconnected { reason }`) |
| `EventSaved(Event)` | An Event was saved |
| `RunRebound { old_run_id, new_run_id }` | A Run's `local-` id was replaced in the history |
| `SaveFailed { what, error, retry_in }` | A save or rebind failed; it is tried again in `retry_in` |
| `LogError(String)` | Reading `Player.log` failed; it is tried again on the next pass |
| `LiveSnapshot(Option<BoardState>)` | The Board changed; `None` means the game has no Board (no run) |

## `just tracker track`

`just tracker track` runs `crates/bazaar-tracker/examples/track.rs`: it follows the live game headlessly
and prints each notice. It saves to `history.db` in the data folder, found as the app does
(`BAZAAR_TRACKER_DATA_DIR`, else `.bazaar-tracker/` in a debug build, else
`%APPDATA%\BazaarTracker\`; see [data-folder.md](data-folder.md)). `--log <path>` picks another
`Player.log`. Stop it with Ctrl+C.

```
following C:\Users\you\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log
saving to D:\code\bazaar-tracker-gpui\.bazaar-tracker\history.db
[connection] Connected { pid: 4242, unity_version: "...", details: ... }
[saved] {"id":"…","run_id":"local-…","timestamp_ms":…,"day":1,"hour":0,"type":"RunStarted","hero":"Jules"}
[rebound] local-… -> 3f2a…
[save failed] event … (Selected): …; trying again in 1s
[log error] …
```

`LiveSnapshot` notices are not printed.

## Replaying a whole log

`crates/bazaar-tracker/tests/run_replay.rs` plays recorded logs from `tests/fixtures/logs/`
(`real-run-start.log`, `real-pvp-combat.log`) through a `RunMachine`:

1. `FakeMemory` shows one Board: hero `Jules`, gold 10, day 1, hour 0, 0–0, every card the log
   places known as a Bronze Small item, and the chosen ids on offer. Its run init is
   `3f2a9c1e-real-run` (`Normal`), and its combat result is winner 0, loser 1, 659 frames.
2. A `ManualClock` starts at `1_761_900_000_000` and follows the log's own timestamps: before each
   line it moves to every timer that falls due on the way, calls `tick()`, then sets the clock to
   the line's time and calls `on_raw_line`.
3. After the last line the clock moves 5000 ms and ticks once more, so pending settles, retries and
   rank waits finish.

The tests then check the Events: a RunStarted first, every Event in the real run, time order kept,
every placed card and the skill selected once in log order, one Rerolled, a CombatStart and a
CombatEnd that refers to it with a `Win` and 659 expected frames, the PvP fight marked `is_pvp`,
and the first OptionsPresented in `ChoiceState` with three options, stamped exactly 500 ms after
the log entered `ChoiceState`.

## Tests

| Test file | Covers |
|---|---|
| `run/event_tests.rs` | Every kind's JSON round trip, the flat shape with a `type` tag, absent values left out, `name()`, `is_local_run_id` |
| `run/machine_tests.rs` | Run start (real and `local-` id, hero wait and its 3 s limit), the 500 ms settle and its cancelling, options reported once, Snapshots taken when built, selection links, skills, sales, rerolls, merges, run end and abandon, no end without activity, a new run id, day and hour filled in, `reset` |
| `run/combat_tests.rs` | PvE and PvP combats, the `pvp_` prefix, the 1200 ms retry and `Unknown` after it, counters, stale simulations, end-of-run outcomes, the capture pushed back and cancelled, the ranked 4 s wait, rebinding |
| `tracker/tracker_tests.rs` | Start and stop, a recorded log saved to a real `history.db`, failed saves retried in order with a growing delay, connection notices and reset, rebinding in the history, live polling only while attached and only on change |
| `tests/run_replay.rs` | Whole recorded logs, as above |

Run them with `just tracker test` (all crates), or `cargo test -p bazaar-tracker run::`,
`cargo test -p bazaar-tracker tracker::` and `cargo test -p bazaar-tracker --test run_replay`.
