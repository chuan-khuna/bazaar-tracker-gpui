# BazaarDBTracker: log parsing, run state machine and event model

This is a deep dive into how `Player.log` is tailed and parsed, and how `GameTracker` mixes log lines with memory reads to produce run events. The target is `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll` (1.0.0-beta.7). See `overview.md` for identity and hashes. The target was never executed.

## Method

- There is no decompiler on this machine, so I wrote a small CIL disassembler with `System.Reflection.Metadata`, compiled in-process by PowerShell 7's `Add-Type`. It resolves every operand: `ldstr` literals, `ldc` constants, branch targets, member and type tokens, exception-clause catch types, field constants and property attributes. Nothing new was installed and the target never ran. The tool is at `scratchpad/logagent/DisL.cs`; its dumps are `log.txt`, `regex.txt`, `events.txt`, `gm.txt`, `watcher.txt` and `vm.txt` in the same folder.
- I read the IL of every method of `LogReader`, `LogParser`, the `LogLine` records, the 7 generated regex classes, `GameTracker` with all its closures, `GameManager`, and the relevant parts of `MainWindowViewModel` and `MemoryReader`.
- To check the Mac feasibility claims, I grepped the user's own local game log (`%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log` and `Player-prev.log`) for line shapes and counts. No account identifiers are copied into this doc.

How each claim is marked:
- **[V]**: verified from IL, meaning control flow, constants and literals were read.
- **[V-log]**: verified against the local `Player.log` sample.
- **[I]**: inferred.

No embedded directives or prompt-injection text were found in any string in scope.

---

## 1. Tailing `Player.log` (`BDC.Watcher.LogReader`)

| Aspect | Behaviour | |
|---|---|---|
| Default path | `Environment.GetFolderPath(UserProfile /*40*/) + "AppData\LocalLow\Tempo Storm\The Bazaar\Player.log"` (static `DefaultLogPath`). | [V] |
| Override | `MainWindowViewModel` passes `AppSettings.LogPath` to the `LogReader` ctor if it is not whitespace; otherwise it passes `null`, which selects `DefaultLogPath`. The setting is read once, at construction. | [V] |
| Waiting for the file | `Start` loops `File.Exists`, then `Task.Delay(1000, ct)`, until the file exists or the token is cancelled. | [V] |
| Start position | **The end of the file.** `_lastPosition = new FileInfo(path).Length`. Existing lines are never replayed at startup. | [V] |
| Open mode | `new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite)`, opened fresh on every read pass. | [V] |
| Change trigger | A `FileSystemWatcher(dir, fileName)` with `NotifyFilter = Size \| LastWrite` (24) whose `Changed` handler is `OnFileChanged` (async void), which calls `ReadNewLines`. | [V] |
| Poll | There is also an unconditional loop: `Task.Delay(500, ct)`, then `ReadNewLines()`, until cancelled. **The poll interval is 500 ms.** | [V] |
| Re-entrancy | A `SemaphoreSlim(1,1)` is taken with `WaitAsync(0)`. If a read is already running, the new call returns immediately; it is not queued. | [V] |
| Truncation and rotation | Before reading, the reader checks whether `fs.Length < _lastPosition` or `!TailMatches(fs)`. If either holds, it resets `_lastPosition = 0` and `_tailSignature = null`, then reads the new file from the start. | [V] |
| Tail signature | Holds the last `TailSignatureLength = 256` bytes before `_lastPosition`. `TailMatches` re-reads the 256 bytes ending at `_lastPosition` and compares them with `SequenceEqual`. An `IOException` there counts as a match. This catches a file that was replaced and has already grown past the old offset. | [V] |
| Chunking | Each pass reads at most `16 MiB` (`16777216`), from `_lastPosition` to EOF. It looks for the last `'\n'` in the buffer and decodes only up to it as UTF-8, so a partial trailing line waits for the next pass. `_lastPosition` advances by the consumed byte count, and the tail signature is updated incrementally. | [V] |
| Line split | `Split('\n')`, then `TrimEnd('\r')`. Lines that are `IsNullOrWhiteSpace` are skipped. Each line raises `LogEventReceived(new LogEventArgs{RawLine})` synchronously. A handler exception is logged as `"Log line handler failed: …"` and the loop continues. | [V] |
| Errors | An `IOException` from the read is swallowed silently. Any other exception is logged as `"Log tail error: …"`. If the file is missing during a pass, the pass returns and keeps its position. | [V] |
| Dispose | Sets `_disposed`, unhooks and disposes the watcher, and waits up to 3 s for the semaphore. | [V] |

**When the game restarts.** Unity moves the old log to `Player-prev.log` and starts a new `Player.log` [I, standard Unity behaviour; the `Player-prev.log` file is present locally]. The new file is shorter than `_lastPosition`, so the next 500 ms poll or watcher event resets to offset 0 and reads the new log from the top [V]. Separately, `GameManager.ConnectionLoop` polls the memory attach every 500 ms. When the connection drops from true to false, it raises `ConnectionChanged(false)` and calls `GameTracker.Reset()`, which clears the run id and all per-run state [V]. Attaching again does not reset anything [V].

