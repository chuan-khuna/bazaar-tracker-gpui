# BazaarDBTracker 1.0.0-beta.7: Overview

This is a static survey of `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll`. The target was never executed.

- Assembly: `BazaarDBTracker` 1.0.0.0, product version `1.0.0-beta.7`, Velopack channel `beta`.
- SHA-256: `ab1734c404fa8708f51384813f6190264f4fad002737833135cdd3cc476f362d`
- MVID: `f3dd3a4a-456c-4c32-a84c-2000d9b684ca`
- Build: .NET 10 (`.NETCoreApp,Version=v10.0`, runtime 10.0.9), x64, IL plus ReadyToRun. The IL bodies are intact and the names are not obfuscated.
- **The root namespace is `BazaarDBCompanion`, not `BazaarDBTracker`.** Only the assembly and exe are named "Tracker". Throughout this doc, `BazaarDBCompanion.` is shortened to `BDC.`.

REA evidence IDs:
- managed artifact `ev_af88337926cc5272506e865abc9abec26090431987b9efa899da97f11a4d7bd8`
- managed members `ev_aaf4c467062ffc61c14bf03dcce30d306b72f4a90facc159ad09f311852174ce`
- native boundaries `ev_bd224d55f7599a5cd88773cfc8838c53c2bf6805c79cbd34340e9187df76759c`

How each claim is marked:
- **[V]**: verified from metadata, ldstr literals, call edges or raw IL bytes.
- **[I]**: inferred from names or call shape, not from reading control flow.

No C# was decompiled. The call edges list callees; they do not prove branch order or conditions.

## 1. What the app does

BazaarDBTracker is a desktop companion for Tempo Storm's Unity game **The Bazaar**. It works like this:

1. It tails the game's `Player.log` and parses specific log lines with regexes. [V]
2. When it sees a state change, it attaches to the running `TheBazaar` process and reads live managed game state through **HackF5.UnitySpy**: board, stash, skills, opponent, health, gold, day/hour, wins/losses, hero, rank, combat simulation and frames. [V]
3. It turns this into a stream of typed run events: `RunStarted`, `OptionsPresented`, `Selected`, `Sold`, `Rerolled`, `CombatStart`, `CombatEnd`, `CombatFrames`, `RunEnded` and `RunRebound`. [V]
4. It saves the events to a local SQLite history at `%APPDATA%\BazaarDBCompanion\history.db`. [V]
5. If the user has saved a Bazaar DB secret key and turned on "Run Sync", it uploads the events in gzipped JSON batches to `https://adira.bzdb.network/api/tracker-v2/runs/batch` with a Bearer token. [V]
6. A single Avalonia window lists past runs, a combat-node timeline per run, the events of each run, and the card boards of a selected event. [V]

Card names come from the game's own cache: `GameData.db` or `cards.json` under `%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar\prod\cache`. [V]

## 2. Tech stack

All entries are verified from `deps.json`, the AssemblyRefs and the call edges unless marked otherwise.

| Concern | Library / version | Notes |
|---|---|---|
| Runtime | .NET 10.0.9 self-contained (win-x64) | `runtimeconfig.json` lists `includedFrameworks`. |
| UI | Avalonia 11.2.3 + Avalonia.Themes.Fluent, Skia/HarfBuzz/ANGLE | Compiled XAML (`CompiledAvaloniaXaml.*`), dark theme forced via `ThemeVariant.Dark`. |
| Game memory | HackF5.UnitySpy **1.0.0-beta.7** | Same version string as the app, so it is probably a custom build or fork [I]. `ProcessFacadeWindows`, `UnityProcessFacade`, `AssemblyImageFactory.Create`, `IAssemblyImage.GetTypeDefinition`, `TypeDefinition.GetStaticValue`, `ManagedObjectInstance[...]`. |
| PE parsing | PeNet 2.8.1 + PeNet.Asn1 | **BazaarDBTracker.dll does not reference it.** It is shipped as a UnitySpy dependency, presumably for finding mono exports [I]. |
| Game enum discovery | System.Reflection.MetadataLoadContext 9.0.0 | `GameEnums.Load` reads enum literals from the game's `BazaarGameShared.dll` without loading it. |
| SQLite | Microsoft.Data.Sqlite 9.0.0, SQLitePCLRaw 3.0.3, e_sqlite3 3.50.4 | Used for both the app's history DB and the game's `GameData.db` (read-only, via a temp copy). |
| HTTP | System.Net.Http `SocketsHttpHandler` + `HttpClient`, `GZipStream` | Used only by `EventSubmissionClient`. |
| Secret storage | System.Security.Cryptography.ProtectedData 9.0.0 (DPAPI) | `SecretStore`. |
| JSON | System.Text.Json (`JsonSerializer`, `JsonNode`, `JsonDocument`) | Also uses `Microsoft.CSharp` `dynamic` binder to walk UnitySpy objects. |
| Updates | Velopack 1.2.0 | `Updater`. |
| Log parsing | `[GeneratedRegex]` source-generated regexes | `LogParser.TimestampPrefix`, `StateChangedRe`, `SkillSelectedRe`, `GameSimSummaryRe`, `CardPlacedRe`, `CommandCompletedRe`, `CombatPreparedRe`. |
| Native P/Invoke | `kernel32!GetCurrentProcess`, `CloseHandle`; `advapi32!OpenProcessToken`, `GetTokenInformation` | Only in `BDC.Elevation`, for the admin check. |

