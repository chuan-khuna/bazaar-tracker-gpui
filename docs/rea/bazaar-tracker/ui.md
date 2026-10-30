# BazaarDBTracker 1.0.0-beta.7: UI

This is a static deep dive into the UI of `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll` (SHA-256 `ab1734c4…f362d`). The app was never run. Read [overview.md](overview.md) first; `BDC.` stands for the `BazaarDBCompanion.` namespace.

How each claim is marked:
- **[V]**: verified from CIL. This means ldstr/ldc constants, call order, or branch targets read from the method body.
- **[I]**: inferred from names, call shape or Avalonia defaults.

## 0. Method and sources

- **The `.axaml` sources are not in the binary.** `!AvaloniaResources` holds only `/Assets/Icon.ico`, `/Assets/Icon.png` and an `AvaloniaResourceXamlInfo` map (`BDC.App → /App.axaml`, `BDC.MainWindow → /MainWindow.axaml`). The XAML compiler stripped the text. [V]
- The layout below is rebuilt from the CIL of `App.!XamlIlPopulate`, `MainWindow.!XamlIlPopulate`, and the `XamlClosure_1/2.Build_*` template and resource factories.
- The CIL was disassembled with a small `System.Reflection.Metadata` script. It used no installed tools and did not execute the target. Output is in the scratchpad: `il.txt` holds the full IL, and `app_c.txt`, `mw_c.txt` and `mwc_c.txt` hold condensed XAML op streams.
- How Avalonia enum constants are read: `Dock` Left=0 Bottom=1 Right=2 Top=3; `HorizontalAlignment` Stretch=0 Left=1 Center=2 Right=3; `VerticalAlignment` Top=1 Center=2; `Orientation` Horizontal=0; `Stretch.Uniform`=2; `StretchDirection.DownOnly`=1; `GridUnitType` Auto=0 Pixel=1 Star=2.

## 1. Window layout

`MainWindow` properties [V]:
- Title `"Bazaar DB Tracker"`, icon `/Assets/Icon.ico`.
- `Width=1000`, `Height=640`, `MinWidth=820`, `MinHeight=520`.
- `WindowStartupLocation=CenterScreen`.
- The Fluent dark theme is forced (`RequestedThemeVariant = Dark`).

### 1.1 Tree