**The log timestamp is discarded.** `LogParser` strips the `[hh:mm:ss.fff]` prefix and never parses it. Every event's `TimestampUtc` is `DateTime.UtcNow` at the moment the event factory runs on the emit thread (see §3.6) [V].

---

## 2. Regexes and `LogLine` types (`BDC.Watcher.LogParser.Parse`)

All patterns are `[GeneratedRegex]` with `RegexOptions.None` (`roptions = 0`) and the default timeout. They are recovered verbatim from the `ldstr` in each generated class's `.ctor` [V]. `capsize` (the group count plus 1) matches the capture groups below.

| # | Generated property | Pattern (verbatim) |
|---|---|---|
| 0 | `TimestampPrefix` | `^\[\d{2}:\d{2}:\d{2}\.\d{3}\]\s*` |
| 1 | `StateChangedRe` | `State changed from \[([A-Za-z0-9_]+)\] to \[([A-Za-z0-9_]+)\]` |
| 2 | `SkillSelectedRe` | `Selected skill ([\w-]+) to socket ([\w-]+)` |
| 3 | `GameSimSummaryRe` | `GameSim cards: id=\S*, dealt=(\d+), spawned=(\d+), disposed=(\d+)` |
| 4 | `CardPlacedRe` | `InstanceId:\s*([\w-]+)\s*-\s*TemplateId([0-9a-fA-F-]+)\s*-\s*Target:(\S+)\s*-\s*Section([A-Za-z]+)` |
| 5 | `CommandCompletedRe` | `Command completed: type=(\w+Command),.*\bresult=(\w+)` |
| 6 | `CombatPreparedRe` | `Combat prepared: frames=(\d+)` |

`Parse(raw)` first runs `line = TimestampPrefix.Replace(raw, "").Trim()`. It then tries the checks below in order, and the first match returns a value. Each regex is guarded by an ordinal `string.Contains` pre-check [V].

| Order | Pre-check substring | Regex | Produces (`LogLine` record) | Captured fields |
|---|---|---|---|---|
| 1 | `"] State changed from ["` | 1 | `StateChanged(From, To)` | `From`=g1, `To`=g2 (strings) |
| 2 | `"] Selected skill "` | 2 | `SkillSelected(SkillId, Socket)` | g1, g2 |
| 3 | `"GameSim cards:"` | 3 | `GameSimSummary(Dealt, Spawned, Disposed)` | `int.Parse` of g1..g3 |
| 4 | `"Card Purchased:"` | 4 | `CardPlaced(InstanceId, TemplateId, Target, Section)` | g1..g4 |
| 5 | `"Command completed: type="` | 5 | `CommandSent(Command, Succeeded)` | `Command`=g1, `Succeeded = (g2 == "success")` |
| 6 | `"Handled: type=NetMessageRunInitialized"` | none | `RunInitialized()` | none |
| 7 | `"[CombatSimHandler]"` and `"Combat prepared:"` | 6 | `CombatStarted(Frames: int?)` | g1 parsed; `null` if the regex fails |
| 8 | `"[CombatSimHandler]"` and `"Combat simulation completed"` | none | `CombatCompleted()` | none |
| | otherwise | | `null` | |

Real line shapes from the local log [V-log]. Ids are shortened, and every shape has at least one hit:

```
[21:21:23.356] [AppState] State changed from [StartRunAppState] to [ChoiceState]
[21:21:42.841] [AppState] Selected skill skl_tVCPvxR to socket SkillSocket_0
[21:21:33.589] [GameSimHandler] GameSim cards: id=dqT, dealt=3, spawned=0, disposed=0
[21:21:33.598] [BoardManager] Card Purchased: InstanceId: enc__lsiwsJ - TemplateIdd16c204b-… - Target:OpponentSocket_5 - SectionOpponent
[21:21:33.589] [NetworkManager] [HttpGameClient] Command completed: type=SelectEncounterCommand, requestId=2, status=200 OK, durationMs=57, result=success, endpoint=/commands
[21:21:20.558] [GameMessageHandler] Handled: type=NetMessageRunInitialized, id=Ydf, durationMs=0, result=success
[21:23:35.691] [CombatSimHandler] Combat prepared: frames=659, effectsMs=5
[21:23:48.042] [CombatSimHandler] Combat simulation completed: durationMs=12356
```

Observed `AppState` names include `StartRunAppState`, `ChoiceState`, `EncounterState`, `LootState`, `LevelUpState`, `PedestalState`, `CombatState`, `PVPCombatState` and `ReplayState` [V-log]. The tracker also compares against `EndRunVictoryState` and `EndRunDefeatState` [V]. Neither appears in the local sample, so the exact end-of-run state strings are [I].

Observed command types: `InitializeRunCommand`, `SelectEncounterCommand`, `SelectItemCommand`, `SelectSkillCommand`, `MoveItemCommand`, `SellCardCommand`, `RerollCommand`, `ExitCurrentStateCommand` and `CommitToPedestalCommand` [V-log]. `AbandonRunCommand` is matched by the tracker but does not appear in the sample.