Embedded resources: a single manifest resource, `!AvaloniaResources` (244,086 bytes). It contains `/App.axaml`, `/MainWindow.axaml`, `/Assets/Icon.ico` and `/Assets/Icon.png`. [V] SVG path icons (gear, chevrons, edit, trash) are inline `StreamGeometry` strings in `App.!XamlIlPopulate`. [V]

## 3. Namespace and type inventory

The assembly has 204 TypeDefs and 1,784 methods. Compiler-generated closures and state machines are left out below.

### 3.1 App shell, config, update: `BazaarDBCompanion`
- `Program`: `Main`, `AnotherInstanceRunning`, `TryAcquireSingleInstance`, `BuildAvaloniaApp`.
- `App`: an Avalonia `Application`. `OnFrameworkInitializationCompleted` builds the VM and window.
- `AppInfo`: `Version`, resolved from `AssemblyInformationalVersionAttribute`, with fallback `"0.0.0"`.
- `AppSettings` (nested `Model`: `ShowRunDuration`, `SyncEnabled`, `LogPath`): `Load`/`Save` as JSON.
- `SecretStore`: `Load`/`Save`, using DPAPI.
- `Updater`: Velopack integration. Members: `FeedUrl`, `HandleLifecycle`, `CheckInBackgroundAsync`, `ApplyOnExit`, `StagedVersion`, `UpdateStaged` event, `RestartAfterUpdate`.
- `Elevation`: `RelaunchAsAdminIfNeeded`, `IsElevated`, plus the 4 P/Invokes.
- `FileLog`: a `TextWriter` that replaces `Console.Out` and `Console.Error`, with size-based `Rotate`.
- `RelayCommand`: an `ICommand`.

### 3.2 UI views and view-models: `BazaarDBCompanion`
- `MainWindow`: the view. Members: `InitializeComponent`, `OnBrowseLogPath` file picker, compiled XAML closures `XamlClosure_2.Build_*`.
- `MainWindowViewModel`: 130 methods, 55 fields. This is the god object. Its ctor constructs every service (see §4).
- Display models: `RunDisplay`, `EventDisplay` (nested `EncounterInfo`), `CardVisual`, `RunNodeVisual`, `EncounterHeader`, `HourSeparator`, `BdbPalette`.
- `CompiledAvaloniaXaml.*`: generated by the Avalonia XAML compiler.

