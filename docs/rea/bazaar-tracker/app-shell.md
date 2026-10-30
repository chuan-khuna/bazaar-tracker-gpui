# BazaarDBTracker 1.0.0-beta.7: App shell, config, update

This is a static deep dive into the process shell of `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll`. The app was never run. Read [overview.md](overview.md) for context; the UI is in [ui.md](ui.md).

The source is CIL disassembled with `System.Reflection.Metadata`. The dump is in the session scratchpad as `il.txt`.

How each claim is marked:
- **[V]**: verified from CIL (constants, call order, branch targets).
- **[I]**: inferred.

Types covered: `Program`, `App`, `AppInfo`, `AppSettings`, `Updater`, `Elevation`, `FileLog` and `RelayCommand`.

## 1. Startup sequence (`Program.Main`) [V]

```
Main(args):
  Updater.HandleLifecycle()                  // VelopackApp.Build().Run() — must be first (handles --veloapp-* hooks)
  if AnotherInstanceRunning(): return        // before elevation, so an unelevated duplicate never prompts UAC
  if Elevation.RelaunchAsAdminIfNeeded(args): return   // true => an elevated copy was launched (or the attempt failed) → this process exits
  if !TryAcquireSingleInstance(): return
  FileLog.Install()
  _ = Updater.CheckInBackgroundAsync()       // fire-and-forget
  AppBuilder.Configure<App>().UsePlatformDetect().LogToTrace().StartWithClassicDesktopLifetime(args)
```

`App`:
- `Initialize` loads the compiled XAML: the forced dark theme, the palette and the styles.
- `OnFrameworkInitializationCompleted` builds `MainWindowViewModel`, creates `MainWindow { DataContext = vm }`, sets it as `MainWindow`, and calls `vm.StartMonitoring()`.
- It hooks `lifetime.Exit`, which calls `vm.Dispose()` and then `Updater.ApplyOnExit()`. [V]