The parsed data is not fully used:
- `GameSimSummary.Disposed` is never read [V].
- **`CombatCompleted` is parsed, but `GameTracker.Process` has no branch for it**, so it is ignored [V].
- `CardKind.FromInstanceId` maps an id prefix to a kind: `itm_`→`Item`, `enc_`→`Encounter`, `ste_`→`EncounterStep`, `skl_`→`Skill`, `com_`/`pvp_`→`Combat`, and anything else →`Unknown` [V].

---

## 3. `GameTracker`: the run state machine

### 3.1 Wiring and threading
- `GameManager` builds `GameTracker` with 7 memory `Func<>`s: `ReadBoardSafe`, `ReadCombatResultSafe`, `ReadRunInitSafe`, `ReadPvpOpponentSafe`, `ReadSelectedHeroSafe`, `ReadCombatFramesSafe` and `ReadPlayerRankSafe`. Each one takes `_memoryLock`, calls `TryConnect()` if the reader is not connected, and then reads. It also passes `isRunEnded`, which is `EventStore.IsRunEnded` from the VM [V].
- `LogReader` thread: `GameManager.OnLogLine` → `GameTracker.ProcessRawLine` → `LogParser.Parse` → `Process`. **`Process` runs synchronously on the log thread, including the memory reads that happen before it takes the lock** [V].
- Handlers do not build events directly. They append `Func<GameEvent>` factories to a list and push them, under `_lock`, into an unbounded single-reader `Channel` (`_emitQueue`). `EmitLoop`, started with `Task.Run` in the ctor, invokes each factory in order. Factories usually do their own memory reads, so **board snapshots are taken at emit time, not at log time** [V]. A factory returning `null` drops the event. Exceptions are logged as `"Event construction failed:"`.
- In `EmitLoop`: if an event has no `Day` or no `Hour`, and `_lastRunStats` has a Day or an Hour, the event is cloned (`with`) and the missing values are filled from the most recent successful board read. The event then goes to `EventDetected`, which `GameManager` forwards to `MainWindowViewModel.OnEventDetected` [V].
- Timers use `Task.Delay(ms).ContinueWith(…)` with a **generation counter** (`_settleGen`, `_combatCaptureGen`). Bumping the counter cancels any pending timer of that kind [V].

### 3.2 State fields
`_appState` (the last `StateChanged.To`), `_bzRunId`, `_runEnded`, `_runHasActivity`, `_runPlayMode`, `_runBoundaryPending`, `_pendingRunInit` and `_pendingRunInitValid` (set per line), `_endedBzRunIds`, `_lastPlayerItemIds`, `_lastRunStats (Day, Hour, Victories, Losses)`, `_dealtCurrent`, `_dealtPrevious` (capped at `DealtPreviousCap = 40`), `_reportedOptionIds`, `_optionsDirty`, `_selectedIds`, `_soldIds`, `_grantedIds`, `_combatIsPvp`, `_preCombatBoard`, `_combatPlaybackStarted`, `_expectedSimFrames`, `_combatStartEventId`, `_preCombatVictories`, `_preCombatLosses`, `_preCombatDay`, `_preCombatHour`, `_preRunRating`, and `_entityIndex` (instanceId → `EntityRef{TemplateId,Tier,Enchantment,Size}`, accumulated from every board read) [V].

Option states are a static `OptionStates = {ChoiceState, EncounterState, LootState, LevelUpState, PedestalState}`. Combat states are `CombatState` and `PVPCombatState` [V].

### 3.3 Constants

| Constant | Value | Use | |
|---|---|---|---|
| Options settle delay | **500 ms** | `ScheduleSettleLocked(500)` on entering an option state, or on `dealt>0` while in one. | [V] |
| Combat pre-board capture delay | **350 ms** | `ScheduleCombatCaptureLocked(350)` on entering combat, or on `spawned>0` before playback starts. | [V] |
| Deferred combat-end retry | **1200 ms** | Used when neither the sim nor HP can decide the outcome. | [V] |
| `RankSettleDelayMs` | **4000 ms** | `RunEnded` in Ranked mode waits for the rating to change. | [V] |
| Hero poll at run start | 6 × `Thread.Sleep(500)` | Up to 3 s waiting for `PlayerHero` or `SelectedHero`. | [V] |
| `DealtPreviousCap` | 40 | Size limit of the recently-retired option-id list. | [V] |
| Log poll / file-wait / connection poll | 500 / 1000 / 500 ms | `LogReader` and `GameManager`. | [V] |

### 3.4 Run identity: `local-` ids and rebinding
- **The real run id is memory-only.** `MemoryReader.ReadRunInit` reads `TheBazaar.AppState._cachedRunInitialized.<RunId>k__BackingField`, plus `PlayMode`, `BuildId` and `Environment`. If that fails, `ReadActiveRunId` reads `TheBazaar.Data.<Run>` and tries the fields `<Id>`, `<RunId>`, `Id`, `RunId` and `_id`, as a string or a Guid struct. That fallback yields only the `BzRunId`, with null PlayMode, BuildId and Environment [V].
- The local `Player.log` contains no run id, and no hero, gold, health, victories or rating values. Its only hero mention is `RunConfigurationCache: Changing EHero to <Hero>` [V-log].
- `StartRunLocked(info)` sets `_bzRunId = info?.BzRunId ?? $"local-{Guid.NewGuid():N}"`. It resets `_runEnded`, `_runHasActivity`, `_preRunRating`, `_lastRunStats` and `_lastPlayerItemIds`, clears the sold, granted and reported-option sets and the entity index, and queues a `RunStarted` event [V].
- `IsLocalRunId(id) = id != null && id.StartsWith("local-", StringComparison.Ordinal)` [V].
- **Pre-lock read in `Process`.**
  - If the line is `RunInitialized`, it sets `pending = ReadRunInit()` and `valid = true`.
  - Otherwise, if `_bzRunId == null || _runBoundaryPending || IsLocal(_bzRunId)`, it sets `valid = true` and then `if (ReadBoard() != null) pending = ReadRunInit()`.
  - So **while the run id is `local-`, every parsed log line triggers a full board read plus a run-init read** [V].