### 3.3 Game watcher and memory reading: `BazaarDBCompanion.Watcher`
- `GameManager`: the orchestrator. It owns an `IMemoryReader`, an `ILogReader` and a `GameTracker`. Members: `Start`, `ConnectionLoop`, `Read*Safe` wrappers (passed to `GameTracker` as `Func<>`), and the events `EventDetected` and `ConnectionChanged`.
- `IMemoryReader` / `MemoryReader`: `TryConnect`, `Disconnect`, `IsConnected`, `ReadBoardState`, `ReadCombatResult`, `ReadCombatFrames`, `ReadPvpOpponent`, `ReadRunInit`, `ReadSelectedHero`, `ReadPlayerRank`, plus helpers `ReadSimFromBoardManager`, `ReadSimFromAppState`, `ReadActiveRunId`, `ReadOpponentBoardPositions`, `ComputeOpponentVisualPositions`, `SortBySkillOrder`, `HeroName`, `RankName`.
- `Util`: low-level field readers. `ReadGuid`, `ReadGuidStruct` (fields `_a`…`_k`), `ReadInstanceId`, `ReadTier`, `ReadType`, `ReadSize`, `ReadLeftSocketId`, `ReadEnchantment`, `ReadSection`, `ReadAttributes`, `ReadPlayerAttributes`, `ReadOwnerCombatantId`, `ReadTags`, `ReadRunState`, `ReadCurrentEncounterId`, `ReadInt`, `ReadString`, `ReadStringList`, `ReadSkillOrder`, `ReadNullableEnum`.
- `ManagedJson`: a generic UnitySpy object-graph-to-`JsonNode` serializer with a node budget (`Truncated`, `NodesEmitted`). It understands the internals of mono `List` (`_size`/`_items`), `Dictionary` (`_count`/`_entries`), `HashSet` (`_slots`/`_entries`) and `Nullable` (`hasValue`/`value`). Used for combat frames.
- `ILogReader` / `LogReader`: tails `Player.log` with `FileSystemWatcher` and a tail signature to detect truncation or rotation. `LogEventArgs.RawLine`.
- `LogParser.Parse`: maps a line to one of the `LogLine` subtypes `StateChanged`, `SkillSelected`, `GameSimSummary`, `CardPlaced`, `CommandSent`, `RunInitialized`, `CombatStarted` or `CombatCompleted`.
- DTOs: `BoardStateEventArgs` (25 fields), `EntityRef`, `CardKind`, `CombatSimResult`, `CombatFramesResult`, `PvpOpponentInfo`, `RunInitInfo`, `PlayerRankInfo`.

### 3.4 Event model and run state machine: `BazaarDBCompanion.Events`
- `GameTracker`: 53 methods, 46 fields. It turns log lines plus memory snapshots into events. Members: `ProcessRawLine` → `Process` → `HandleStateChanged`, `HandleRunInitialized`, `HandlePlaced`, `HandleSell`, `HandleCombatStart`, `HandleGameSimSummary`, `QueueCombatEnd`, `QueueSelected`, `FlushOptionsLocked`, `StartRunLocked`, `EndRunLocked`, `RebindRunLocked`, `ScheduleCombatCaptureLocked`, `ScheduleSettleLocked`, and `EmitLoop` (an async queue drain).
- `GameEvent` is the base class. Subclasses: `RunStartedEvent`, `OptionsPresentedEvent` (with `PresentedOption`), `SelectedEvent`, `SoldEvent`, `RerolledEvent`, `CombatStartEvent`, `CombatEndEvent`, `CombatFramesEvent`, `RunEndedEvent`, `RunReboundEvent`, `SpawnedCardRef`.
- Enums: `GameEventType {RunStarted, OptionsPresented, Selected, Sold, Rerolled, CombatStart, CombatEnd, CombatFrames, RunEnded, RunRebound}` and `CombatOutcome {Unknown, Win, Loss}`. [V]

### 3.5 Game enums: `BazaarDBCompanion.Game`
- Baked enums: `ECardAttributeType` (84 fields), `ECardTag`, `ECardType`, `ECardSize`, `ETier`, `EHero`, `ERank`, `ERunState {Choice, Combat, Encounter, EndRunDefeat, EndRunVictory, LevelUp, Loot, NewRun, Pedestal, PVPCombat, Shutdown}`, `EContainerSocketId`, `ECombatantId`, `EEnchantmentType`, `EInventorySection`.
- `GameEnums`: `InitializeFrom(managedDir)`, `Load`, `ValueOf`, `Name`, `NameOf`, nested `EnumMap`. It re-maps the baked enums from the live `BazaarGameShared.dll` (`BazaarGameShared.Domain.Core.Types.*`, `BazaarGameShared.Domain.Runs.ERunState`, `BazaarGameShared.TempoNet.Enums.ERank`) and falls back to the baked values if that fails. [V]

