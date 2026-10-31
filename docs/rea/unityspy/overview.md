# UnitySpy: Overview

This is a static survey of the UnitySpy source tree in `references/unityspy`. Nothing was built or run, and no game was attached.

- Upstream: `https://github.com/hackf5/unityspy`. The README points to a Medium walkthrough ("Hacking into Unity games") and a Discord server.
- Snapshot: commit `56227cbe9b6654126f48499929a560c1c9238c1f`, 2024-09-17, "Merge pull request #23 from riQQ/add-2022-3-42f1-offsets". The history has 68 commits.
- License: MIT, "Copyright (c) 2019 brian t" (`LICENSE`).
- Language: C#. The core library targets `netstandard2.0` (`src/HackF5.UnitySpy/HackF5.UnitySpy.csproj:4`). There is also a small amount of C for a native helper.
- All paths below are relative to `references/unityspy`. Line numbers refer to the snapshot commit.

How each claim is marked:
- **[V]**: verified by reading the source.
- **[I]**: inferred. This covers Mono struct-member names, runtime behaviour that the source does not show, and anything taken from general knowledge of Mono, .NET or Windows.

## 1. What the library does

UnitySpy reads the managed (C#) state of a running Unity game that uses the **Mono** scripting backend. It runs in a separate process and only reads memory. It never injects code, calls into Mono, or writes memory. [V: there are no write or remote-thread APIs anywhere in `src/HackF5.UnitySpy`]

It works like this:
1. It picks a table of Mono struct offsets for the game's Unity version and bitness (`Offsets/MonoLibraryOffsets.cs`). [V]
2. It finds the Mono runtime DLL in the target (`mono-2.0-bdwgc.dll` on Windows, `libmonobdwgc-2.0.dylib` on macOS) and resolves the exported function `mono_get_root_domain`. [V]
3. It decodes that function's machine code to find the global `MonoDomain*`, walks the domain's loaded assemblies, and picks one by name. The default is `Assembly-CSharp`. [V]
4. It enumerates the image's class cache and builds a `TypeDefinition` for every loaded class. [V]
5. The caller then reads static fields from a class, follows object references, and reads instance fields, strings and arrays by field name. [V]

**There is no IL2CPP support.** The code depends entirely on the Mono export `mono_get_root_domain` and Mono's `MonoClass` layout. The string "il2cpp" appears nowhere in the tree. [V]

## 2. Repository layout

| Path | Kind | Target | Purpose |
|---|---|---|---|
| `src/HackF5.UnitySpy/` | Library | `netstandard2.0` | The core. Public interfaces at the root, Mono object model in `Detail/`, offset tables in `Offsets/`, OS backends in `ProcessFacade/`, helpers in `Util/`. Depends on `PeNet` 2.8.1 (only for reading the game exe's version) and `Microsoft.CSharp` (for `dynamic`). |
| `src/HackF5.UnitySpy.Gui/` | Library | `netstandard2.0` | View-models shared by both GUIs (`InstanceFieldViewModel`, `StaticFieldViewModel`, `ListContentViewModel` …). |
| `src/HackF5.UnitySpy.Gui.Avalonia/` | Exe | `netcoreapp3.1`, x64 | The cross-platform browser (Avalonia 0.10.10, ReactiveUI, Autofac). See [tooling.md](tooling.md). |
| `src/HackF5.UnitySpy.Gui.Wpf/` | Exe | .NET Framework 4.7.2 | The original Windows-only browser. |
| `src/HackF5.UnitySpy.Native/` | C | gcc | A TCP memory-read server for Linux and macOS (`server.c`), and `macos.c`, the source of the `macos.dylib` used by the macOS direct backend. |
| `src/HackF5.UnitySpy.Tests/` | Tests | `netcoreapp3.0` | MSTest tests for a `HackF5.UnitySpy.HearthstoneLib` project that **is not in the tree**, so they cannot build. [V] |
| `src/HackF5.UnitySpy.sln` | Solution | | Lists the library, `.shared`, Gui, Gui.Wpf and Gui.Avalonia. It does not list Tests or Native. [V] |
| `.vscode/` | Config | | Build and launch for the Avalonia GUI. |

The core library is small: about 3,400 lines of C# in 38 files, including blank lines and comments. [V: `wc -l`]

## 3. Public API

Public surface (`src/HackF5.UnitySpy/*.cs`) [V]:

| Type | Members |
|---|---|
| `AssemblyImageFactory` (static) | `Create(UnityProcessFacade process, string assemblyName = "Assembly-CSharp") → IAssemblyImage` |
| `IAssemblyImage` | `TypeDefinitions`, `GetTypeDefinition(fullName)`, `dynamic this[fullTypeName]` |
| `ITypeDefinition` | `Fields`, `FullName`, `Name`, `NamespaceName`, `IsEnum`, `IsValueType`, `TypeInfo`, `GetField(name, declaringTypeFullName?)`, `GetStaticValue<T>(name)`, `dynamic this[fieldName]` (static read) |
| `IFieldDefinition` | `Name`, `DeclaringType`, `TypeInfo` |
| `ITypeInfo` | `IsStatic`, `IsConstant`, `TypeCode`, `TryGetTypeDefinition(out …)` |
| `IManagedObjectInstance` | `TypeDefinition`, `GetValue<T>(fieldName[, declaringTypeFullName])`, `dynamic this[fieldName]`, `dynamic this[fieldName, typeFullName]` |
| `MonoLibraryOffsets` | Four static tables, plus `GetOffsets(gameExePath, force = true)` and `GetOffsets(versionString, is64Bits, format, force)` |
| `UnityProcessFacade` | Wraps a `ProcessFacade` and an offset table. |
| `ProcessFacadeWindows`, `ProcessFacadeLinuxDirect`/`PTrace`/`Dump`/`Client`, `ProcessFacadeMacOSDirect`/`Client` | OS backends. See [process-access.md](process-access.md). |

Typical use, in the shape the Avalonia and WPF GUIs use (`src/HackF5.UnitySpy.Gui.Wpf/ViewModel/MainViewModel.cs:63-66`) [V]:

```csharp
var facade  = new ProcessFacadeWindows(Process.GetProcessById(pid));
var offsets = MonoLibraryOffsets.GetOffsets(facade.GetMainModuleFileName());
var unity   = new UnityProcessFacade(facade, offsets);
var image   = AssemblyImageFactory.Create(unity, "Assembly-CSharp");

var manager = image["SomeNamespace.SomeManager"];     // TypeDefinition (dynamic)
var inst    = manager["s_instance"];                   // static field → ManagedClassInstance
var items   = inst["m_items"]["_items"];               // List<T> internals → object[]
int count   = inst["m_items"]["_size"];
```

The field names in the last three lines are illustrative only. Values come back as boxed primitives, `string`, `object[]`, `ManagedClassInstance`, `ManagedStructInstance` or `null`. [V: `ProcessFacade.ReadManaged`, `src/HackF5.UnitySpy/ProcessFacade/ProcessFacade.cs:33-143`]

## 4. End-to-end flow

[V, with Mono struct names I]

```
MonoLibraryOffsets.GetOffsets(exe)           exe FileVersion (PeNet) + PE machine type  → table
UnityProcessFacade(facade, table)            sets facade.Is64Bits
AssemblyImageFactory.Create(upf, name)
  GetMonoModule()                            facade.GetModule("mono-2.0-bdwgc.dll")
  ReadModule(module)                         copy the whole DLL image out of the target
  GetRootDomainFunctionAddressPEFormat       export dir → "mono_get_root_domain"
  decode  mov rax,[rip+rel32]                → &mono_root_domain → MonoDomain*
  walk domain->domain_assemblies (GSList)    match MonoAssembly.aname.name == name
  new AssemblyImage(image*)
    walk image->class_cache hash table       → TypeDefinition per MonoClass
TypeDefinition.GetStaticValue(field)         runtime_info → domain vtable → static data + offset
FieldDefinition.GetValue(addr)               ReadManaged(type code) → value / instance / array
```

Detail is in [mono-discovery.md](mono-discovery.md) and [type-model.md](type-model.md).

## 5. Platform and runtime matrix

| Target | Backend(s) | Offsets available | Notes |
|---|---|---|---|
| Windows, x64 Mono, Unity 2019.4 / 2020.3 | `ProcessFacadeWindows` | `Unity2019_4_2020_3_x64_PE_Offsets` | [V] |
| Windows, x64 Mono, Unity 2021.3 / 2022.3 | `ProcessFacadeWindows` | `Unity2021_3_2022_3_x64_PE_Offsets` (2021.3.14, 2022.3.42) | The newest table. [V] |
| Windows, x86 Mono, Unity 2018.4 | `ProcessFacadeWindows` | `Unity2018_4_10_x86_PE_Offsets` | [V] |
| macOS, x64 Mono, Unity 2019.4 | `ProcessFacadeMacOSDirect` (root) or `…Client` | `Unity2019_4_2020_3_x64_MachO_Offsets` | No arm64 decode and no 32-bit. [V] |
| Linux | `ProcessFacadeLinuxDirect`, `…PTrace`, `…Dump`, `…Client` | **None for ELF** | `GetOffsets` only accepts a `.exe` path, or a macOS host (`MonoLibraryOffsets.cs:261-325`). In practice this means a Windows game under Wine/Proton, e.g. the GUI's default target `MTGA.exe`. [V for the code, I for the intent] |
| Any IL2CPP build | — | — | Unsupported. [V] |

Unity versions outside the tables silently fall back to the "closest" table when `force` is true, which is the default. See [offsets.md](offsets.md) §5.

## 6. Limitations at a glance

These are all [V] from source; the details are in the linked docs.
- **Float and double reads are broken.** `R4` and `R8` read only 2 bytes (`sizeof(char)`) (`ProcessFacade.cs:76-80`). See [type-model.md](type-model.md) §6.
- **`I1` and `U1` signedness is swapped**, and native `I`/`U` are read as 32-bit even in a 64-bit process (`ProcessFacade.cs:48-66`).
- **There are no collection helpers.** `List<T>`, `Dictionary<K,V>` and `HashSet<T>` must be read through their private BCL fields by hand.
- **Generic classes are not indexed by name** (names containing a backtick are skipped), and duplicate full names keep the first one found (`Detail/AssemblyImage.cs:35-49`).
- **Only classes already in the image's class cache are visible.** The whole cache is enumerated eagerly when the image is constructed (`Detail/AssemblyImage.cs:80-100`).
- **Values are never cached, and there is no snapshot.** Each field access is one or more `ReadProcessMemory` calls against a live, mutating heap.
- **Version detection is best-effort.** An unknown Unity version gets the nearest table with only a console warning.
- **The native server is unauthenticated** and listens on all interfaces, port 39185. Its module-info reply does not match what the client expects. See [process-access.md](process-access.md) §6.
- **The tests reference a missing project**, so the solution has no runnable tests.

## 7. Upstream versus the BazaarDBTracker fork

BazaarDBTracker ships its own `HackF5.UnitySpy.dll`, which was decompiled in [../bazaar-tracker/memory-reading.md](../bazaar-tracker/memory-reading.md). Comparing that doc with this source:

| Topic | Upstream (this tree) | Tracker's fork (per memory-reading.md) |
|---|---|---|
| Opening the process | `ProcessFacadeWindows` uses `System.Diagnostics.Process.Handle` (`ProcessFacadeWindows.cs:47-48`). .NET opens that handle with broad access rights [I]. | `OpenProcess(0x410 = PROCESS_VM_READ \| PROCESS_QUERY_INFORMATION)`. |
| x64 PE 2021/2022 table | `UnityVersions = {2021.3.14, 2022.3.42}` (`MonoLibraryOffsets.cs:88`). | Also lists `6000.0.58` (Unity 6). The offset values are the same. |
| String length | No bound. `length` is trusted (`ProcessFacade.cs:266-271`). | Throws on length < 0 or > 4,194,304. |
| Version selection | `GetOffsets` from the exe's FileVersion. | Never called. The tracker hard-codes the 2021/2022 x64 PE table. |
| Linux/macOS facades | Present. | Present but unused. |

The other mechanics (root-domain decode, class-cache walk, field dispatch, VALUETYPE int fallback) match between the two docs. [V for both sides as documented]

## 8. Relevance to our Rust tracker

The Bazaar is a Windows x64 Mono Unity game. The tracker's memory reader attaches with the 2021/2022 x64 PE table and looks up classes in the assembly `TheBazaarRuntime` (see [../bazaar-tracker/memory-reading.md](../bazaar-tracker/memory-reading.md) §1.1). Only a narrow slice of UnitySpy is needed to replicate that:
- the Windows reader;
- PE export lookup and the x64 root-domain decode;
- the assembly walk;
- class lookup;
- field resolution;
- the primitive, string, array and object readers;
- static fields;
- hand-written `List`/`Dictionary` readers.

The rest of the library is not needed for this: the GUIs, the Linux and macOS backends, the native server, Mach-O and x86. [rust-port.md](rust-port.md) lists the components, a module shape and the pitfalls.