- **Inside the lock:**
  - If `_runBoundaryPending && pending != null`, it clears the flag and calls `ResolveRunInitLocked(pending)`.
  - Otherwise, if `pending != null && IsLocal(_bzRunId) && !_runEnded`, it calls `ResolveRunInitLocked(pending)`.
  - The line is then dispatched, and `pending` is cleared [V].
- **`ResolveRunInitLocked(info)`:**
  - If `info.BzRunId == _bzRunId`, it does nothing.
  - If the current id is `local-` and the run has not ended, it calls **`RebindRunLocked`**. This sets `_bzRunId = newId`, keeps `PlayMode` (`info.PlayMode ?? old`), and queues a **`RunRebound{BzRunId=new, OldBzRunId=old}`** event.
  - Otherwise it calls `EndRunLocked(Unknown)` on the old run, then `StartRunLocked(info)` [V].
- **`HandleRunInitialized`** (the log line): with no pending info it sets `_runBoundaryPending = true`, which retries on later lines; with pending info it calls `ResolveRunInitLocked` [V].
- **`EnsureRunLocked`** is called before any activity event. If `_bzRunId == null` it starts a run, using `pending` when it is valid and otherwise trying `ReadBoard`/`ReadRunInit`. If that id is already finished, either in `_endedBzRunIds` or `EventStore.IsRunEnded`, it passes `null`, which yields a fresh `local-` id [V].
  - **It does nothing if `_bzRunId` is non-null, even after the run has ended.** A new run is started only by `ResolveRunInitLocked` (which needs memory) or by `Reset()` [V].
- **The VM side** (`OnEventDetected`):
  - `RunReboundEvent` is **not inserted**. It calls `EventStore.RebindRun(old, new)`, which moves the rows and rewrites `$.BzRunId`, and then nudges the uploader.
  - `RunStartedEvent` for a run that already exists in the DB calls `ClearRunEnd` and is **not inserted**. Every other event is serialized and inserted [V].
  - The uploader never sends `local-` runs (see `overview.md` §5.4).

### 3.5 Transitions (log-line driven)

```
StateChanged(from,to):            [HandleStateChanged]
  if IsCombat(_appState) && !IsCombat(to):
       QueueCombatEnd(outcome = to=="EndRunVictoryState"?Win : to=="EndRunDefeatState"?Loss : null)
  if to=="EndRunVictoryState" -> EndRunLocked(Win);  if to=="EndRunDefeatState" -> EndRunLocked(Loss)
  _appState = to
  if to == previous state:  if IsOption(to) ScheduleSettle(500); return
  RetireCurrentOptions()                     // _dealtCurrent -> _dealtPrevious
  if IsOption(to): ScheduleSettle(500); return
  _settleGen++                                // cancel pending settle
  if IsCombat(to): _combatIsPvp |= (to=="PVPCombatState"); _combatStartEventId=null;
                   _preCombatBoard=null; _combatPlaybackStarted=false; ScheduleCombatCapture(350)

GameSimSummary(dealt,spawned,_):
  if dealt>0:  _optionsDirty=true; if IsOption(_appState) ScheduleSettle(500)
  if spawned>0 && IsCombat(_appState) && !_combatPlaybackStarted: ScheduleCombatCapture(350)  // debounce

RunInitialized:                   HandleRunInitialized (see 3.4)

CombatStarted(frames):            ("Combat prepared: frames=N")
  if frames: _expectedSimFrames = frames
  unless _appState is EndRun* or (_runEnded && !IsCombat(_appState)):  HandleCombatStart
     -> EnsureRun; _settleGen++; _combatCaptureGen++ (cancel capture); _combatPlaybackStarted=true;
        new Guid -> _combatStartEventId; snapshot _preCombatVictories/Losses/Day/Hour from _preCombatBoard
        queue CombatStart (board = _preCombatBoard ?? ReadBoard at emit; PvP opponent read if isPvp)

CardPlaced(id,tpl,target,section): ("Card Purchased:")       [HandlePlaced]
  inDealt = _dealtCurrent∋id; inPrev = _dealtPrevious∋id
  if !_selectedIds∋id && (inDealt || inPrev || !_lastPlayerItemIds∋id):
       _selectedIds+=id; if id starts "pvp_" _combatIsPvp=true
       if inDealt: FlushOptions(presented=_dealtCurrent copy, proven=id)
       EnsureRun; QueueSelected(kind=CardKind(id), id, tpl, section, target, mergedInto=null)
  else if IsOption(_appState) && CardKind(id)=="Item":       // bought a duplicate -> merge/upgrade
       candidates = _dealtCurrent where !selected && kind==Item; if none return
       queue factory: read board; pick candidates no longer in EntityIndex; if exactly 1 ->
             Selected{InstanceId=that, MergedIntoInstanceId=id}

CommandSent(cmd, ok) where ok:
  "AbandonRunCommand" (exact)            -> EndRunLocked(Loss)
  cmd contains "ReRoll" (OrdinalIgnoreCase)  -> EnsureRun; queue Rerolled
  cmd contains "SellCard" (OrdinalIgnoreCase)-> HandleSell: EnsureRun; snapshot before=_lastPlayerItemIds;
        factory: read board; gone = before − after(board items) not already in _soldIds;
        emit Sold for each (first returned, extras pushed straight to the channel)

SkillSelected(skill, socket):
  if _selectedIds.Add(skill): FlushOptions(presented=_dealtCurrent copy, proven=skill);
       EnsureRun; QueueSelected(kind="Skill", id=skill, tpl=null, section=null, target=socket)

CombatCompleted:   ignored
```
All of the above is [V].

