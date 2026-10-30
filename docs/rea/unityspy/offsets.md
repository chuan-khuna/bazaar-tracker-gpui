# UnitySpy: Offsets and version selection

This doc covers the per-version Mono struct offsets in `src/HackF5.UnitySpy/Offsets/MonoLibraryOffsets.cs`, the other layout constants that are hard-coded outside that table, and how UnitySpy picks a table. It is the reference for building a `MonoOffsets` table in Rust.

How each claim is marked:
- **[V]**: verified from source. The hex values below were computed from the source expressions (many are written as sums like `0x84 + 0x34 + 0x18`).
- **[I]**: inferred. The Mono member names are [I], mapped from Unity's Mono headers. A few source comments name members, and those are noted.

## 1. The four tables [V]

| UnitySpy property | Mono member [I] | 2018.4.10 x86 PE | 2019.4 / 2020.3 x64 PE | **2021.3 / 2022.3 x64 PE** | 2019.4 x64 Mach-O |
|---|---|---|---|---|---|
| `MonoLibrary` | module file name | `mono-2.0-bdwgc.dll` | `mono-2.0-bdwgc.dll` | `mono-2.0-bdwgc.dll` | `libmonobdwgc-2.0.dylib` |
| `AssemblyImage` | `MonoAssembly.image` (source comment) | 0x44 | 0x60 | **0x60** | 0x60 |
| `ReferencedAssemblies` | `MonoDomain.domain_assemblies` (source comment) | 0x6C | 0xC8 | **0xA0** | 0xE0 |
| `ImageClassCache` | `MonoImage.class_cache` (source comment) | 0x354 | 0x4C0 | **0x4D0** | 0x4C0 |
| `HashTableSize` | `MonoInternalHashTable.size` | 0xC | 0x18 | **0x18** | 0x18 |
| `HashTableTable` | `MonoInternalHashTable.table` | 0x14 | 0x20 | **0x20** | 0x20 |
| `TypeDefinitionFieldSize` | `sizeof(MonoClassField)` | 0x10 | 0x20 | **0x20** | 0x20 |
| `TypeDefinitionBitFields` | `MonoClass` flag word (`valuetype` 0x4, `enumtype` 0x8) | 0x14 | 0x20 | **0x20** | 0x20 |
| `TypeDefinitionClassKind` | `MonoClass.class_kind` (byte, `& 7`) | 0x1E | 0x2A | **0x1B** | 0x24 |
| `TypeDefinitionParent` | `MonoClass.parent` | 0x20 | 0x30 | **0x30** | 0x28 |
| `TypeDefinitionNestedIn` | `MonoClass.nested_in` | 0x24 | 0x38 | **0x38** | 0x30 |
| `TypeDefinitionName` | `MonoClass.name` | 0x2C | 0x48 | **0x48** | 0x40 |
| `TypeDefinitionNamespace` | `MonoClass.name_space` | 0x30 | 0x50 | **0x50** | 0x48 |
| `TypeDefinitionVTableSize` | `MonoClass.vtable_size` | 0x38 | 0x5C | **0x5C** | 0x54 |
| `TypeDefinitionSize` | `MonoClass.sizes` (`instance_size` / `element_size` / `class_size`) | 0x5C | 0x90 | **0x90** | 0x88 |
| `TypeDefinitionFields` | `MonoClass.fields` | 0x60 | 0x98 | **0x98** | 0x90 |
| `TypeDefinitionByValArg` | `MonoClass._byval_arg` (inline `MonoType`) | 0x74 | 0xB8 | **0xB8** | 0xB0 |
| `TypeDefinitionRuntimeInfo` | `MonoClass.runtime_info` | 0x84 | 0xD0 | **0xD0** | 0xC8 |
| `TypeDefinitionMonoGenericClass` | `MonoClassGenericInst.generic_class` | 0x94 | 0xF0 | **0xF0** | 0xE8 |
| `TypeDefinitionFieldCount` | `MonoClassDef.field_count` | 0xA4 | 0x100 | **0x100** | 0xF8 |
| `TypeDefinitionNextClassCache` | `MonoClassDef.next_class_cache` | 0xA8 | 0x108 | **0x108** | 0x100 |
| `TypeDefinitionGenericContainer` | `MonoClassGtd.generic_container` | 0xAC | 0x110 | **0x110** | 0x108 |
| `TypeDefinitionRuntimeInfoDomainVTables` | `MonoClassRuntimeInfo.domain_vtables[0]` | 0x4 | 0x8 | **0x8** | 0x8 |
| `VTable` | `MonoVTable.vtable[]` (start of the method slots) | 0x28 | 0x40 | **0x48** | 0x40 |

