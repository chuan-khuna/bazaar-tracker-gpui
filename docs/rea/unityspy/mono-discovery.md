# UnitySpy: Mono runtime discovery

This doc covers how UnitySpy gets from a process handle to the list of classes in one managed assembly: `src/HackF5.UnitySpy/AssemblyImageFactory.cs` and `Detail/AssemblyImage.cs`. The numeric offsets are in [offsets.md](offsets.md); field and value reading is in [type-model.md](type-model.md).

How each claim is marked:
- **[V]**: verified from source.
- **[I]**: inferred. All Mono struct and member names are [I]: they are mapped from the Unity Mono headers that the source comments point to (`Unity-Technologies/mono`, `unity-master`: `metadata-internals.h`, `class-internals.h`, `domain-internals.h`). The source itself names only a few of them in comments.

## 1. Pipeline [V]

`AssemblyImageFactory.Create(process, assemblyName = "Assembly-CSharp")` (`AssemblyImageFactory.cs:20-40`) runs these steps:
1. `monoModule = process.GetMonoModule()`. This is `GetModule(offsets.MonoLibrary)`, which is `"mono-2.0-bdwgc.dll"` (PE) or `"libmonobdwgc-2.0.dylib"` (Mach-O).
2. Find the address of `mono_get_root_domain`:
   - If the **host** OS is macOS, it parses the dylib from disk (§3).
   - Otherwise it reads the whole module from the target and parses PE exports (§2).
   The branch tests `RuntimeInformation.IsOSPlatform(OSX)`, i.e. the OS UnitySpy runs on, **not** the target's binary format. A Linux host reading a Wine game takes the PE path, which is correct. A macOS host reading a dumped Windows game would wrongly take the Mach-O path. [V for the code; I for the effect]
3. `GetAssemblyImage`: decode the function to get `MonoDomain*`, walk the assemblies, and match the name (§4–5).
4. `new AssemblyImage(process, imageAddress)`: enumerate the class cache (§6).

## 2. PE export lookup [V]

`GetRootDomainFunctionAddressPEFormat(moduleDump, module, is64Bits)` (`AssemblyImageFactory.cs:123-157`) works on the **in-memory** copy of the DLL, so all offsets are RVAs:

```
e_lfanew     = i32 @ 0x3C
exportDirRva = i32 @ e_lfanew + (is64 ? 0x88 : 0x78)       // OptionalHeader.DataDirectory[0].VirtualAddress
numFuncs     = i32 @ exportDirRva + 0x14                     // NumberOfFunctions
funcsRva     = i32 @ exportDirRva + 0x1C                     // AddressOfFunctions
namesRva     = i32 @ exportDirRva + 0x20                     // AddressOfNames
for i in 0..numFuncs:
    name = cstr @ i32 @ namesRva + 4*i
    if name == "mono_get_root_domain":
        fn = module.Base + i32 @ funcsRva + 4*i
```

- It loops over `NumberOfFunctions` but indexes `AddressOfNames`. The proper bound is `NumberOfNames`.
- It uses the **name index as the function index** and skips `AddressOfNameOrdinals`. This is only correct when every export is named and ordinals are dense and sorted the same way as names [I]. It evidently holds for Unity's Mono builds, but a port should resolve through the ordinal table.
- If nothing matches, it throws `InvalidOperationException("Failed to find mono_get_root_domain function.")`.

## 3. Mach-O symbol lookup [V]

`GetRootDomainFunctionAddressMachOFormat` (`AssemblyImageFactory.cs:159-204`):
1. It reads the dylib **from disk** (`module.Path`) with `File.ReadAllBytes`.
2. It walks the load commands from file offset 0x20 (`mach_header_64` size), using `ncmds` at 0x10, until it finds `cmd == 2` (`LC_SYMTAB`).
3. It scans `nlist_64` entries (16 bytes each) for `"_mono_get_root_domain"`.
4. The address is `module.Base + i32 @ (symoff + j*16 + 8)`. This reads `n_value` as a **32-bit** int, so it only works while the symbol's offset is below 2 GiB, which is always true for a dylib [I].
5. It assumes a thin (non-fat) x86_64 Mach-O file. There is no `FAT_MAGIC` handling and no arm64 decode. [V by absence]

Constants are in `Offsets/MachOFormatOffsets.cs`.

## 4. Decoding the root domain [V]

`mono_get_root_domain()` simply returns the global `mono_root_domain` [I]. UnitySpy reads the global's address straight out of the instruction bytes (`AssemblyImageFactory.cs:42-104`):

| Target | Expected code | Formula | Constants |
|---|---|---|---|
| x64 PE | `48 8B 05 <rel32>` `C3` (`mov rax,[rip+rel32]; ret`) | `domain = ReadPtr(fn + i32@(fn+3) + 7)` | `ripPlusOffsetOffset = 3`, `ripValueOffset = 7` |
| x64 Mach-O | `55` `48 89 E5` `48 8B 05 <rel32>` `5D C3` (frame-pointer prologue) | `domain = ReadPtr(fn + i32@(fn+7) + 11)` | 7 / 11 |
| x86 PE | `A1 <abs32>` (`mov eax,[abs32]`) [I for the opcode] | `domain = ReadPtr(ReadPtr(fn + 1))` | — |

- The opcode bytes are **never checked**. A compiler or Unity build that emits a different sequence (e.g. a `jmp` thunk, a different register, or `/hotpatch` padding) yields a garbage pointer, and the failure only shows up later as a bad read. [V that there is no check; I for the scenarios]
- The comment at `:77-93` says the bytes were taken from a disassembly of `mono-2.0-bdwgc.dll`.

## 5. Assemblies [V]