### 3.6 Persistence: `BazaarDBCompanion.Persistence`
- `EventStore`: SQLite access. Members: `EnsureSchema`, `MigrateSchema`, `BackfillRunStats`, `InsertEvent`, `UpsertRunInternal`, `GetBatchSendable`, `MarkBatchSent`, `MarkBatchDeadLetter`, `GetRunSummaries`, `RebindRun`, `ClearRunEnd`, `DeleteRun`, `IsRunEnded`, `HasRunStarted`, `ListRuns`, `GetSyncedRunIds`, `ListEvents`, `LoadEvent`.
- Records: `RunSummary`, `EventSummary`, `EventDetail`, `UnsentEvent`. Enum: `SyncStatus {Pending, Sent, Failed, DeadLetter}`, which is 0/1/2/3 in SQL. [V]

### 3.7 Card catalog: `BazaarDBCompanion.Rendering`
- `CardCatalog`: `Initialize`, `LoadFromGameDataDb`, `LoadFromCardsJson`, `ParseCard`, `NewestExisting`.
- `CardInfo`: `Name`, `Tier`, `Size`, `Type`, `Description`.

### 3.8 Submission: `BazaarDBCompanion.Submission`
- `EndpointConfig`: a record with `BatchUrl`, plus `Default()`.
- `EventSubmissionClient`: `SendLoop`, `DrainBatchOnce`, `SendBatch` (with a local `SplitAndRetry`), `BuildBatchBody`, `GzipUtf8`, `LimitBySize`, `BackoffDelay`, `Nudge`, `ExtractApiError`, `HttpErrorText`. Nested `RunDto`: `BzRunId`, `StartedUtc`, `EndedUtc`, `Outcome`, `PlayerHero`, `Day`, `Victories`, `Losses`.

## 4. Startup flow

These are the callees of `BDC.Program.Main` in IL order [V]. The exact branch conditions are [I].

1. `Updater.HandleLifecycle()` runs `VelopackApp.Build().Run()`, which handles Velopack install and update hooks.
2. `Program.AnotherInstanceRunning()` calls `Mutex.TryOpenExisting("Global\\BazaarDBCompanion.SingleInstance")`. It presumably exits if another instance holds the mutex [I].
3. `Elevation.RelaunchAsAdminIfNeeded()` checks `IsElevated()` through the token P/Invokes. If the process is not elevated, it relaunches `Environment.ProcessPath` with verb `runas` and the same args. Admin rights are presumably needed to read the game's memory [I].
4. `Program.TryAcquireSingleInstance()` creates or waits on the same global mutex.
5. `FileLog.Install()` redirects `Console.Out` and `Console.Error` to `%APPDATA%\BazaarDBCompanion\logs\tracker.log`, with rotation. Banner: `=== BazaarDBTracker <ver> started <time> (pid N) ===`.
6. `Updater.CheckInBackgroundAsync()` is fire-and-forget. See §5.6.
7. `BuildAvaloniaApp()` runs `AppBuilder.Configure<App>().UsePlatformDetect().LogToTrace()`, then `StartWithClassicDesktopLifetime(args)`.

Then `App.OnFrameworkInitializationCompleted` runs. It creates `MainWindowViewModel`, creates `MainWindow` with `DataContext = vm`, and calls `vm.StartMonitoring()`. On `Exit` it calls `vm.Dispose()` and then `Updater.ApplyOnExit()`. [V]

There is no DI container. Wiring is manual inside the `MainWindowViewModel` ctor, which calls `AppSettings.Load`, `SecretStore.Load`, `EventStore..ctor`, `CardCatalog.Initialize`, `MemoryReader..ctor`, `LogReader..ctor`, `GameManager..ctor` and `EventSubmissionClient..ctor`. [V] `StartMonitoring` starts `GameManager.Start`, which runs `Task.Run(ConnectionLoop)` and `ILogReader.Start`. [V]

The data flow looks like this [V by call edges, ordering I]:

```
LogReader --LogEventReceived--> GameManager.OnLogLine --> GameTracker.ProcessRawLine
   GameTracker --(Func<> Read*Safe)--> MemoryReader (UnitySpy) 
   GameTracker.EmitLoop --EventDetected--> GameManager --> MainWindowViewModel.OnEventDetected
      --> EventStore.InsertEvent/UpsertRun (SQLite) --> EventSubmissionClient.Nudge
EventSubmissionClient.SendLoop --> EventStore.GetBatchSendable --> POST batch --> MarkBatchSent/DeadLetter
```