**Timers.**
- `OnSettleFired(gen)`: if `gen == _settleGen` and `IsOption(_appState)`, it calls `FlushOptionsLocked(null, null)` in settle mode.
  - Settle mode emits only when `_optionsDirty` is set; it then clears the flag.
  - The factory reads the board. That read calls `RefreshOptionsFromBoardLocked`, which sets `_dealtCurrent = board.SelectionSet` and moves dropped ids to `_dealtPrevious`.
  - The factory then emits `OptionsPresented{Context=_appState, Options=ids not yet in _reportedOptionIds}`. If the board read fails, it sets `_optionsDirty = true` again and emits nothing [V].
- `OnCombatCaptureFired(gen)`: if `gen == _combatCaptureGen`, the state is still combat and playback has not started, it reads the board and stores it as `_preCombatBoard`. The second gen check happens after the read [V].

**`EndRunLocked(outcome)`.**
- It is a no-op unless `_bzRunId != null && !_runEnded && _runHasActivity`.
- It sets `_runEnded = true` and adds the id to `_endedBzRunIds`. It captures `_lastRunStats` (Day, Hour, V, L) and `_preRunRating`, and sets `ranked = PlayMode.Equals("Ranked", OrdinalIgnoreCase)`.
- The factory reads the board and the rank. If the run is ranked and the rating equals the pre-run rating, it waits 4000 ms, re-reads the rank, and emits `RunEnded` from the continuation. Otherwise it emits immediately [V].
- Callers: `EndRunVictoryState` (Win), `EndRunDefeatState` (Loss), `AbandonRunCommand` (Loss), and the run-id change in `ResolveRunInitLocked` (Unknown) [V].

**`QueueCombatEnd(knownOutcome?)`** fires when the state leaves combat, usually to `ReplayState` [V-log].
1. It captures `wasPvp` and resets it, and captures and resets `_expectedSimFrames` and `_combatStartEventId` [V].
2. **If the outcome is known** (from an EndRun state), it queues `CombatEnd(known)` and then `CombatFrames` [V].
3. **Otherwise** a factory works through these steps [V]:
   1. It reads the combat sim (`BoardManager._lastCombatSim` or the `AppState` cache). If `FrameCount == expected`, or no expected count is known, it maps the result through `OutcomeFromSim`: (winner 0, loser 1) → **Win**, (winner 1, loser 0) → **Loss**.
   2. If that fails, it reads the board. Opponent HP ≤ 0, or player HP ≤ 0, lets `BuildCombatEnd` derive the outcome.
   3. If neither works, it sets `outcomeDeferred`, waits 1200 ms, and then pushes `CombatEnd(null outcome)` and `CombatFrames`. The `CombatFrames` factory queued in step 2 returns null when the end was deferred.
4. `BuildCombatEnd` calls `DeriveCombatOutcome` in this order: sim winner/loser; victories increased over the pre-combat count → Win; losses increased → Loss; `RunState == EndRunDefeat(3)` → Loss; `RunState == EndRunVictory(4)` → Win; opponent HP ≤ 0 → Win; player HP ≤ 0 → Loss; otherwise Unknown [V].
5. A sim whose frame count does not match the expected count is discarded, with `SimDiag = "stale (frame mismatch)"` [V].
6. For PvP, `VictoriesAfter` and `LossesAfter` are bumped to `pre+1` if the board has not caught up yet [V].
7. `Day` and `Hour` come from the pre-combat values, falling back to the board [V].

**`BuildCombatFrames`** reads `ReadCombatFrames`, which serializes the frames with `ManagedJson` under a node budget. It skips the event, logging `"Combat frames stale (got X, expected Y); skipping"`, when the count does not match the expected count, and it returns null when there are no frames. A truncated capture is still emitted, after a warning is logged [V].