```
list = ReadPtr(domain + ReferencedAssemblies)       // MonoDomain.domain_assemblies : GSList*   [I]
for node = list; node != 0; node = ReadPtr(node + ptr):   // GSList.next                  [I]
    asm  = ReadPtr(node)                            // GSList.data : MonoAssembly*           [I]
    name = cstr @ ReadPtr(asm + 2*ptr)              // MonoAssembly.aname.name               [I]
    if name == assemblyName:
        return new AssemblyImage(ReadPtr(asm + AssemblyImage))   // MonoAssembly.image    [I]
throw InvalidOperationException("Unable to find assembly '<name>'")
```

(`AssemblyImageFactory.cs:105-120`)

- The match is exact and case-sensitive. The name is the assembly's simple name with no extension: `Assembly-CSharp`, `TheBazaarRuntime`, `UnityEngine.CoreModule` and so on. [V; I for the example names other than the default]
- **Only one assembly is opened per `AssemblyImage`.** To read types from two assemblies (for example game code plus `mscorlib` for BCL types), call `Create` twice. Types from other images that are reached through pointers (field types, array element classes, runtime object classes) are still wrapped as `TypeDefinition`s inside the image that reached them, via `GetTypeDefinition(IntPtr)`. [V: `Detail/AssemblyImage.cs:68-78`]
- The `2*ptr` offset of `aname.name` is hard-coded rather than taken from the table. It assumes `MonoAssembly` begins with `int ref_count` (padded to a pointer) and `char* basedir`. [V for the constant; I for the layout]

## 6. Enumerating classes: `AssemblyImage` [V]

The constructor (`Detail/AssemblyImage.cs:23-50`) works in three stages.

**1. Walk the class cache** (`CreateTypeDefinitions`, `:80-100`):

```
cache = image + ImageClassCache                     // MonoImage.class_cache : MonoInternalHashTable  [I]
size  = u32 @ cache + HashTableSize                 // .size                                          [I]
table = ReadPtr(cache + HashTableTable)             // .table : gpointer*                             [I]
for i in 0..size:
    for klass = ReadPtr(table + i*ptr); klass != 0; klass = ReadPtr(klass + TypeDefinitionNextClassCache):
        definitions[klass] = new TypeDefinition(image, klass)     // MonoClassDef.next_class_cache [I]
```

Each `TypeDefinition` constructor immediately reads about ten values with one syscall each: flags, field count, name, namespace, size, runtime_info, the vtable, the `byval_arg` `MonoType`, vtable size and class kind. For a generic instance it reads more. See [type-model.md](type-model.md) §2.

**2. `Init()` every definition.** This forces `NestedIn` and `Parent` (recursively), which pulls in parent classes from other images through `GetTypeDefinition(IntPtr)`.

**3. Build the by-name index** `typeDefinitionsByFullName`:
- **Any full name containing a backtick (a generic class) is skipped**. The comment says generic names clash and would need `TypeInfo.Data` to tell apart (`:37-43`).
- On a duplicate full name, **the first one found wins**. Hash-bucket order decides which, so the choice is effectively arbitrary [I].

**Lookup.**
- `GetTypeDefinition(string fullName)` returns `null` (not an exception) when the name is unknown (`:65-66`).
- The full-name format is `Namespace.Outer+Inner`: the namespace of the outermost declaring type, then nested names joined with `+` (`Detail/TypeDefinition.cs:219-234`).
- A type in the global namespace has no prefix, e.g. `BoardManager`.

### 6.1 What the class cache does and does not contain

- `class_cache` is filled as Mono **loads** classes from the image's TypeDef table [I: Mono runtime behaviour]. A class that the game has not touched yet (no static access, no instance created) may be missing, and `GetTypeDefinition` returns `null` for it. The cache is read only once, at construction, so classes loaded later are not picked up unless the image is rebuilt. [V for the one-time read; I for the lazy loading]
- Generic instantiations such as `List<Card>` are `MonoClassGenericInst`s and are **not** stored in the image's `class_cache` [I]. UnitySpy only meets them through pointers (a field's runtime object, or `GENERICINST` field types).

## 7. Cost and threading [V]

- Attaching is O(number of loaded classes) multiplied by about 10 small `ReadProcessMemory` calls, plus one read of the whole Mono DLL for the export parse. For a large game that means thousands of syscalls before the first field is read [I for the magnitude].
- `typeDefinitionsByAddress` is a `ConcurrentDictionary`. `typeDefinitionsByFullName` is a plain `Dictionary` that is written only in the constructor. Per-type `Lazy<>` fields default to thread-safe initialisation. So concurrent reads from several threads work as far as UnitySpy's own caches are concerned. The process facades have no locking, apart from the TCP client and the dump reader. [V for the types; I for the overall thread-safety claim]

## 8. Failure modes

| Symptom | Cause |
|---|---|
| `ArgumentNullException` from `ReadModule` | Mono DLL not found. The game is IL2CPP, the DLL has a different name, or a different process was selected. [V: `GetModule` returns null] |
| `Failed to find mono_get_root_domain function.` | Export not found. A non-Mono or stripped runtime. [V] |
| `Unable to find assembly 'X'` | Wrong assembly name, or **wrong `ReferencedAssemblies` offset**. A bad domain pointer usually throws a `Win32Exception` (299 or 998) before this point. [V for the message; I for the causes] |
| Empty or garbage type names, or huge `size` values | Wrong `ImageClassCache` or `TypeDefinition*` offsets, i.e. the wrong table for this Unity version. [I] |
| `GetTypeDefinition` returns `null` | The name is generic, misspelled (`+` for nested types), in another assembly, or not loaded yet. [V/I] |