## 5. External surfaces

### 5.1 Game process memory
- **Attach.** `MemoryReader.TryConnect` uses `Process.GetProcessesByName("TheBazaar")`, then `new ProcessFacadeWindows(proc)`, then `UnityProcessFacade`, then `AssemblyImageFactory.Create(..., "TheBazaarRuntime")`. It resolves the type definitions `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache`. If `TheBazaar.Data` is missing, it logs "game build changed?". [V]
- **On connect.** `OnConnected` locates `<exeName>_Data\Managed\TheBazaarRuntime.dll`, logs the Unity version (from `FileVersionInfo`) and the DLL timestamp, and calls `GameEnums.InitializeFrom(managedDir)`. [V] A `Managed` folder means a **Mono** (not IL2CPP) Unity build [I].
- **Fields read.** These are backing-field names taken from ldstr literals; full paths are for the deep dive:
  - `ReadBoardState`: `<IsStorageOpen>`, `<CurrentState>`, `<SelectionSet>`, `<CurrentEncounterId>`, `<Run>`, `<Player>`, `<Opponent>`, `<Day>`, `<Hour>`, `<Victories>`, `<Losses>`, `<Hero>`, `Entities` (dictionary). It also reads `EPlayerAttributeType` keys `Health`, `HealthMax`, `Gold`, `Prestige`, `Income` and `Level`, and filters on type `BazaarGameClient.Domain.Models.Cards.ItemCard`.
  - Combat: `BoardManager.Instance._lastCombatSim` and `AppState._cachedCombatSim` / `<Data>`, with `Winner`, `Loser` and `Frames`.
  - Run init: `_cachedRunInitialized` → `<RunId>`, `<PlayMode>`, `<BuildId>`, `<Environment>`.
  - PvP: `<SimPvpOpponent>` → `<Name>`, `<Hero>`, `<Rating>`, `<Level>`.
  - Rank: `Rank._value` → `<SeasonID>`, `<Rank>`, `<Division>`, `<Rating>`.
  - Selected hero: `RunConfig._value.<Hero>`.
  - Opponent positions: `<CardControllerToBoardTarget>` / `<CardData>`. [V]
- **Process death.** `IsProcessDeath(ex)` detects it, then the reader disconnects and `GameManager.ConnectionLoop` reconnects. [V]

### 5.2 Files and paths
`GetFolderPath(ApplicationData)` is shown as `%APPDATA%`. This is verified from the IL `ldc.i4.s 0x1A` in `AppSettings..cctor`, `SecretStore..cctor` and `FileLog.Install`. `EventStore..ctor` uses the same `"BazaarDBCompanion"` folder; its enum constant was not byte-checked, so it is [I], with high confidence.

| Path | Owner | Use |
|---|---|---|
| `%APPDATA%\BazaarDBCompanion\settings.json` | `AppSettings` | `{ShowRunDuration, SyncEnabled, LogPath}`, indented JSON. |
| `%APPDATA%\BazaarDBCompanion\secret.dat` | `SecretStore` | Bazaar DB secret key, DPAPI `Protect`ed on Windows, plain text otherwise. |
| `%APPDATA%\BazaarDBCompanion\logs\tracker.log` (+ rotated backup) | `FileLog` | App log; stdout and stderr are redirected here. |
| `%APPDATA%\BazaarDBCompanion\history.db` | `EventStore` | SQLite history, WAL mode. |
| `<UserProfile>\AppData\LocalLow\Tempo Storm\The Bazaar\Player.log` | `LogReader` | Game log, tailed. Can be overridden by `AppSettings.LogPath`. |
| `<same root>\prod\cache\GameData.db` or `...\cache\GameData.db` | `CardCatalog` | Game card DB. Copied to `%TEMP%\bdbc_gamedata_<guid>.db`, then `SELECT Id, Data FROM cards`. |
| `<same root>\prod\cache\cards.json` | `CardCatalog` | Fallback card source. |
| `<game>\TheBazaar_Data\Managed\BazaarGameShared.dll`, `TheBazaarRuntime.dll` | `GameEnums`, `MemoryReader` | Read via MetadataLoadContext and `FileInfo`. |