### 3.6 How memory reads and log lines combine
- **Log lines are only triggers and keys.** They provide the state names, the purchased instance and template ids, skill ids, command success, the expected frame count and the run-init moment.
- **Memory provides everything else:**
  - the real run id, play mode, build and environment
  - the hero and the rank or rating
  - the option list (`SelectionSet`)
  - the board, stash and skills with attributes, tags and enchantments
  - tier and enchantment for ids (`_entityIndex`)
  - which item was sold
  - granted items
  - HP, gold, prestige, level, income, day, hour, victories and losses
  - the combat winner and loser, the frames, and the PvP opponent name, hero, rating and level.
- Reads are lazy (inside factories), happen once per run boundary in `Process`, and are debounced by the 350 ms and 500 ms timers. All of this is [V].

---

## 4. Event payloads (JSON)

**Serializer.** `MainWindowViewModel.WireJson` is `new JsonSerializerOptions { Converters = { new JsonStringEnumConverter() }, DefaultIgnoreCondition = WhenWritingNull }`. Each event is serialized with `JsonSerializer.Serialize(ev, ev.GetType(), WireJson)`, and the result is stored as `events.payloadJson` [V].

Serialization rules:
- **Names:** PascalCase, because no naming policy is set. Enums are serialized as names [V].
- **Nulls:** `null` reference and `Nullable<T>` values are omitted. `false` and `0` are kept [V].
- **No custom attributes:** none of the event types carries a `[JsonPropertyName]`, `[JsonIgnore]` or polymorphism attribute [V].
- **Compiler property:** `EqualityContract` is protected, so it is not serialized [I, standard STJ behaviour].
- **Property order:** I expect the base `GameEvent` properties to come first [I, STJ on .NET 7+ walks the type hierarchy from the base]. Readers should not depend on the order.
- **Type formats:** `Guid` uses the `D` format, and `DateTime` is ISO 8601 with `Z` [I, STJ defaults].

**Board snapshots** (`Board` on `OptionsPresented`, `Selected` and `Sold`, from `SnapshotPayload`) are an anonymous object with **camelCase** keys [V]:
`playerBoard, playerStash, playerSkills, opponentBoard, opponentSkills, isStorageOpen, runState (ERunState name), currentEncounterId (Guid string), playerHealth, playerHealthMax, opponentHealth, opponentHealthMax, gold, prestige, level, income, day, hour, victories, losses, playerHero, opponentHero`.

Card lists are `List<Dictionary<string,object>>`. `MemoryReader.ReadBoardState` writes these keys: `typeName, templateId, instanceId, type, tier, size, section, socketId, attributes, tags, enchantment, boardPosition, slotWidth`, and `visualPosition` for opponents [V keys; value shapes belong to the memory deep dive]. `CombatStart.PlayerBoard` and the other board fields are the same raw lists.

### 4.1 Common base: `GameEvent` (a record)

| Field | Type | Notes |
|---|---|---|
| `EventId` | Guid | `Guid.NewGuid()` in the ctor; `CombatStart` sets a pre-generated one [V] |
| `Type` | `GameEventType` string | `RunStarted`=0 … `RunRebound`=9 |
| `TimestampUtc` | DateTime | emit-time `UtcNow` |
| `BzRunId` | string | real game RunId or `local-<32 hex>` |
| `Day` | int? | filled from `_lastRunStats` by `EmitLoop` if missing |
| `Hour` | int? | ditto |

### 4.2 Per-type fields (in declaration order) [V]

| Type | Fields (type) | Source |
|---|---|---|
| `RunStarted` | `PlayMode` (string), `BuildId` (string), `Environment` (string), `PlayerHero` (string), `PlayerRating` (int?), `PlayerRank` (string), `PlayerRankDivision` (int?) | all memory |
| `OptionsPresented` | `Context` (string = `_appState`), `Options` (`PresentedOption[]`), `Board` (object) | ids memory (`SelectionSet`); trigger log |
| ↳ `PresentedOption` | `InstanceId`, `Kind`, `Size` (default `"Unknown"`), `TemplateId`, `Tier`, `Enchantment` (all string) | memory `_entityIndex`; Kind from id prefix |
| `Selected` | `CardKind` (string), `InstanceId` (string), `TemplateId` (string), `Tier` (string), `Enchantment` (string), `Section` (string), `Target` (string), `MergedIntoInstanceId` (string), `Board` (object), `GrantedInstanceIds` (string[]) | ids, template, section and target from the log; the rest from memory |
| `Sold` | `InstanceId` (string), `TemplateId`, `Tier`, `Enchantment` (string), `Board` (object) | memory diff (the log line is not used) |
| `Rerolled` | (none) | log |
| `CombatStart` | `PlayerCards`, `OpponentCards` (`SpawnedCardRef[]`), `PlayerBoard`, `OpponentBoard`, `PlayerStash`, `PlayerSkills`, `OpponentSkills` (object), `PlayerHealthMax`, `OpponentHealthMax` (int?), `IsPvp` (bool), `PlayerHero`, `OpponentName`, `OpponentHero` (string), `OpponentRating`, `OpponentLevel` (int?) | memory; trigger and IsPvp from the log |
| ↳ `SpawnedCardRef` | `InstanceId`, `Owner` (`"Player"`/`"Opponent"`), `Section`, `Socket` (null for skills), `Size` (default `"Unknown"`) | memory |
| `CombatEnd` | `Outcome` (`Unknown`/`Win`/`Loss`), `CombatStartEventId` (Guid?), `PlayerHpAfter`, `OpponentHpAfter`, `VictoriesAfter`, `LossesAfter`, `SimWinner`, `SimLoser`, `SimFrames`, `ExpectedSimFrames` (int?), `SimDiag` (string) | memory, except `ExpectedSimFrames` (log) |
| `CombatFrames` | `CombatStartEventId` (Guid?), `Frames` (JsonNode), `FrameCount` (int), `Winner`, `Loser` (int?), `Truncated` (bool), `Nodes` (int), `CaptureMs` (long), `Diag` (string) | memory |
| `RunEnded` | `Outcome` (CombatOutcome), `Victories`, `Losses`, `Gold`, `HealthMax`, `Prestige`, `Level`, `Income`, `PlayerRatingAfter` (int?), `PlayerRankAfter` (string), `PlayerRankDivisionAfter` (int?) | outcome from the log (state or command); the rest from memory |
| `RunRebound` | `OldBzRunId` (string) | memory; not persisted |