```
Window (bg BdbBackgroundBrush #000, font BdbFontFamily)
└─ Grid  Rows: Auto, Auto, *
   ├─ [row 0] Border  bg Surface1, border Outline, BorderThickness 0,0,0,1           ── HEADER BAR
   │   └─ DockPanel  Margin 16,0  Height 48
   │      ├─ (Dock=Left) StackPanel H spacing 8, VCenter
   │      │   └─ TextBlock "Bazaar DB Tracker"  11pt 600 Tertiary  margin 0,2,0,0
   │      └─ (Dock=Right) StackPanel H spacing 8, HAlign Right, VCenter
   │          ├─ Button.node  Command=UpdateNowCommand  IsVisible=HasUpdateReady  ToolTip=UpdateReadyText
   │          │   └─ pill Border bg Surface2, border Heal 1px, radius 100, pad 10,3
   │          │       └─ ● Ellipse 8×8 Heal  + "Update ready" 12pt Secondary
   │          ├─ pill Border (IsVisible=SyncDisabled, tooltip "Runs are captured to local history. They sync to Bazaar DB once a secret key is saved and Run Sync is on (⚙).")
   │          │   └─ ● Ellipse 8×8 Primary  + "Local only — no key" 12pt Secondary
   │          ├─ pill Border
   │          │   └─ ● Ellipse 8×8 Fill=ConnectionIndicatorBrush + {ConnectionStatus} 12pt Secondary
   │          └─ Button.icon  ToolTip "Settings"  content PathIcon IconGear 16×16
   │              └─ Flyout (Placement=10 [I: BottomEdgeAlignedRight]) → SETTINGS PANEL (§1.3)
   ├─ [row 1] Border  bg BdbErrorTintBrush, border Outline 0,0,0,1, IsVisible=HasError  ── ERROR STRIP
   │   └─ DockPanel Margin 16,4
   │      ├─ (Right) Button.icon "✕" 11pt  Command=DismissErrorCommand
   │      └─ TextBlock {LastError} 12pt Damage, VCenter, Wrap
   └─ [row 2] Grid  Columns: Auto, *   Margin 12                                     ── BODY
      ├─ [col 0] Border.panel  Width 34  margin 0,0,12,0  IsVisible=IsRunsCollapsed   (collapsed rail)
      │   └─ Button.icon (Top, HCenter, margin 0,6,0,0) "Show runs" → ToggleRunsCommand, PathIcon IconChevronRight 14×14
      ├─ [col 0] Border.panel  Width 248  margin 0,0,12,0  IsVisible=!IsRunsCollapsed  ── RUNS SIDEBAR
      │   └─ DockPanel
      │      ├─ (Top) DockPanel margin 12,8,8,4
      │      │   ├─ (Left) "RUNS" .sectionTitle
      │      │   ├─ (Right) Button.icon "Hide runs" → ToggleRunsCommand  (IconChevronLeft 14)
      │      │   ├─ (Right) Button.icon "Edit — show delete buttons on runs" → ToggleRunsEditCommand (IconEdit 14)
      │      │   └─ (fill, HAlign Right) Button.icon "Refresh" → RefreshCommand (inline 18×18 canvas in a 14×14 Viewbox: 2 arcs + 2 arrowheads, stroke 2, Fill/Stroke = $parent[Button].Foreground, second half at opacity 0.6)
      │      ├─ (Top) TextBlock "No runs yet — start a run in The Bazaar and it will appear here."  12pt Tertiary wrap margin 12,24,12,0  IsVisible=!HasRuns
      │      └─ ListBox ItemsSource=Runs SelectedItem=SelectedRun Padding 4,0,4,4 ItemTemplate=RUN ROW (§1.4)
      └─ [col 1] Border.panel                                                          ── RUN PANEL
          └─ Grid Rows: Auto, *   IsVisible=HasSelectedRun
             ├─ [row 0] Border border Outline 0,0,0,1 pad 12,8                          ── RUN HEADER
             │   └─ Grid Rows: Auto, Auto
             │      ├─ [r0] DockPanel
             │      │   ├─ (Left) StackPanel H spacing 8:
             │      │   │    {SelectedRun.StartedText} 14pt 600 ·
             │      │   │    {SelectedRun.HeroText} 12pt 600 Secondary (IsVisible SelectedRun.HasHero) ·
             │      │   │    {RunRankText} 12pt Secondary tooltip "Season standing at run start" (IsVisible HasRunRank) ·
             │      │   │    {RunRankDeltaText} 12pt 600 Fg=RunRankDeltaBrush tooltip "Season standing after the run" (IsVisible HasRunRankDelta) ·
             │      │   │    chip "LIVE" 9pt 700 Heal, border Heal (IsVisible SelectedRun.IsLive) ·
             │      │   │    {SelectedRun.OutcomeText} 12pt 600 Fg=SelectedRun.OutcomeBrush
             │      │   └─ (Right) StackPanel H spacing 12 HAlign Right:
             │      │        HyperlinkButton "See on Bazaar DB ↗" 12pt NavigateUri=SelectedRun.BazaarDbUri (IsVisible SelectedRun.IsSynced) ·
             │      │        {RunSyncSummary} 12pt Tertiary
             │      └─ [r1] DockPanel margin 0,8,0,0  IsVisible=HasRunCombatNodes          ── COMBAT TIMELINE
             │          ├─ (Left) StackPanel H spacing 8 margin 0,0,12,0:
             │          │    {SelectedRun.DayText} 12pt 600 · {SelectedRun.RecordText} 12pt 600 Fg=OutcomeBrush
             │          └─ ScrollViewer H=Auto V=Disabled
             │              └─ StackPanel H margin 0,0,0,14
             │                 ├─ ItemsControl ItemsSource=RunCombatNodes, panel = H StackPanel, template = NODE (§1.5)
             │                 └─ Rectangle 56×2 VTop margin 0,8,0,0 IsVisible=SelectedRun.IsLive
             │                      fill LinearGradient (0,.5)→(1,.5): #1FFFFFFF → #00FFFFFF   ("run continues" fade)
             └─ [row 1] Grid Columns: *, Auto
                ├─ [col 0] ListBox x:Name="EventsList" ItemsSource=Events SelectedItem=SelectedEvent Padding 4
                │     ItemsPanel = VirtualizingStackPanel (transparent bg)
                │     DataTemplates by type: HourSeparator, EncounterHeader, EventDisplay (§1.6)
                └─ [col 1] Border Width 400 border Outline 1,0,0,0 IsVisible=HasSelectedEvent ── DETAIL PANE
                    └─ Grid Rows: Auto, *
                       ├─ [r0] StackPanel margin 12,6,6,2 spacing 2
                       │   ├─ DockPanel: (Right) Button.icon "✕" 11pt → CloseDetailCommand; {DetailTitle} 12pt 600 Secondary
                       │   └─ {DetailBanner} 12pt 600 Fg=DetailBannerBrush IsVisible=HasDetailBanner
                       └─ [r1] ScrollViewer margin 8,2,8,8 → StackPanel spacing 6
                           ├─ ItemsControl DetailEncounterCards (WrapPanel H) template CardVisualTemplate  IsVisible HasDetailEncounterCards
                           ├─ "OPPONENT" .sectionTitle margin 4,2,0,0         IsVisible HasDetailOpponentCards
                           ├─ Viewbox Stretch=Uniform Dir=DownOnly HAlign Left  IsVisible HasDetailOpponentCards
                           │   └─ ItemsControl DetailOpponentCards (H StackPanel) template CardVisualTemplate
                           ├─ "PLAYER" .sectionTitle margin 4,2,0,0           IsVisible HasDetailOpponentCards (sic)
                           └─ Viewbox Stretch=Uniform Dir=DownOnly HAlign Left  IsVisible HasDetailCards
                               └─ ItemsControl DetailCards (H StackPanel) template CardVisualTemplate
```
All of this tree is [V], except the Flyout placement name, which is [I].

### 1.2 ASCII mockup (1000×640, settings closed, run and event selected)