### 5.3 SQLite schema (`history.db`)
These are verbatim DDL fragments from `EventStore.EnsureSchema` and `MigrateSchema`. [V]
- `runs(id TEXT PK, startedUtc TEXT NOT NULL, endedUtc TEXT, outcome TEXT, realRunId TEXT, eventCount INTEGER NOT NULL DEFAULT 0)`. Migrations add `day`, `victories`, `losses INTEGER` and `hero TEXT`.
- `events(id INTEGER PK AUTOINCREMENT, runId TEXT NOT NULL REFERENCES runs(id), seq INTEGER NOT NULL, timestampUtc TEXT NOT NULL, type TEXT NOT NULL, payloadJson TEXT NOT NULL, syncStatus INTEGER NOT NULL DEFAULT 0)`. Migrations add `attemptCount`, `nextRetryUtc`, `batchSyncStatus`, `batchAttemptCount` and `batchNextRetryUtc`.
- Indexes: `ix_events_run(runId, seq)`, `ix_events_sync` (partial), `ix_runs_started`, `ix_events_batchsync` (partial).
- `PRAGMA user_version` steps through 2 → 6. Pragmas: `journal_mode=WAL; foreign_keys=ON; busy_timeout=3000`.
- Run ids are prefixed `local-` until the real game RunId is known. `RebindRun` then moves the events over and rewrites `$.BzRunId` in `payloadJson` with `json_set`. [V]

### 5.4 Network endpoints
- **Batch upload.** `POST https://adira.bzdb.network/api/tracker-v2/runs/batch`, which can be overridden with the env var `BAZAARDB_BATCH_API_ENDPOINT`. [V]
  - Body: `{"schemaVersion":1,"clientVersion":<ver>,"runs":[RunDto...],"events":[...]}`, gzipped, `Content-Type: application/json`, `Content-Encoding: gzip`.
  - Headers: `Authorization: Bearer <secret key>`, `User-Agent: BazaarDBCompanion/<ver>`.
  - Error JSON is parsed for `message`, `error`, `detail` and `title`.
  - The client backs off, splits a refused batch and retries (`SplitAndRetry`), and skips events that are too large. [V]
  - An event is sendable only if it has `$.EventId`, its `$.BzRunId` does not start with `local-`, and its run has ended or gone stale. [V]
- **Links opened in the browser.** `https://bazaardb.gg/account` (get key), `https://bazaardb.gg/card/<templateId>` (`CardVisual.CardPage`), `https://bazaardb.gg/run/tracker/<id>` (`RunDisplay.BazaarDbUri`). [V]
- No other URLs appear in the #US heap. [V, bounded to ldstr literals]

### 5.5 Secrets and auth
There is one user-supplied "Bazaar DB secret key". It is entered in Settings, saved by `MainWindowViewModel.SaveKey` through `SecretStore.Save`, and used as the Bearer token. There is no OAuth or login flow. [V]

### 5.6 Update feed
- `Updater.FeedUrl` is `https://depot.bzdb.network/tracker/releases`, or the env var `BAZAARDB_UPDATE_URL`, with the trailing `/` trimmed. [V]
- `CheckInBackgroundAsync` creates `new UpdateManager(feed, new UpdateOptions{ExplicitChannel="beta"})`. If `IsInstalled` is true, it runs `CheckForUpdatesAsync` and then `DownloadUpdatesAsync`, sets `StagedVersion`, and raises `UpdateStaged`. The UI then shows "Update … downloaded. Click to relaunch and update." [V]
- `ApplyOnExit` calls `WaitExitThenApplyUpdates(..., restart: RestartAfterUpdate)`. [V]
- Velopack's own package metadata is in `current/sq.version`: id `BazaarDBTracker`, channel `beta`, shortcuts Desktop and StartMenuRoot.

## 6. Coverage and limitations
- Metadata coverage is complete. The #US user-string decode reported partial status because of about 50 non-standard terminal-flag bytes. The text was still readable, and this is not obfuscation.
- The call-edge order is IL order. It is not a control-flow reconstruction. Conditions, loop timings (poll intervals, backoff constants) and payload JSON shapes still need a CIL-level read or a decompiler.
- `HackF5.UnitySpy.dll` (the custom beta.7 build) was not inspected. Its mono-offset tables decide whether the memory reading works against the current game build.
