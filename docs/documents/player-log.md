# Player Log

How the tracker reads The Bazaar's `Player.log`: where the file is, how it is followed while the game writes it, and which lines become a `LogLine`. The code is in `crates/bazaar-tracker/src/log/` (`path.rs`, `follow.rs`, `parse.rs`). Log lines are triggers only: the data for an Event comes from game memory ([memory-reading.md](memory-reading.md)), and the terms Run and Event are defined in [GLOSSARY.md](../../GLOSSARY.md).

## How it fits

```
Player.log ─► LogFollower::poll (every 500 ms, tracker-log thread)
                 │ complete lines, no line endings, no blank lines
                 ▼
             Input::Line ─► tracker-machine thread ─► RunMachine::on_raw_line ─► parse_line ─► LogLine
```

`Tracker::start_live` runs the follower on the `tracker-log` thread and sends each line to the machine thread one by one. A read error becomes `Notice::LogError` and the next pass tries again. What the machine does with each `LogLine` is in [run-state-machine.md](run-state-machine.md).

## Where the file is

| Function | What it gives |
|---|---|
| `game_folder_in(profile)` | `<profile>\AppData\LocalLow\Tempo Storm\The Bazaar` |
| `default_path_in(profile)` | That folder plus `Player.log` |
| `user_profile()` | `USERPROFILE`, if it is set and not empty |
| `resolve(setting, profile)` | The path to follow (below) |

`resolve` picks, in order:

1. The `log_path` setting, trimmed, if it holds anything but whitespace (`"  D:\logs\Player.log  "` → `D:\logs\Player.log`).
2. Otherwise the default under the user's profile: `C:\Users\player\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log`.
3. With no profile either, the bare name `Player.log`. It will not be found, so the follower just waits.

`log_path` lives in `settings.toml` and is empty by default ("the usual place"). The app reads it once, when `main.rs` starts the `Service`, so a change takes effect on the next launch. `just tracker track -- --log <path>` passes the same kind of value to the headless tracker. The game folder from `game_folder_in` is also where the card catalog and art look for the game's data ([card-catalog-and-art.md](card-catalog-and-art.md)).

## Following

`LogFollower::new(path)` follows one file. Each `poll()` returns the complete lines written since the last call.

| Constant | Value | What it is |
|---|---|---|
| `POLL_INTERVAL` | 500 ms | How often `run` polls |
| `MAX_PASS_BYTES` | 16 MiB | The most bytes read in one pass; the rest comes next pass |
| `TAIL_SIGNATURE_LEN` | 256 bytes | How much of the file before our position is remembered |

### Rules

- **Start at the end.** The first time the file is found, the follower only notes where its end is and remembers the last 256 bytes. An old session is never replayed.
- **Missing file.** `NotFound` gives no lines and no error; the follower waits. A file that appears after it was seen missing is new (the game started after the tracker), so it is read from its first line.
- **Truncated file.** A file shorter than our position is a new file: read from the top.
- **Replaced file.** A file that is already longer than our position but whose 256 bytes before it are not what we read is also a new file: read from the top. A file deleted and recreated is followed again the same way.
- **Whole lines only.** Everything up to the last `\n` is taken; a partial last line waits for the next pass. Bytes are decoded as UTF-8 (lossy) only after that, so a multi-byte character split across two writes arrives whole.
- **Clean lines.** `\r` is trimmed from each line and blank or whitespace-only lines are dropped.
- **Errors.** Any other open or read error is returned. If the tail cannot be re-read to check it, it counts as matching, so a passing error does not lose our place.

`run(stop, on_lines, on_error)` polls every 500 ms until `stop` is set; `run_every` takes another interval. It sleeps in 25 ms steps so a stop is noticed quickly, and calls `on_lines` only for a pass that found lines.

## Parsing

`parse_line(raw)` returns `Some(LogLine)` for the lines the tracker uses and `None` for everything else (most of the file: Unity warnings, `Animation clip is null.`, network chatter).

### Timestamps dropped

`strip_timestamp` removes a leading `[hh:mm:ss.fff]` (exactly two, two, two and three digits) and trims the rest. `[1:2:3.4] x` is not a timestamp and stays. The game's clock is never used: Events are stamped when they are made.

### The substring guard

Each pattern sits behind a plain `contains` check, so most lines never reach a regex. The checks run in a fixed order and the first match wins. The regexes are compiled once (`LazyLock`).

| Order | Guard (substring) | Then |
|---|---|---|
| 1 | `] State changed from [` | `STATE_CHANGED` |
| 2 | `] Selected skill ` | `SKILL_SELECTED` |
| 3 | `GameSim cards:` | `GAME_SIM_SUMMARY`, and all three counts must fit a `u32` |
| 4 | `Card Purchased:` | `CARD_PLACED` |
| 5 | `Command completed: type=` | `COMMAND_COMPLETED` |
| 6 | `Handled: type=NetMessageRunInitialized` | nothing more |
| 7 | `[CombatSimHandler]` and `Combat prepared:` | `COMBAT_PREPARED` for the count, optional |
| 8 | `[CombatSimHandler]` and `Combat simulation completed` | nothing more |