```
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│ Bazaar DB Tracker                          (● Update ready) (● Local only — no key) (● Tracking) ⚙ │ 48px, Surface1
├──────────────────────────────────────────────────────────────────────────────────────────────┤
│ Failed to record Selected: …                                                             ✕  │ only if HasError
├────────────────────┬─────────────────────────────────────────────────────────────────────────┤
│ RUNS      ⟳ ✎ ‹   │ 10/30/2026 9:14 PM  Vanessa  Gold III · 812  [LIVE]  Victory   See on Bazaar DB ↗  Synced │
│┌──────────────────┐│ Day 7  7W–2L   ─D1─●─D2─●─·─D3─● ──── ▶ fade                                          │
││10/30 9:14 PM LIVE││─────────────────────────────────────────────────────┬───────────────────────────│
││Vanessa 7W–2L Day7││ 0:00  D1H1  Run Started   Vanessa · Ranked         │ Options · #12 · 9:20:11 PM… ✕│
│└──────────────────┘│ 0:12  H2    Options       4 options · Choice        │ Picked 1 of 4               │
│ 10/29 8:02 PM      │ 0:15        ↳ Selected    Fang: Item (Bronze)       │ ┌─────┐┌──────────┐┌─────┐  │
│ Dooley 10W–0L Day9 │ ───────────────────────────────────────────────────  │ │Fang ││ Katana   ││ …   │  │
│ …                  │ Encounter   Merchant name                           │ │Item·││ Item·Gold││     │  │
│                    │ 1:02  D2H4  Combat Start  PvP vs Bob (Mak)          │ └─────┘└──────────┘└─────┘  │
│  248px sidebar     │ 1:40        Combat End    Victory                   │ OPPONENT / PLAYER boards    │
│                    │                    events list (*)                  │      400px detail pane      │
└────────────────────┴─────────────────────────────────────────────────────┴───────────────────────────┘
  12px body margin; panels are Border.panel (Surface1, 1px Outline, radius 8)
```

### 1.3 Settings flyout (StackPanel width 320, spacing 8) [V]

1. **SECRET KEY** (`.sectionTitle`). DockPanel: `Save` button (Dock Right, margin 8,0,0,0, `SaveKeyCommand`) + TextBox `SecretKey` with `PasswordChar='●'` and watermark `"Paste your Bazaar DB secret key..."`.
2. `{KeyStatus}` 11pt wrap, `Foreground=KeyStatusBrush`.
3. "Your Bazaar DB secret key authorizes run sync." 11pt Tertiary.
4. HyperlinkButton "Get your secret key." → `https://bazaardb.gg/account` (11pt, padding 0, HAlign Left).
5. 1px divider (Border height 1, bg Outline).
6. **RUN SYNC**: CheckBox `IsChecked=SyncEnabled` "Upload captured runs to Bazaar DB" (12pt). Help text: "Runs are always saved to local history first, then upload in the background and retry automatically. Turn off to keep runs local only."
7. Divider. **LOG FILE**:
   - TextBox `LogPathInput`, watermark "Path to The Bazaar's Player.log".
   - Right-aligned H StackPanel: `Browse…`, which is a code-behind Click handler `OnBrowseLogPath`, and `Save` (`SaveLogPathCommand`).
   - `{LogPathStatus}` 11pt Heal.
   - Help text: "Change this if the log is kept somewhere non-standard and isn't detected. Restart the app to apply."
8. Divider. **DISPLAY**: CheckBox `ShowRunDuration` "Show run duration instead of clock time".
9. Divider. `{AppVersionText}` 11pt centered Tertiary. Below it, HyperlinkButton `{UpdateReadyText}` (Heal, `UpdateNowCommand`, IsVisible `HasUpdateReady`).

`OnBrowseLogPath` opens `StorageProvider.OpenFilePickerAsync` with:
- title "Select The Bazaar's Player.log"
- filters `Log files (*.log)` and `All`
- `AllowMultiple=false`
- start folder = the directory of the current `LogPathInput` if it exists

It then sets `LogPathInput` to the chosen file's `LocalPath`. Errors go to stderr as "Log-path browse failed: …". [V]

### 1.4 Run row template (`XamlClosure_2.Build_3`) [V]

```
Grid Columns: *, Auto        ContextFlyout = MenuFlyout { MenuItem "Delete run" Command=DeleteCommand CommandParameter={Binding} }
├─ [c0] StackPanel spacing 2
│   ├─ H spacing 6: {StartedText} 13pt 600 · chip "LIVE" (IsLive; Heal 9pt 700)
│   └─ H spacing 8: {HeroText} 11pt Secondary (HasHero) · {RecordText} 11pt 600 Fg=OutcomeBrush · {DayText} 11pt Tertiary
└─ [c1] Button.icon VCenter, tooltip "Delete run", IsVisible=$parent[ListBox].DataContext.IsRunsEditMode,
        content PathIcon IconTrash 14×14 (Fg Damage),
        Flyout (width 190): "Delete this run from local history? This can't be undone." 12pt wrap
                             + Button "Delete" HAlign Right, bg Damage, fg #14120F, no border → DeleteCommand(param = row)
```

### 1.5 Combat node template (`Build_5`) [V]

```
StackPanel H
├─ Rectangle 16×2 VTop margin 0,8,0,0 fill OutlineStrong   IsVisible=HasLeadIn      (connector line)
└─ Button.node  IsHitTestVisible=IsClickable  Command=$parent[ItemsControl].DataContext.SelectCombatNodeCommand  CommandParameter={Binding}  ToolTip=Tooltip
    └─ StackPanel V
       ├─ Grid Height 18 → Border Width=Size Height=Size CornerRadius 100 Background=Fill, centered (dot)
       └─ TextBlock {Label} 9pt Tertiary HCenter margin 0,1,0,0
```

### 1.6 Event list templates [V]

- **`HourSeparator`** (`Build_7`): a 1px Rectangle, fill Outline, margin 4,3.
- **`EncounterHeader`** (`Build_8`): a Grid with columns `78px, 46px, *` and margin 0,2,0,0. Column 2 holds an H StackPanel with spacing 6: "Encounter" (12pt 600, Tag brush) and `{Name}` (12pt Secondary).
- **`EventDisplay`** (`Build_9`): a Grid with columns `78px, 46px, *`.
  - c0: `{TimeColumnText}` 11pt Tertiary, tooltip `TimeColumnTooltip`.
  - c1: `{HourText}` 10pt 600 Tertiary, tooltip `DayHourText`.
  - c2: a Grid with columns `96px, *` and `Margin={IndentMargin}`.
    - c0: H StackPanel spacing 4: "↳" 11pt Tertiary (IsVisible `HasParent`) and `{TypeText}` 12pt 600 `Fg=TypeBrush`.
    - c1: `{SummaryText}` 12pt `Fg=SummaryBrush`, ellipsis, margin 8,0.