### 4.3 Examples
These are illustrative. Field names and types are [V], and values are made up. Keys whose values are null do not appear.

```json
{"EventId":"7b0c…","Type":"RunStarted","TimestampUtc":"2026-10-30T21:21:24.112Z","BzRunId":"3f2a9c1e-…","Day":1,"Hour":0,
 "PlayMode":"Ranked","BuildId":"…","Environment":"prod","PlayerHero":"Jules","PlayerRating":1234,"PlayerRank":"Gold","PlayerRankDivision":2}

{"EventId":"…","Type":"OptionsPresented","TimestampUtc":"…","BzRunId":"local-0d4c2b9e8f7a4c1b9a3e5d6f7a8b9c0d","Day":2,"Hour":3,
 "Context":"ChoiceState",
 "Options":[{"InstanceId":"itm_EuUtMwz","Kind":"Item","Size":"Small","TemplateId":"d16c204b-…","Tier":"Bronze"},
            {"InstanceId":"enc_T4OLAka","Kind":"Encounter","Size":"Unknown"}],
 "Board":{"playerBoard":[{"instanceId":"itm_…","templateId":"…","type":"Item","tier":"Silver","size":"Medium","section":"Player","socketId":"Socket_2"}],
          "playerStash":[],"playerSkills":[],"opponentBoard":[],"opponentSkills":[],"isStorageOpen":false,"runState":"Choice",
          "playerHealth":300,"playerHealthMax":300,"gold":12,"day":2,"hour":3,"victories":1,"losses":0,"playerHero":"Jules"}}

{"EventId":"…","Type":"Selected","TimestampUtc":"…","BzRunId":"…","Day":2,"Hour":3,
 "CardKind":"Item","InstanceId":"itm_EuUtMwz","TemplateId":"d16c204b-…","Tier":"Bronze","Section":"Player","Target":"PlayerSocket_3",
 "Board":{ … },"GrantedInstanceIds":["itm_x1…"]}

{"EventId":"…","Type":"Sold","TimestampUtc":"…","BzRunId":"…","Day":3,"Hour":1,"InstanceId":"itm_mu-K0YD","TemplateId":"…","Tier":"Bronze","Board":{ … }}

{"EventId":"…","Type":"Rerolled","TimestampUtc":"…","BzRunId":"…","Day":3,"Hour":2}

{"EventId":"c9e1…","Type":"CombatStart","TimestampUtc":"…","BzRunId":"…","Day":4,"Hour":5,
 "PlayerCards":[{"InstanceId":"itm_…","Owner":"Player","Section":"Player","Socket":"Socket_0","Size":"Small"},
                {"InstanceId":"skl_…","Owner":"Player","Size":"Unknown"}],
 "OpponentCards":[ … ],"PlayerBoard":[ … ],"OpponentBoard":[ … ],"PlayerStash":[ … ],"PlayerSkills":[ … ],"OpponentSkills":[ … ],
 "PlayerHealthMax":450,"OpponentHealthMax":420,"IsPvp":true,"PlayerHero":"Jules","OpponentName":"…","OpponentHero":"Vanessa","OpponentRating":1180,"OpponentLevel":9}

{"EventId":"…","Type":"CombatEnd","TimestampUtc":"…","BzRunId":"…","Day":4,"Hour":5,
 "Outcome":"Win","CombatStartEventId":"c9e1…","PlayerHpAfter":210,"OpponentHpAfter":0,"VictoriesAfter":3,"LossesAfter":1,
 "SimWinner":0,"SimLoser":1,"SimFrames":659,"ExpectedSimFrames":659}

{"EventId":"…","Type":"CombatFrames","TimestampUtc":"…","BzRunId":"…","Day":4,"Hour":5,
 "CombatStartEventId":"c9e1…","Frames":[ … ManagedJson tree … ],"FrameCount":659,"Winner":0,"Loser":1,"Truncated":false,"Nodes":48211,"CaptureMs":183}

{"EventId":"…","Type":"RunEnded","TimestampUtc":"…","BzRunId":"…","Day":9,"Hour":4,
 "Outcome":"Win","Victories":10,"Losses":3,"Gold":17,"HealthMax":900,"Prestige":20,"Level":14,"Income":12,
 "PlayerRatingAfter":1262,"PlayerRankAfter":"Gold","PlayerRankDivisionAfter":1}

{"EventId":"…","Type":"RunRebound","TimestampUtc":"…","BzRunId":"3f2a9c1e-…","OldBzRunId":"local-0d4c2b9e8f7a4c1b9a3e5d6f7a8b9c0d"}
```
The `Size`, `Tier`, `runState` and HP value formats are [I]. They come from the memory reader and `GameEnums.Name`.