Source locations:
- 2018 x86: `MonoLibraryOffsets.cs:12-47`
- 2019/2020 x64 PE: `:49-84`
- 2021/2022 x64 PE: `:86-146`
- Mach-O: `:148-183`

The registered list is `SupportedVersions` (`:185-191`).

### 1.1 What changed between 2019/2020 and 2021/2022 x64 PE [V]

Only four values differ:
- `ReferencedAssemblies`: 0xC8 → 0xA0
- `ImageClassCache`: 0x4C0 → 0x4D0
- `TypeDefinitionClassKind`: 0x2A → 0x1B
- `VTable`: 0x40 → 0x48

`MonoClass` itself barely moved between those versions. `MonoDomain`, `MonoImage` and `MonoVTable` did change. [V for the numbers; I for the struct attribution]

### 1.2 Source comments that disagree with the values [V]

In the 2021/2022 table:
- `TypeDefinitionFieldCount` is commented `// 0xE0`, but the expression gives **0x100** (`:133`).
- `TypeDefinitionNextClassCache` is commented `// 0xE4`, but the expression gives **0x108** (`:135`).

In the Mach-O table:
- `TypeDefinitionRuntimeInfo` is commented `// 0xD0`, but the expression gives **0xC8** (`:172`).

The **expressions** are what the code uses. The comments are stale.

The 2021/2022 table also has a comment explaining `VTable = 0x48`: "5 ptr + 8 byte (max_interface_id → gc_bits) + 8 bytes (4 + 4 padding) + 2 ptr" (`:142-145`). `RuntimeInfoDomainVTables = 0x2 + 0x6` is explained as a 2-byte `max_domain` plus alignment (`:140`).

## 2. Layout constants outside the table [V]

These are hard-coded in the readers. A new Mono version that changed them would need code changes, not just a new table.

| Constant | Where | Value | Meaning [I] |
|---|---|---|---|
| Assembly name | `AssemblyImageFactory.cs:112` | `asm + 2·ptr` → `char*` | `MonoAssembly.aname.name` |
| GSList node | `:107-111` | data at +0, next at +ptr | glib `GSList` |
| Root-domain decode | `:72-95` | x64 PE 3/7, x64 Mach-O 7/11, x86 `+1` absolute | the `mono_get_root_domain` body |
| Field entry | `Detail/FieldDefinition.cs:26-33` | `type*` +0, `name*` +ptr, `offset` (i32) +3·ptr | `MonoClassField` |
| MonoType | `Detail/TypeInfo.cs:18-19` | `data` +0, `attrs` (u32) +ptr; typecode = bits 16–23, static 0x10, literal 0x40 | `MonoType` |
| Object header | `ManagedClassInstance.cs:29-32`, `FieldDefinition.cs:83` | vtable* +0; header = 2·ptr | `MonoObject` |
| VTable → class | `ManagedClassInstance.cs:32` | `klass` at +0 | `MonoVTable.klass` |
| String | `ProcessFacade.cs:266-269` | length (i32) +2·ptr, chars +2·ptr+4 | `MonoString` |
| Array | `ProcessFacade.cs:205-206` | length (i32) +3·ptr, data +4·ptr | `MonoArray.max_length`, `vector` |
| Array element class | `ProcessFacade.cs:203` | `ReadPtr(arrayClass)` | `MonoClass.element_class` at +0 |
| Generic args | `FieldDefinition.cs:45-56` | `class_inst` at `generic_class + ptr`; `type_argc` at `container + 4·ptr`; `type_argv` at `inst + 8` | `MonoGenericClass`, `MonoGenericContainer`, `MonoGenericInst` |
| Generic param index | `ProcessFacade.cs:240` | `num` (i32) at `param + ptr` | `MonoGenericParam.num` |
| Class flags | `TypeDefinition.cs:98-100` | enum 0x8, valuetype 0x4 | `MonoClass` bitfields |
| PE export dir | `Offsets/PEFormatOffsets.cs` | `e_lfanew` 0x3C; DataDirectory[0] at +0x78 (PE32) / +0x88 (PE32+); `NumberOfFunctions` 0x14, `AddressOfFunctions` 0x1C, `AddressOfNames` 0x20 | PE/COFF spec |
| Mach-O | `Offsets/MachOFormatOffsets.cs` | `ncmds` 0x10, commands at 0x20, `cmdsize` +4, `symoff` +8, `nsyms` +0xC, `stroff` +0x10, nlist size 0x10, `n_value` +8 | `loader.h` |