- **ListBoxItem containers.** The code-behind hook `ContainerPrepared` (`MainWindow.<.ctor>b__5_1`) sets these on each container [V]:
  - For `HourSeparator` and `EncounterHeader` rows: `IsHitTestVisible=false` and `Focusable=false`.
  - For every row: `MinHeight=0`.
  - Padding is `0` for separators and `8,4` for other rows.
- **Auto-scroll.** `MainWindow` subscribes to `Events.CollectionChanged`. Before the change it records whether the list was within **40 px** of the bottom (`IsPinnedToBottom`). Then it posts one `ScrollIntoView(last)` at `DispatcherPriority.Background`, but only if it was pinned and no event is selected. Repeated changes are coalesced through `_scrollQueued`. [V]

### 1.7 Card template (`CardVisualTemplate`, `Build_2`) [V]

```
Border Width={Width} Height=96 Margin={Margin} bg Surface2 BorderBrush={TierBrush} BorderThickness={BorderThickness} CornerRadius 8 ToolTip={Tooltip}
└─ Grid Rows: *, Auto
   ├─ [r0] TextBlock {Name} 12pt 600 wrap, centered (H and V), margin 6,4
   ├─ [r1] TextBlock {Caption} 9pt Tertiary centered, ellipsis, margin 4,0,4,5
   └─ [r0] HyperlinkButton "↗" 10pt pad 3,1 top-right, NavigateUri={CardUri}, IsVisible={HasCardLink}, tooltip "View card on Bazaar DB"
```
Geometry, from `CardVisual` getters [V]:
- `Width = SizeUnits*64 − 6`
- `Margin = (3 + GapUnits*64, 3, 3, 3)`
- `BorderThickness = Picked ? 4 : 2`
- `UnitsForSize`: "Small"→1, "Large"→3, else 2. One board slot is 64 px, so a 10-slot board is 640 px wide; the `Viewbox` (DownOnly) shrinks it to fit the 400 px pane [I].

## 2. View-model: `MainWindowViewModel`

The view model implements `INotifyPropertyChanged` itself. Its commands are all `RelayCommand`, with `CanExecute` always `true`. Bindings are `ReflectionBindingExtension` (runtime reflection bindings, not compiled ones). [V]

### 2.1 Collections and selection
| Property | Type | Drives |
|---|---|---|
| `Runs` | `ObservableCollection<RunDisplay>` | Runs list. `RefreshRuns` rebuilds it with `store.ListRuns(200)`, `GetSyncedRunIds`, `IsRunLive`. |
| `HasRuns` | bool | Empty-state text. |
| `SelectedRun` / `HasSelectedRun` | `RunDisplay` | The setter clears `SelectedEvent`, sets `followLive = (value == Runs.FirstOrDefault())` and calls `LoadEventsForSelectedRun`. It is ignored while `suppressSelection`. |
| `Events` | `ObservableCollection<object>` | Event list. It holds a mix of `HourSeparator`, `EncounterHeader` and `EventDisplay`. |
| `RunCombatNodes` / `HasRunCombatNodes` | `ObservableCollection<RunNodeVisual>` | Timeline. |
| `SelectedEvent` / `HasSelectedEvent` | `EventDisplay` | The setter raises `DetailTitle` and calls `LoadSelectedEventDetail`. |
| `DetailTitle` | string | `"{TypeText} · #{Seq} · {TimeText}"` plus `" · {DayHourText}"` when one is present. |
| `DetailCards` / `DetailOpponentCards` / `DetailEncounterCards` (+ `Has*`) | `ObservableCollection<CardVisual>` | Detail pane boards. |
| `DetailBanner` / `DetailBannerBrush` / `HasDetailBanner` | string / brush | Detail sub-header. |
| `EventDetailJson` | string | The pretty-printed payload (`WriteIndented`). **It is not bound by any XAML**, so it is a vestigial raw-JSON view. [V: no binding path "EventDetailJson" in MainWindow IL] |