## 2. Single instance [V]
- The mutex name is **`Global\BazaarDBCompanion.SingleInstance`**. Because of the `Global\` prefix it is machine-wide, across sessions.
- `AnotherInstanceRunning()`:
  - Returns `true` when `Mutex.TryOpenExisting` succeeds. The handle is disposed right away.
  - Returns `false` when the open fails normally.
  - Returns `true` when it **throws** (e.g. `UnauthorizedAccessException`). That happens when an elevated instance owns the mutex and this caller is not elevated [I for the exception type].
- `TryAcquireSingleInstance()`:
  - Calls `new Mutex(initiallyOwned: true, name, out createdNew)`.
  - If the mutex was newly created, it returns `true`. Otherwise it returns `mutex.WaitOne(TimeSpan.Zero)`.
  - It has two catch handlers. The first returns `true`; it is probably `AbandonedMutexException`, where the previous owner crashed [I]. The second returns `false`.
  - The mutex is kept in the static `singleInstanceMutex` for the life of the process.
- When the app exits because another instance is running, it shows nothing: there is no "already running" message and the existing window is not focused. [V: no other calls]

## 3. Admin relaunch (`Elevation`) [V]
- The exe manifest says `requestedExecutionLevel level="asInvoker"`, in both the apphost exe and the dll. Elevation is therefore done at runtime, not through the manifest. [V]
- `IsElevated()`:
  1. `OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY=8, out h)`
  2. `GetTokenInformation(h, TokenElevation=20, out int elevated, 4, out _)`
  3. The result is `elevated != 0`. It returns `false` on any failure.
  4. `CloseHandle(h)` runs in a `finally`.
- `RelaunchAsAdminIfNeeded(args)`:
  - If already elevated, it returns `false`.
  - Otherwise it runs `Process.Start(new ProcessStartInfo(Environment.ProcessPath) { UseShellExecute = true, Verb = "runas", Arguments = string.Join(' ', args) })`.
  - The args are joined with spaces and **not re-quoted**, so an argument containing spaces would split [V shape, I effect].
  - Exceptions are swallowed; for example, the user declining UAC raises `Win32Exception 1223`.
  - It **always returns `true` when not elevated**, so the unelevated process exits even if the UAC prompt was cancelled. The app therefore cannot run without admin.
- **Why it is needed [I]**: HackF5.UnitySpy reads `TheBazaar.exe` memory with `ReadProcessMemory`. That needs `PROCESS_VM_READ` / `PROCESS_QUERY_INFORMATION` on the game process. Steam games normally run at the same integrity level as the user, so admin is not strictly needed in the common case. It may be required when the game (or its anti-cheat or launcher) runs elevated, or to defeat handle-access restrictions. The authors chose to require it unconditionally.

## 4. Settings (`AppSettings`) [V]
- Path: `Environment.GetFolderPath(ApplicationData)` (`ldc 26`) → **`%APPDATA%\BazaarDBCompanion\settings.json`**.
- Model (`AppSettings.Model`, serialized by `System.Text.Json` with default naming, i.e. PascalCase keys):

| Field | Type | Default | Meaning |
|---|---|---|---|
| `ShowRunDuration` | bool | **true** | Show elapsed run time instead of clock time in the event list. |
| `SyncEnabled` | bool | **true** | Upload to Bazaar DB (it still needs a saved key). |
| `LogPath` | string? | null | Override for `Player.log`. `null` means `LogReader.DefaultLogPath`. |

- `Load()`: if the file exists, it deserializes it (null becomes `new Model()`). Any exception also falls back to `new Model()`.
- `Save(model)`: `Directory.CreateDirectory`, then `File.WriteAllText(JsonSerializer.Serialize(model, { WriteIndented = true }))`.
- Example file:
  ```json
  {
    "ShowRunDuration": true,
    "SyncEnabled": true,
    "LogPath": null
  }
  ```
- Settings are saved immediately by the view-model setters. A `LogPath` change needs a restart ("Saved — restart the app to apply."), because `LogReader` is built once in the VM ctor.
- Related: the secret key is in **`%APPDATA%\BazaarDBCompanion\secret.dat`** (`SecretStore`, DPAPI). See overview §5.5.

## 5. Version (`AppInfo`) [V]
`Version` is resolved once in the static ctor:
1. `AssemblyInformationalVersionAttribute`, cut at the first `'+'` (removes the SourceLink commit hash). This gives `1.0.0-beta.7`.
2. Otherwise `AssemblyName.Version.ToString(3)`.
3. Otherwise `"0.0.0"`.

The version is used for the log banner, the settings footer, and the upload client `User-Agent` / `clientVersion`.

## 6. Updates (`Updater`, Velopack 1.2.0) [V]
- `HandleLifecycle()` runs `VelopackApp.Build().Run()`, the first call in `Main`.
- `FeedUrl` is `Environment.GetEnvironmentVariable("BAZAARDB_UPDATE_URL")?.TrimEnd('/') ?? "https://depot.bzdb.network/tracker/releases"` [V]. Only an unset variable falls back to the default. A variable set to an empty string gives an empty feed URL, and `UpdateManager` then probably throws, which the catch swallows [I].
- `CheckInBackgroundAsync()`:
  1. `mgr = new UpdateManager(FeedUrl, new UpdateOptions { ExplicitChannel = "beta" })`.
  2. `if (!mgr.IsInstalled) return`. This means the portable or dev build never updates itself. The `BazaarDBTracker-beta-Portable` folder does contain `Update.exe` + `current\sq.version`, so Velopack probably treats the portable copy as installed [I].
  3. `info = await mgr.CheckForUpdatesAsync().ConfigureAwait(false)`; if it is null, return.
  4. `await mgr.DownloadUpdatesAsync(info)`. This downloads the update but does not apply it.
  5. It stores `stagedManager = mgr` and `stagedAsset = info.TargetFullRelease`, and sets `StagedVersion = asset.Version.ToString()`.
  6. It logs `"Update {v} downloaded; installs when the app exits."` and raises `UpdateStaged?.Invoke(version)`.
  7. All exceptions are swallowed silently. There is one check per launch, with no periodic re-check [V: no timer/loop].
- UI hookup: `MainWindowViewModel` subscribes to `UpdateStaged` and also reads `StagedVersion` at construction, in case the download finished first. It shows the "Update ready" pill and the settings link. `UpdateNowCommand` sets `RestartAfterUpdate = true` and calls `Shutdown()`.
- `ApplyOnExit()` runs from `lifetime.Exit`. If `stagedManager` and `stagedAsset` are set, it calls `mgr.WaitExitThenApplyUpdates(asset, silent: true, restart: RestartAfterUpdate, restartArgs: null)`. Failures are logged as "Update apply-on-exit failed: …".
  - Net behaviour: a downloaded update is **always** installed when the app quits. The only difference is that it relaunches when the user clicked "Update ready".
- Package metadata (`current\sq.version`):
  - id `BazaarDBTracker`, authors "Bazaar DB", version `1.0.0-beta.7`, channel `beta`
  - mainExe `BazaarDBTracker.exe`, shortcuts `Desktop,StartMenuRoot`, AUMID `velopack.BazaarDBTracker`
- Velopack's channel convention (`releases.beta.json`, OS-specific RID) means the feed URL serves `…/releases/releases.beta.json` [I from Velopack conventions; no network requests were made].

## 7. Logging (`FileLog`) [V]
- `Install()`:
  - Log directory: `%APPDATA%\BazaarDBCompanion\logs\`, which it creates. The file is **`tracker.log`**.
  - It creates `new FileLog(path, tee: Console.Error)` (the original stderr) and sets it as **both `Console.Error` and `Console.Out`**.
  - It writes a blank line and then `=== BazaarDBTracker {Version} started {Now:yyyy-MM-dd HH:mm:ss zzz} (pid {pid}) ===`.
- Writing:
  - Every write takes a lock, writes to the file and flushes immediately.
  - It counts UTF-8 bytes. The starting count is the existing file length from `OpenAppend`.
  - It also writes to the tee (the original stderr).
  - It uses UTF-8 without BOM, opening with `FileMode.Append, FileAccess.Write, FileShare.ReadWrite`.
- **Rotation**: once the byte count reaches **2,000,000 bytes (~1.9 MiB)**:
  1. Close the writer.
  2. Delete `tracker.5.log`.
  3. Shift `tracker.4.log`→`.5`, down to `.1`→`.2`.
  4. Move `tracker.log`→`tracker.1.log`.
  5. Reopen `tracker.log`.

  The backup name is `ChangeExtension(path, null) + "." + n + ".log"`. So the app keeps the **current file + 5 backups (≤ ~12 MB)**. Failures while rotating are swallowed. If reopening fails, the writer is null and file logging stops.
- There is no log level and no structured logging. All diagnostics are `Console.WriteLine` / `Console.Error.WriteLine` strings.

## 8. `RelayCommand` [V]
- A minimal `ICommand` with two ctors: `RelayCommand(Action)`, which wraps into `Action<object?>`, and `RelayCommand(Action<object?>)`.
- `CanExecute` is always `true`. `CanExecuteChanged` exists but is never raised.

## 9. Files and identifiers at a glance
| Item | Value |
|---|---|
| Mutex | `Global\BazaarDBCompanion.SingleInstance` |
| App data dir | `%APPDATA%\BazaarDBCompanion\` (settings.json, secret.dat, history.db, logs\tracker.log) |
| Update feed | `https://depot.bzdb.network/tracker/releases` (env `BAZAARDB_UPDATE_URL`), channel `beta` |
| Upload endpoint | `https://adira.bzdb.network/api/tracker-v2/runs/batch` (env `BAZAARDB_BATCH_API_ENDPOINT`). See overview. |
| Game data root | `%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar\` |
| Log rotation | 2,000,000 bytes, 5 backups |

## 10. Rust/GPUI porting notes [I]
- **Single instance**: `CreateMutexW(None, true, w!("Global\\BazaarDBCompanion.SingleInstance"))` via the `windows` crate, then check `ERROR_ALREADY_EXISTS`. Keep the same name if both apps must exclude each other; otherwise choose a new name. A better UX would focus the existing window, for example with a named pipe or `FindWindow` + `SetForegroundWindow`.
- **Elevation**:
  - Check with `GetTokenInformation(TokenElevation)`.
  - Relaunch with `ShellExecuteExW` and `lpVerb = "runas"`. Quote each argument properly.
  - Do not exit when UAC is cancelled. Consider elevating only when `OpenProcess(PROCESS_VM_READ)` on the game fails, rather than always.
- **Settings**: a `serde` struct with `#[serde(rename_all = "PascalCase", default)]` and defaults `true/true/None`, so the existing `settings.json` can be read. Use `dirs::config_dir()` (= `%APPDATA%`).
- **Logging**: `tracing` + `tracing-appender`, or a custom size-based roller (2 MB, 5 files). Keep the banner format so support logs still compare.
- **Updates**:
  - Velopack has a Rust SDK (`velopack` crate) with `VelopackApp::build().run()`, `UpdateManager::new(feed, Some(UpdateOptions { ExplicitChannel: Some("beta") }))`, `check_for_updates`, `download_updates` and `wait_exit_then_apply_updates`.
  - This mirrors the C# flow one-to-one. Packaging with `vpk pack` keeps the same feed layout.
- **App lifecycle**: in GPUI, do the update apply and the store/uploader shutdown in `cx.on_app_quit`, which replaces `lifetime.Exit`.

## 11. Unknowns
- The exact exception types in the `TryAcquireSingleInstance` and `AnotherInstanceRunning` catch clauses (only handler shapes were read).
- Whether Velopack's `IsInstalled` is true for the portable folder layout, which decides whether portable users ever get updates.
- The real reason admin is needed was not confirmed. It would take a run against the game, or reading UnitySpy's `OpenProcess` access mask.