The first two guards include the closing bracket of the category before the words, so `Selected skill skl_x to socket S_0` with no `[AppState]` before it is not a skill selection.

### `LogLine` variants

Each example below is a raw line from the tests or from `tests/fixtures/logs/`.

**`StateChanged { from, to }`**: the game's app state moved. State names are `[A-Za-z0-9_]+`; the first state of a session is `null`.

```
[21:21:23.350] [AppState] State changed from [null] to [StartRunAppState]
[21:25:11.016] [AppState] State changed from [ChoiceState] to [PVPCombatState]
```

Not matched: `State changed from A to B` (no brackets), `[A-B]` (a dash), `Input System module state changed to: Initialized.`

**`SkillSelected { skill_id, socket }`**: ids and sockets are `[\w-]+`.

```
[21:21:42.841] [AppState] Selected skill skl_tVCPvxR to socket SkillSocket_0
```

**`GameSimSummary { dealt, spawned, disposed }`**: the cards a game simulation dealt, spawned and disposed of. The `id` may be empty.

```
[21:21:33.589] [GameSimHandler] GameSim cards: id=dqT, dealt=3, spawned=0, disposed=0
[21:25:10.911] [GameSimHandler] GameSim cards: id=XAG, dealt=0, spawned=11, disposed=0
```

Not matched: a missing `disposed`, a non-number, or a count too large for a `u32` (`dealt=99999999999`).

**`CardPlaced { instance_id, template_id, target, section }`**: a card went onto a socket. The template id is hex and dashes, written straight after `TemplateId` with no space; the section is letters (`Player`, `Storage`, `Opponent`).

```
[21:22:59.034] [BoardManager] Card Purchased: InstanceId: itm_I_dRbS- - TemplateIdda766a09-0352-4966-829f-20bda8da48d7 - Target:PlayerSocket_3 - SectionPlayer
[21:23:05.872] [BoardManager] Card Purchased: InstanceId: itm_zl-t1UD - TemplateId825700a8-f21b-4867-ad08-d752f13a51ab - Target:PlayerStorageSocket_0 - SectionStorage
[21:25:11.027] [BoardManager] Card Purchased: InstanceId: pvp_9i8982w - TemplateIdb79845a7-f806-4e7a-9011-d98ebd22a323 - Target:OpponentSocket_5 - SectionOpponent
```

Not matched: a non-hex template id, a missing section, `[BoardManager] Sold Card itm_EuUtMwz for 1 gold.`

**`CommandCompleted { command, succeeded }`**: a game command came back. The type must end in `Command`; `succeeded` is `result=success`, anything else is `false`.

```
[21:21:33.589] [NetworkManager] [HttpGameClient] Command completed: type=SelectEncounterCommand, requestId=2, status=200 OK, durationMs=57, result=success, endpoint=/commands
[HttpGameClient] Command completed: type=SellCardCommand, requestId=9, status=400, result=failure, endpoint=/commands
```

Not matched: `type=Ping`, no `result=`, `Command started:`.

**`RunInitialized`**: a Run begins. Recognised by its substring alone.

```
[21:21:20.558] [GameMessageHandler] Handled: type=NetMessageRunInitialized, id=Ydf, durationMs=0, result=success
```

The `[NetMessageProcessor] Received: type=NetMessageRunInitialized` line just before it is not the trigger.

**`CombatPrepared { frames }`**: a combat simulation is ready. `frames` is `None` if the count is missing or not a number.

```
[21:23:35.691] [CombatSimHandler] Combat prepared: frames=659, effectsMs=5
[CombatSimHandler] Combat prepared: frames=many            → frames: None
```

**`CombatCompleted`**: the combat replay finished. The tracker has no use for it yet.

```
[21:23:48.042] [CombatSimHandler] Combat simulation completed: durationMs=12356
```

Combat lines in another category (`[Other] Combat prepared: frames=659`) and `Combat simulation started` are not matched.

## Fixtures

| File | What it holds |
|---|---|
| `tests/fixtures/logs/real-run-start.log` | 143 lines from a real session: `RunInitialized`, the first `null` → `StartRunAppState` change, encounters, a skill, items placed on the board and in storage, `SelectItemCommand`s, a combat (`frames=659`), `ReplayState` and `LootState` |
| `tests/fixtures/logs/real-pvp-combat.log` | 51 lines of a PvP fight: `ChoiceState` → `PVPCombatState`, `pvp_` cards placed on the opponent's sockets, `frames=415` |

## Tests

- `log/path_tests.rs`: the default path under `LocalLow`, the setting overriding it (trimmed), blank meaning default, no profile.
- `log/follow_tests.rs`: starting at the end, order across passes, partial lines held back, line endings and blank lines, a missing file read from its first line, truncation, replacement detected by the tail, delete and recreate, the 16 MiB cap, a split multi-byte character, and `run` delivering lines until stopped. They write real files in a temp folder.
- `log/parse_tests.rs`: timestamp stripping, each variant and its near misses, ordinary lines, and both fixtures parsed whole.
- `tests/run_replay.rs`: both fixtures played through the whole `RunMachine` with `FakeMemory` and a clock that follows the log's timestamps.

Run them with `just tracker test`.
