# UnitySpy: Tooling, GUIs, tests and build

This doc covers everything in `references/unityspy` outside the core library: the two GUI browsers, the shared view-models, the debugging helpers, the test project and the native build. Read [overview.md](overview.md) for context.

How each claim is marked:
- **[V]**: verified from source.
- **[I]**: inferred.

## 1. Projects [V]

| Project | Framework | UI stack | Platforms |
|---|---|---|---|
| `HackF5.UnitySpy.Gui` | `netstandard2.0` | none (view-models only), Autofac 4.9.2 | — |
| `HackF5.UnitySpy.Gui.Avalonia` | `netcoreapp3.1`, `Platforms=x64` | Avalonia 0.10.10, ReactiveUI, Autofac 6.3.0 | Windows, Linux, macOS |
| `HackF5.UnitySpy.Gui.Wpf` | .NET Framework 4.7.2 | WPF, Autofac | Windows |

`.vscode/tasks.json` and `.vscode/launch.json` build and launch the Avalonia GUI from `bin/Debug/netcoreapp3.1/`. [V]

Neither GUI is a CLI. There is **no command-line tool** in the tree. [V]

## 2. Avalonia browser

**Entry point.** `Program.Main` → `AppBuilder.Configure<App>().UsePlatformDetect()`, with Skia GPU resource cap 8,096,000 and `AllowEglInitialization`, then `.UseReactiveUI()` (`src/HackF5.UnitySpy.Gui.Avalonia/Program.cs:13-23`). [V]

**Main window** (`ViewModel/MainWindowViewModel.cs`) [V]:
- **Process list.** `Process.GetProcesses()` sorted by name. After every refresh it auto-selects a process named **`MTGA.exe`** (`:236`), a leftover from Magic: The Gathering Arena work. On Linux under Wine the process name includes `.exe`.
- **Mode combo** (`Modes`, `:71-105`):
  - Windows: "Direct", "Linux Dump Files".
  - macOS: "Direct (root access)", "Linux Dump Files", "Client (user access) - server.c (root access)".
  - Linux: "Direct (root access)", "Dump Files", "Client …", "PTrace (process_vm_readv)".
  - The mode index maps to a facade in `CreateProcessFacade` (`:406-464`). See [process-access.md](process-access.md).
- **Selecting a process:**
  - On Windows and macOS this immediately builds the image.
  - On Linux it only fills `MemPseudoFilePath = /proc/<pid>/mem`, and the user then presses a build button (`:109-127`).