## 3. `UnityVersion` [V]

`Offsets/UnityVersion.cs` defines `(Year, VersionWithinYear, SubversionWithinYear)` and five constants:
- 2018.4.10
- 2019.4.5
- 2020.3.13
- 2021.3.14
- 2022.3.42

`Parse(string)` splits the string on `.`, takes the first two parts as ints, and takes the leading digits of the third part. For example `"2022.3.42f1"` becomes 2022.3.42. The `f1`/`p2` suffix is dropped (`:31-41`).

## 4. Detecting the version: `GetOffsets(gameExecutableFilePath, force = true)` [V]

`MonoLibraryOffsets.cs:253-326` handles two cases.

**Path ends with `.exe`** (Windows, or a Wine game seen from Linux):
1. `new PeNet.PeFile(path).Resources.VsVersionInfo.StringFileInfo.StringTable[0].FileVersion`. Unity stamps the player exe's FileVersion with the editor version, e.g. `2022.3.42f1` [I].
2. It reads the PE machine type from the file. `0x8664` means 64-bit and `0x14C` means 32-bit. Any other value falls through to `NotSupportedException`.
3. It prints `Game executable file closed, unity version = …` to the console.

**Host is macOS:**
1. It reads `<exe>/../../Info.plist` and takes the token after `"Unity Player version "`.
2. It runs the `file` command on the exe. 64-bit means the output ends with `"x86_64\n"`. A universal or arm64 binary is therefore treated as 32-bit and finds no table [I].

**Anything else** throws `NotSupportedException("Platform not supported")`. This includes a Linux-native ELF game.

## 5. Picking a table and the `force` fallback [V]

`GetOffsets(UnityVersion, is64Bits, format, force)` (`:333-409`):
1. It looks for an exact match: same bitness, same format, and the version listed in `UnityVersions`.
2. If there is no match and `force` is true (**the default**):
   - It keeps the tables with the same bitness and format.
   - For each one, `GetBestCandidate` picks the nearest listed version, comparing year distance first, then minor, then patch (`:411-461`).
   - Across tables it keeps the candidate with the smallest (year, minor, patch) distance tuple.
   - It prints `The unity version the process is running (<v> in 64 bits mode) is not supported.` and `Offsets of <v> selected instead.`, then returns that table.
3. If there is no match and `force` is false, or no table has the right architecture, it throws `NotSupportedException`.

For a 64-bit PE game this resolves as follows:
- Any 2021.x or 2022.x version gets the 2021/2022 table.
- 2019.x and 2020.x get the 2019/2020 table.
- **Unity 6 (6000.x) is "closest" to 2022.3.42 by year distance**, so it gets the 2021/2022 table too.

The BazaarDBTracker fork lists 6000.0.58 in that table explicitly and skips `GetOffsets` entirely (overview §7). Whether those offsets are actually right for Unity 6 is not established by this source. [V for the selection logic; I for correctness]

The fallback is **silent at the API level**. Only `Console.WriteLine` reports it, so a GUI or library caller cannot tell that it got a guessed table. A wrong table usually fails later, when the assembly or class walk reads garbage. [V]

## 6. Adding or checking a table [I]

The source gives no tooling for deriving offsets. The commit history shows tables were added by hand, one PR per version (e.g. `71bfd85 Add Unity 2022.3.42f1 x64 offsets`). Practical approaches:
- **From headers.** Compile the Unity Mono fork's `class-internals.h` / `metadata-internals.h` / `domain-internals.h` for the target (`unity-<version>` branch of `Unity-Technologies/mono`) and print `offsetof(...)` for each member listed in §1.
- **Empirically.** Attach the Avalonia GUI's raw memory viewer (see [tooling.md](tooling.md)) to the game:
  1. Start from a `MonoClass*` whose name you know.
  2. Find the `name`/`name_space` pointers, the `fields` array, and `runtime_info`.
  3. Confirm the domain offset by finding a `GSList` whose `data → MonoAssembly → aname.name` is `"Assembly-CSharp"`.
- **Self-check at attach time**, which a Rust port can automate:
  - after resolving the image, assert that the class cache yields a well-known class such as `System.Object` (in `mscorlib`) or a known game class;
  - assert that a known field (e.g. `List<T>._size`) reads back at a sane offset.