---

## 5. Mac feasibility: log-only vs memory-only

The macOS Unity player writes `~/Library/Logs/Tempo Storm/The Bazaar/Player.log` [I, standard Unity location]. There is no UnitySpy or Windows memory facade on Mac, so the question is what survives with the log alone.

| Datum | In `Player.log`? | Used by the tracker from | Notes |
|---|---|---|---|
| App/run state transitions (Choice, Encounter, Loot, LevelUp, Pedestal, Combat, PVPCombat, Replay) | **Yes** [V-log] | log | Drives settle and capture timers and the combat end. |
| Run start moment | **Yes**: `Handled: type=NetMessageRunInitialized`, plus `InitializeRunCommand` and `[null]→StartRunAppState` [V-log] | log | |
| Real game RunId (`BzRunId`) | **No** [V-log, bounded to the sample] | memory | Without it, runs stay `local-…`, and the uploader refuses `local-` runs. |
| PlayMode / BuildId / Environment | No [V-log] | memory | Ranked detection and the rank-settle wait depend on PlayMode. |
| Hero | **Partly**: `RunConfigurationCache: Changing EHero to <Hero>` [V-log] | memory (`Hero`, `RunConfig`) | Not parsed by the tracker; a port could parse it. |
| Rank / rating / division (before and after) | No | memory | |
| Purchased card instance id, template id, target socket, section | **Yes**: `Card Purchased:` [V-log] | log | This covers items and encounters. |
| Skill picked and its socket | **Yes**: `Selected skill … to socket …` [V-log] | log | |
| Reroll happened | **Yes**: `RerollCommand … result=success` [V-log] | log | |
| Sell happened | **Yes**: `SellCardCommand` | log | |
| Which card was sold, and for how much | **Yes**: `[BoardManager] Sold Card itm_X for N gold.` [V-log] | **memory diff** (the tracker ignores the log line) | A port can be log-only here. |
| Abandon | Yes, the command name is matched (`AbandonRunCommand`) [I; not in the sample] | log | |
| Run end and outcome | `EndRunVictoryState`/`EndRunDefeatState` state names [I; not in the sample] | log, refined by memory | |
| Options offered (ids) | **Count only** (`dealt=N`) | memory `SelectionSet` | Log-only can list only the options later bought, not the unbought ones. |
| Card tier / enchantment / size | No (template id only) | memory `_entityIndex` | Partly derivable from template id plus `cards.json`, but tier and enchantment are per-instance. |
| Board / stash / skills layout, attributes, tags | No | memory | `MoveItemCommand` success is logged, but not its source and destination. |
| Gold, HP, max HP, prestige, level, income, day, hour, victories, losses | No [V-log] | memory | `Day`/`Hour` on every event come from memory. |
| Granted items (rewards) | No | memory diff | |
| Duplicate-merge (upgrade) resolution | No | memory | |
| Combat start and PvP flag | **Yes**: `Combat prepared: frames=N`, `PVPCombatState`, `pvp_` ids [V-log] | log | |
| Combat expected frame count | **Yes** | log | |
| Combat winner / loser | No | memory sim, then memory HP or W-L | A log-only port would get `Outcome` mostly `Unknown`, except on a run-ending fight. |
| Combat frames (replay) | No | memory | |
| PvP opponent name, hero, rating, level | No | memory | |
| Opponent board | No | memory | |

**Verdict.** The log can produce a coarse run skeleton: run boundaries, state flow, purchases with template ids, skill picks, rerolls, sells (by id), combat start and end markers, PvP versus PvE, and the end of run. Everything that makes the data valuable needs process memory: the real RunId (and therefore server sync), options offered, boards, the economy and HP, combat outcomes and frames, the opponent, and rank.

**A log-only port must change the state machine.** On a log-only path, the tracker never leaves a run once it is set. `EnsureRunLocked` does nothing while `_bzRunId` is non-null, and a new run starts only through `ResolveRunInitLocked` (which needs memory) or `Reset()` (which fires on memory disconnect). A log-only port must start a new `local-` run on each `RunInitialized` line [V].

---

## 6. Unknowns
- The exact end-of-run `AppState` strings, and whether `AbandonRunCommand` appears in logs. Neither is in the local sample.
- The STJ property order for derived records, the `Guid`/`DateTime` text formats, and the value shapes inside board dictionaries (`tier`, `size`, `attributes`, `tags`, `boardPosition`). The shapes belong to the memory deep dive.
- Whether the macOS player logs identical lines. The game's logger categories suggest it does [I], but this is not verified.
- Whether Unity truncates `Player.log` in place, as opposed to rotating it, under some launch paths. Both cases are handled by the length and tail-signature check.