- **`BuildImageAsync`** (`:305-404`):
  1. Get the offsets: from `GetMainModuleFileName()` (Windows), `Process.MainModule.FileName` (macOS), or the chosen or auto-detected exe path (Linux and dump mode).
  2. `new UnityProcessFacade(facade, offsets)`.
  3. **`AssemblyImageFactory.Create(unityProcess)` with the default assembly `Assembly-CSharp`.** The GUI has **no input for the assembly name.** To browse another assembly (e.g. The Bazaar's `TheBazaarRuntime`), the code must be changed. [V for the call; I that The Bazaar's code is not in `Assembly-CSharp`, based on the tracker's use of `TheBazaarRuntime`]
  4. Failures are shown as `Failed to load process <name> (<id>).` plus the exception message.
  5. It then dumps every type name and field count to the console, and prints whether a type starting with `PAPA` exists. That is a leftover debug check, apparently for MTGA [I].
- **Type list** (`AssemblyImageViewModel`) [V]:
  - All `TypeDefinitions`, ordered by `FullName`, with a text filter.
  - It auto-selects `CollectionManager` (`:117`), a Hearthstone/MTGA leftover [I].

**Path navigation** (`ViewModel/TypeDefinitionViewModel.cs:107-182`) [V]:
- A dotted path box, e.g. `s_instance.m_items._items.3`. Each segment is applied with the `dynamic` indexer:
  - a name segment reads a field (static on a type, instance on an object);
  - an **integer segment indexes an `IList`** (an array).
- It stops at the first segment that throws or yields something that is not an `IMemoryObject` or `IList`.
- The trail shows each step. "Back" drops the last segment.
- **Content views:**
  - type → static fields (`HasStaticFields` filters out `const`);
  - object → instance fields with type names and values;
  - list → indexed items.
  Field values that throw are shown as `ERROR: <message>` (`HackF5.UnitySpy.Gui/ViewModel/InstanceFieldViewModel.cs:42-55`).

**Raw memory viewer** (`ViewModel/RawMemoryViewModel.cs`, `View/RawMemoryView.xaml`) [V]:
- Inputs: a start address (hex) and a buffer size (default 1,024).
- It reads with `allowPartialRead: true` and renders 32 bytes per line as hex plus a Latin-1 column.
- When a type is selected, it opens at that `MonoClass*` address (`MainWindowViewModel.cs:288-303`).
- **This is the practical tool for checking offsets by eye** (see [offsets.md](offsets.md) §6).
- A quirk: the hex loop indexes `buffer[i + j]` for 32 bytes per line, so a buffer size that is not a multiple of 32 would throw [I].

## 3. WPF browser [V]

`src/HackF5.UnitySpy.Gui.Wpf/ViewModel/MainViewModel.cs`:
- It has the same flow as Avalonia, Windows only:
  1. `ProcessFacadeWindows(process)`
  2. `GetOffsets(GetMainModuleFileName())`
  3. `UnityProcessFacade`
  4. `AssemblyImageFactory.Create(...)`, again with the default `Assembly-CSharp` (`:59-66`).
- It auto-selects a process named **`Hearthstone`** (`:93`).
- It uses Autofac registration through `[Register]` attributes (`HackF5.UnitySpy.Gui/Mvvm/RegisterAttribute.cs`) and a `ViewLocator` naming convention.

## 4. Helpers in the library [V]

- **`Util/MemoryReadingUtils`.**
  - `ReadMemory(address, length, stepSize = 4, recursiveDepth = 0)` walks memory in steps.
  - At each step it prints int32, uint32, ptr32, ptr64, ASCII and UTF-16 interpretations.
  - If the value is a pointer, it also tries `char*` and optionally recurses (it does not revisit a pointer).
  - It writes the result to `Memory Dump - <timestamp>.txt` in the **current directory**.
  - Nothing in the tree calls it. It is a manual exploration aid for finding offsets.
- **`ProcessFacade.ReadModule`.** Copies a whole module out of the target. It is used for export parsing, but it is also handy for offline disassembly.

## 5. Tests [V]

- `src/HackF5.UnitySpy.Tests/HackF5.UnitySpy.HearthstoneLib.Tests.csproj` references `..\HackF5.UnitySpy.HearthstoneLib\HackF5.UnitySpy.HearthstoneLib.csproj`, **which does not exist in this tree**.
- The tests (`TestHearthstoneLibApi.cs`) call `new MindVision().GetCollectionCards()`, `GetMatchInfo()` and `GetDungeonInfoCollection()`. They need a running Hearthstone, so they are integration tests for a downstream library (HearthSim/Firestone-style "MindVision" [I]).
- The project is not in the `.sln`.
- **There are no unit tests for the core library.** No test exercises offsets, decoding or value reading against fixtures.

## 6. Native build [V]

- `src/HackF5.UnitySpy.Native/Makefile` has `linux` and `macos` targets. Both run `gcc -o server server.c -lpthread`. `server.c` `#include`s `linux.c` or `macos.c` depending on the platform macro.
- There is **no target for `macos.dylib`**, which `ProcessFacadeMacOSDirect` P/Invokes and which the library csproj bundles from `../macos.dylib`. It must be built by hand (e.g. `gcc -dynamiclib macos.c`) [I].
- See [process-access.md](process-access.md) §6 for the server protocol and its bugs.

## 7. Usefulness for our project [I]

- **Exploring The Bazaar's classes.** The Avalonia GUI could browse the game's types if `AssemblyImageFactory.Create(unityProcess)` were changed to pass `"TheBazaarRuntime"`, and if the game's Unity version maps to a table that works. We would build and run it locally, outside this repo; nothing here needs to ship.
- **Verifying a Rust port.** Run the GUI and the Rust reader side by side on the same static path (e.g. a singleton's field) and compare the values. Avoid `float` fields, because UnitySpy reads them wrong.
- **Not worth porting**: the GUIs, the dump-file mode, the TCP server and `MemoryReadingUtils`. A Rust debug command that prints a class's fields with offsets and type codes would replace the browser for development.