### 2.2 Header and status
| Property | Shows |
|---|---|
| `ConnectionStatus` | `"Waiting for game..."` (initial and when disconnected), `"Tracking"` (connected), or `"Log capture stopped"` (the log task faulted). |
| `IsConnected` → `ConnectionIndicatorBrush` | `Heal` (#8EEA31) when connected, else `Tertiary` (#888888). |
| `SyncDisabled` | `IsNullOrWhiteSpace(savedKey) || !settings.SyncEnabled`. Shows the "Local only — no key" pill. |
| `LastError` / `HasError` | Error strip. Set by `OnClientError` (submission errors), by "Failed to record {Type} — will retry: {msg}" or "Failed to record {Type}: {msg}" (persist failures, with a retry queue of ≤256 and a 5 s delay), and by "Log capture stopped: {msg}". `DismissErrorCommand` sets it to "". |
| `UpdateReadyText` / `HasUpdateReady` | `"Update {version} downloaded. Click to relaunch and update."`. Set from `Updater.UpdateStaged`, or from `StagedVersion` at construction. |
| `AppVersionText` | `AppInfo.Version` (e.g. `1.0.0-beta.7`) with no prefix. |
| `RunSyncSummary` | `"Local only"` (sync disabled or `local-` id), `"Synced"`, `"Uploads when the run ends"`, `"Syncing — N pending"`, `"Synced — N skipped"`, or `"Syncing — N pending · M skipped"`. Counts come from `EventSummary.SyncStatus`. |
| `RunRankText` / `RunRankDeltaText` / `RunRankDeltaBrush` (+ `Has*`) | Built by `UpdateRunRankHeader` (§2.4). |

### 2.3 Settings-bound properties
| Property | Behaviour |
|---|---|
| `SecretKey` | Two-way text. `SaveKey` handles it. |
| `KeyStatus` / `KeyStatusBrush` | Possible values: "Key saved" (Heal); "No key saved — runs are kept locally until one is set" (Tertiary); "Key cannot be empty" (Primary); "Save failed: {msg}" (Damage). |
| `SyncEnabled` | Writes `AppSettings` and saves, calls `client.SetEnabled`, raises `SyncDisabled`, then `QueueRunsRefresh`. |
| `ShowRunDuration` | Writes `AppSettings` and saves, then reloads events. |
| `LogPathInput` / `LogPathStatus` | `SaveLogPath` trims the input. Empty or equal to `LogReader.DefaultLogPath` stores `null`. It saves, then sets "Saved — restart the app to apply." |
| `IsRunsCollapsed`, `IsRunsEditMode` | UI toggles. They are not persisted. |

### 2.4 Commands [V]
| Command | Action |
|---|---|
| `SaveKeyCommand` | `SaveKey`. If the key is unchanged it just shows "Key saved". Otherwise `SecretStore.Save`, `client.UpdateApiKey`, clear `LastError`, raise `SyncDisabled`. |
| `SaveLogPathCommand` | `SaveLogPath`. |
| `RefreshCommand` | `RefreshRuns`. |
| `CloseDetailCommand` | `SelectedEvent = null`. |
| `DismissErrorCommand` | `LastError = ""`. |
| `ToggleRunsCommand` / `ToggleRunsEditCommand` | Flip `IsRunsCollapsed` / `IsRunsEditMode`. |
| `DeleteRunCommand` (exposed per row as `RunDisplay.DeleteCommand`) | `store.DeleteRun(id)`. If it was the selected run, clear the selection and event, then `RefreshRuns`. |
| `SelectCombatNodeCommand(RunNodeVisual)` | `SelectedEvent = Events.OfType<EventDisplay>().FirstOrDefault(e => e.Event.Id == node.EventId)`. |
| `UpdateNowCommand` | `Updater.RestartAfterUpdate = true`, then `ApplicationLifetime.Shutdown()`. The exit hook applies the update (see app-shell.md). |

### 2.5 Threading [V]
- Game events arrive on a worker thread through `GameManager.EventDetected`. `OnEventDetected` persists them synchronously (`InsertEvent`, `RebindRun`, `ClearRunEnd`) and calls `client.Enqueue`. It then marshals UI work with `Dispatcher.UIThread.Post`.
- `QueueRunsRefresh` coalesces refreshes with a `runsRefreshQueued` flag and posts at `DispatcherPriority.Background`.
- `RefreshRuns` keeps the selection by run id. If `followLive` is set, it jumps to the newest run. Then it reloads events.
- `IsRunLive(run)` = `EndedUtc == null && isConnected && LastEventUtc` is within **30 min** of now.
- `CardCatalog.Loaded` triggers `QueueRunsRefresh` and `BuildDetailVisuals`, so names fill in once the catalog finishes loading in the background.

## 3. Theme: colors, fonts, styles

### 3.1 Resource palette (`App.axaml`) [V]
| Key | Value |
|---|---|
| `SystemAccentColor` | `#EB813B` (orange). Dark1 `#D26F2D`, Dark2 `#B85E22`, Dark3 `#9E4E19`, Light1 `#F09554`, Light2 `#F4A96E`, Light3 `#F8BD89`. |
| `BdbBackgroundColor` / `Brush` | `#000000` |
| `BdbSurface1Color` / `Brush` | `#14120F` |
| `BdbSurface2Color` / `Brush` | `#292521` |
| `BdbSurface3Color` / `Brush` | `#3E3B35` |
| `BdbSurface4Color` / `Brush` | `#544F4D` |
| `BdbOutlineBrush` | `#FFFFFF` at opacity 0.06 |
| `BdbOutlineStrongBrush` | `#FFFFFF` at opacity 0.12 |
| `BdbForegroundBrush` | `#FFFFFF` |
| `BdbSecondaryBrush` | `#CCCCCC` |
| `BdbTertiaryBrush` | `#888888` |
| `BdbPrimaryBrush` | `#EB813B` |
| `BdbDamageBrush` | `#F5503D` |
| `BdbHealBrush` | `#8EEA31` |
| `BdbTagBrush` | `#98A8FE` |
| `BdbErrorTintBrush` | `#F5503D` at opacity 0.12 |
| `BdbFontFamily` | `"Inter, Segoe UI Variable Text, Segoe UI, sans-serif"` |
| `BdbMonoFontFamily` | `"Cascadia Mono, Consolas, monospace"`. It is defined but not used in MainWindow. |
| `IconGear` | An 18×18 gear path (StreamGeometry; full path string in `il.txt` under `App/XamlClosure_1::Build_18`). |
| `IconChevronLeft` | `M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z` (Material, 24-box) |
| `IconChevronRight` | `M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z` |
| `IconEdit` | `M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z` |
| `IconTrash` | `M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z` |

The fonts are not embedded. Inter is used only if the user has it installed; the fallback is Segoe UI. [V for the resource list, I for the effect]

### 3.2 Code palette: `BdbPalette` (static `SolidColorBrush`es built with `Color.Parse`) [V]
| Field | Hex | Used for |
|---|---|---|
| `Foreground` | `#FFFFFF` | |
| `Secondary` | `#CCCCCC` | Default summary text; unknown outcome. |
| `Tertiary` | `#888888` | Disconnected dot; Rerolled/CombatFrames type; day-boundary dot. |
| `Primary` | `#EB813B` | "Selected" type; "Key cannot be empty". |
| `Gold` | `#FFD700` | Win outcome; RunStarted type. |
| `Damage` | `#F5503D` | Loss; CombatStart type; combat banner. |
| `Heal` | `#8EEA31` | Connected; key saved; rank up. |
| `Freeze` | `#3FC8F7` | CombatEnd type. |
| `Tag` | `#98A8FE` | OptionsPresented type. |
| `Legendary` | `#FF4500` | Tier border; RunEnded type. |
| `Bronze` | `#B46241` | Tier border. |
| `Silver` | `#C0C0C0` | Tier border; Sold type. |
| `Diamond` | `#00FFFF` | Tier border. |
| `Outline` | `#FFFFFF` at 0.12 opacity | Tier border fallback. |
| `Surface3` | `#3E3B35` | Unknown-outcome combat dot. |
| `Navy` | `#2D5C9E` | Won combat dot; opponent header card. |

`TierBrush(tier)` maps "Bronze", "Silver", "Gold", "Diamond" and "Legendary" to the brush of the same name; anything else gets `Outline`. [V]

### 3.3 Global styles (`App.Styles`, after `FluentTheme`) [V]
| Selector | Setters |
|---|---|
| `Window` | Background BdbBackground; FontFamily BdbFontFamily |
| `TextBlock` | Foreground BdbForeground |
| `Border.panel` | bg Surface1, border Outline, 1px, radius 8 |
| `Border.card` | bg Surface2, border Outline, 1px, radius 8 (not used in MainWindow) |
| `Border.chip` | Padding 6,1; radius 4; 1px border OutlineStrong; VCenter |
| `TextBlock.sectionTitle` | 600 weight, 12pt, Tertiary, LetterSpacing 1 |
| `TextBox` | bg Background (#000), border OutlineStrong, radius 4, fg Foreground, padding 8,4, MinHeight 28. `:pointerover` and `:focus` keep the `PART_BorderElement` bg at Background. |
| `Button` | bg Surface3, border Outline 1px, radius 8, fg Secondary, padding 10,4. `:pointerover /template/ ContentPresenter#PART_ContentPresenter` gets bg Surface4 and fg Foreground. |
| `Button.accent` | bg Primary, fg #14120F, 600 weight, no border. On hover, bg #F09554 and fg #14120F. Not used in MainWindow. |
| `Button.icon` | Transparent bg, no border, padding 6,4, fg Secondary. On hover, presenter bg Surface3 and fg Foreground. |
| `Button.node` | Transparent, no border, padding 0, MinWidth/MinHeight 0, Cursor = `new Cursor(9)` (StandardCursorType 9 = Hand [I]). The hover presenter stays transparent. |
| `FlyoutPresenter` | bg Surface2, border OutlineStrong 1px, radius 8, padding 12 |
| `ListBox` | Transparent bg |
| `ListBoxItem` | padding 8,4; radius 4; margin 4,1; MinHeight 0. `:pointerover` presenter bg Surface3; `:selected` presenter bg Surface4. |
| `ScrollBar` | Transparent bg |

## 4. Display models and how they are built from payloads

All display models are C# `record`s, so they have value equality and `ToString`/`PrintMembers`. [V]

### 4.1 `RunDisplay(RunSummary Run, bool IsLive)` + `DeleteCommand`, `IsSynced` [V]
| Getter | Value |
|---|---|
| `StartedText` | `StartedUtc.ToLocalTime().ToString("g")` |
| `OutcomeText` | "Win"→"Victory", "Loss"→"Defeat", else "". |
| `OutcomeBrush` | Gold / Damage / Secondary. |
| `RecordText` | `"{V}W–{L}L"` (en dash). Empty unless both counts are present. |
| `DayText` | `"Day {n}"`. |
| `HeroText` / `HasHero` | `Run.Hero ?? ""`. |
| `BazaarDbUri` | `https://bazaardb.gg/run/tracker/{Run.Id}` |

### 4.2 Event list (`LoadEventsForSelectedRun`) [V, with order details I]
1. `store.ListEvents(runId)` runs, then `UpdateRunRankHeader(events, ended)`.
2. `CombatFrames` events are **skipped** from the list.
3. Duration: `elapsed = ts − firstTs − pausedTime`. `pausedTime` accumulates the gap after each `RunEnded` event to the next event, which covers resumed or rebound runs. `FormatDuration` gives `m:ss`, or `h:mm:ss` once the elapsed time reaches 1 h.
4. Each event becomes an `EventDisplay`:
   - **OptionsPresented**: its option `InstanceId`s are indexed in a dictionary.
   - **Selected**: if its `InstanceId` matches an option, it gets `IsLinkedSelection=true`, `LinkedOptions = parent`, and indent = parent + 1. The `PickedOptionIds` of the parent gets the id.
   - **Encounter events**: an `EncounterHeader(TemplateId)` row is inserted when `GetEncounterInfo` finds `Context == "EncounterState"` with a new card instance. The payload fields are `Context`, `Board.currentEncounterId`, and `opponentBoard[]` items whose `type` is one of `CombatEncounter|EventEncounter|PedestalEncounter|PvpEncounter`, using `instanceId`/`templateId`. A synthetic key `"chain-without-card"` is used when there is no card. Following events in the chain get `IndentLevel` and `EncounterTemplateId`.
   - **CombatStart, CombatEnd and RunEnded** reset the chain [I].
5. `PvpOpponentName` is taken from `CombatStart.OpponentName` when `IsPvp`, and applied to events whose ids start with `pvp_`.
6. When the `HourValue` changes between consecutive rows, an `HourSeparator` is inserted.
7. `RunSyncSummary` is computed, then `BuildCombatNodes`. The previous `SelectedEvent` is restored by id.

`EventDisplay` getters [V]:
- `TypeText`: RunStarted→"Run Started", OptionsPresented→"Options", Selected/Sold/Rerolled unchanged, CombatStart→"Combat Start", CombatEnd→"Combat End", CombatFrames→"Combat Data", RunEnded→"Run Ended".
- `TypeBrush`: RunStarted Gold, Options Tag, Selected Primary, Sold Silver, Rerolled Tertiary, CombatStart Damage, CombatEnd Freeze, CombatFrames Tertiary, RunEnded Legendary, default Secondary.
- `HourText`: `D{day}H{hour}`, `H{hour}`, `D{day}` or "", from payload `Day`/`Hour`. `DayHourText` is "Day N · Hour M".
- `TimeText`: local time `"T"` format. `TimeColumnText` shows `DurationText` when `ShowDuration` is on, otherwise `TimeText`. The tooltip shows the other value.
- `IndentMargin` = `Thickness(IndentLevel*16, 0, 0, 0)`. `HasParent` = `IndentLevel > 0`.
- `SummaryText` (cached `BuildSummary`), by type:
  - Selected/Sold: `"{Name}: {CardKind} ({Tier})"` plus `" · merged"` when `MergedIntoInstanceId` is set. The name comes from the `CardCatalog`. A `pvp_` id becomes the PvP opponent name or "PvP Opponent".
  - OptionsPresented: `"{n} options · {Context minus 'State'}"`, or the encounter name.
  - CombatStart: `"PvE"` or `"PvP vs {OpponentName} ({OpponentHero})"`.
  - RunStarted: `PlayerHero · PlayMode`.
  - Rerolled: "Item: …".
  - CombatFrames: `FrameCount × 0.05 s`, shown as `~12.3s` or `~1m 05.2s`, with `≥… · truncated` when `Truncated`. The brush is Tertiary.
  - CombatEnd/RunEnded: Victory (Gold), Defeat (Damage) or "Unknown outcome" (Tertiary). RunEnded appends `RunStatsSuffix` (`" · {V}W–{L}L · Day {d}"`).

### 4.3 Combat nodes (`BuildCombatNodes`, `RunNodeVisual`) [V]
- Only `CombatEnd` events are used. Each one reads `Outcome` (a string enum name or int, as `CombatOutcome`), `Day` and `Hour`.
- When the day changes, `RunNodeVisual.DayBoundary(day, hasLeadIn)` is added: Size **7**, fill Tertiary, label "", tooltip `"Day N begins"`, no EventId, so it is not clickable.
- Each combat adds `RunNodeVisual.Combat(outcome, hour, day, hasLeadIn, eventId)`:
  - Size **18**.
  - Fill: Win→**Navy #2D5C9E**, Loss→Damage, else Surface3.
  - Label `D{day}`.
  - Tooltip: `"Victory|Defeat|Unknown outcome · Day d · Hour h"`.
- `HasLeadIn` = there is a node before this one, which draws the 16×2 connector [I from the Count check].
- `IsClickable` = `EventId.HasValue`.

### 4.4 Detail pane (`BuildDetailVisuals`) [V]
The pane is cleared first, and the banner brush is reset to Tertiary. `CombatFrames` gets no visuals, only the pretty JSON in the unused `EventDetailJson`. By event type:
- **Selected**:
  - The parent's `LinkedOptions` payload `Options[]` is shown as cards. The option whose id matches the selection is `Picked`.
  - Banner: "Picked 1 of N".
  - `AddEncounterCardsFromSnapshot` runs. If no encounter card was found, the event's `EncounterTemplateId` is shown as an "Encounter" card.
- **Sold**: one card from `TemplateId/CardKind/Tier/Enchantment`. Banner "Sold", plus `"merged into an owned copy (…)"` when `MergedIntoInstanceId` is set.
- **OptionsPresented**:
  - Every option is shown as a card, with `PickedOptionIds` marked.
  - Banner: `"{n} options presented · {Context-State}"` plus `" · {k} picked"`.
  - Encounter cards as for Selected.
- **CombatStart**: `AddCombatBoardsFrom(payload)`. The banner is "PvE combat" or "PvP vs Name (Hero)" plus `" · Rating N"`, in Damage.
- **CombatEnd/RunEnded**:
  - Outcome banner, with `RunStatsSuffix` for RunEnded.
  - Boards come from the **last `CombatStart` with a lower `Seq`**.
- **RunStarted**: banner `PlayerHero · PlayMode · BuildId`.
- **`AddCombatBoardsFrom`**:
  - For PvP, it adds an "Opponent" header card (Navy border) to the encounter row, with name, hero and `Rating N`.
  - It calls `AddBoardCards(DetailCards, payload, "PlayerBoard")` and `AddBoardCards(DetailOpponentCards, payload, "OpponentBoard")`.
- **`AddBoardCards`**:
  - Items are read as `{type, templateId, tier, enchantment, instanceId, size, boardPosition | socketId:"Socket_N"}` and sorted by socket, with unknown sockets last.
  - Encounter-type items go to `DetailEncounterCards`, deduped by templateId.
  - The other items are laid out left to right: `gap = max(0, socket − cursor)`, then `cursor = socket + units`. This puts each card at its true board slot on a 64 px grid.
- **`AddCard`**:
  - `Name` = override ?? `CardCatalog.Name` ?? instanceId ?? kind ?? "?".
  - `Caption` = the non-empty parts of `[kind, tier, enchantment, "picked"]` joined with " · ".
  - `TierBrush` = `BdbPalette.TierBrush(tier ?? catalog.Tier)`.
  - `SizeUnits` = `UnitsForSize(size ?? catalog.Size)`.
  - `Tooltip` = `catalog.Description`.
  - `TemplateId` gives the card link `https://bazaardb.gg/card/{templateId}`.

### 4.5 Rank header (`UpdateRunRankHeader`) [V]
- It reads `RunStarted.PlayerRating/PlayerRank/PlayerRankDivision` and the `RunEnded.*After` fields.
- `Sane` rejects ratings ≥ 1,000,000 and divisions > 10.
- `Standing` = `"{Rank} {Roman(division)} · {rating}"` (Roman I–X), or just the rating.
- The delta text is `"→ {standing after} (+N)"`, in Heal when up, Damage when down, and Secondary otherwise.

## 5. Card data: `Rendering.CardCatalog` [V]
- `Initialize()` runs once (`Interlocked.Exchange` guard) on `Task.Run`:
  - It tries `LoadFromGameDataDb`, then `LoadFromCardsJson`.
  - If both fail it logs "CardCatalog: no game data found — card names will not resolve.".
  - On success it sets the static `Dictionary<string, CardInfo>` (`OrdinalIgnoreCase`) and fires `Loaded`.
- Root folder: `GetFolderPath(UserProfile)` (`ldc 40`) + `AppData\LocalLow\Tempo Storm\The Bazaar`.
- `NewestExisting(candidates)` picks the existing file with the newest `LastWriteTimeUtc`. The candidates are:
  - `<root>\prod\cache\GameData.db` or `<root>\cache\GameData.db`
  - `<root>\prod\cache\cards.json` or `<root>\cache\cards.json`
- **GameData.db**:
  1. The file is opened with `FileMode.Open, FileAccess.Read, FileShare.ReadWrite`, so it does not fight the running game.
  2. It is copied to `%TEMP%\bdbc_gamedata_<guid:N>.db`.
  3. That copy is opened with `Mode=ReadOnly, Pooling=false`.
  4. `SELECT Id, Data FROM cards` runs. `Id` is TEXT, and `Data` is read as a **`byte[]` BLOB of UTF-8 JSON**.
  5. On failure it logs "…read failed (…), trying cards.json". The temp copy is deleted afterwards.
- **cards.json**: the root is an **object whose values are arrays** of card objects. Each card has its `Id` property.
- **`ParseCard(id, json)`**, which builds `CardInfo(Name, Tier, Size, Type, Description)`:
  - `Name` = `Localization.Title.Text` ?? `InternalName` ?? id
  - `Tier` = `StartingTier`
  - `Size` = `Size`
  - `Type` = `Type`
  - `Description` = `Localization.Description.Text`
- **There are no card images.** Nothing in the assembly references `Bitmap` or `ImageBrush`. A card is a colored box: the border color is the tier, and the width is the size. Tier comes from the event payload (current tier) and falls back to the card's `StartingTier`.

## 6. GPUI porting notes [I]
- **Entities**:
  - `AppState`, which replaces the view model. It holds the runs, events, selection and settings, and emits change notifications with `cx.notify()`.
  - Separate `Entity<T>` views for the `RunsSidebar`, `RunPanel` (header + timeline), `EventList`, `DetailPane` and `SettingsPopover`.
  - The worker threads (log tailer, memory reader, uploader) send messages over a channel. They are drained on the foreground executor, which replaces `Dispatcher.UIThread.Post`.
- **Theme**: put the §3 tokens in one `Theme` struct, as a global. Background `#000`, surfaces `#14120F/#292521/#3E3B35/#544F4D`, outline as `white.opacity(0.06/0.12)`. Font fallback is Inter, then Segoe UI.
- **Lists**:
  - Use `uniform_list` for the runs list.
  - The event list mixes row heights (1 px separators), so use `list` (variable height, `ListState`). Alternatively make separators fixed-height rows and keep `uniform_list`.
  - Reproduce the stick-to-bottom-within-40 px rule with `ListState`/`ScrollHandle`.
- **Custom elements**:
  - The combat timeline: dots, connectors and the gradient fade. Plain `div`s with `rounded_full` and `linear_gradient` background are enough.
  - The board strip: an absolute 64 px slot grid scaled to fit the pane, which replaces `Viewbox` DownOnly. Measure the width and scale with `px(64 * scale)`.
  - The refresh icon and the path icons: ship them as SVG assets via `svg().path(...)`.
- **Widgets missing from core GPUI**:
  - Popover/flyout (settings, delete confirm), context menu, checkbox, password `TextInput`, hyperlink, and tooltip.
  - Use `gpui-component` (longbridge), or write a small popover with `deferred(anchored(...))`.
  - For the file picker, use `cx.prompt_for_paths`.
- **Card links** open `https://bazaardb.gg/card/{id}` with `cx.open_url`.

## 7. Unknowns
- The flyout placement enum value `10` is assumed to be `BottomEdgeAlignedRight`.
- The exact chain-reset rules in `LoadEventsForSelectedRun` (which events end an encounter chain) were read only at call level.
- The PNG/ICO icon sizes were not decoded. The resource blob contains four PNGs, about 10.5 KB and 22.9 KB, which look like the ICO frames plus `Icon.png`.
- The behaviour of the Fluent theme defaults that were not overridden (CheckBox, HyperlinkButton colors) has to be matched by eye.
