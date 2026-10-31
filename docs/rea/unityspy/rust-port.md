# UnitySpy: Porting notes for the Rust tracker

This doc covers what our Rust/GPUI tracker would need to reimplement from UnitySpy to read The Bazaar's game state from memory, and what to watch out for. It builds on the other docs in this folder and on [../bazaar-tracker/memory-reading.md](../bazaar-tracker/memory-reading.md), which records the exact classes and field paths the existing tracker reads.

How each claim is marked:
- **[V]**: verified from the UnitySpy source, or from the tracker docs where cited.
- **[I]**: inferred, or a design suggestion.

## 1. Is UnitySpy's approach applicable to The Bazaar?

- **Probably yes.** The existing tracker attaches to `TheBazaar` with the upstream-style Windows facade and the **2021.3/2022.3 x64 PE** offset table, and opens the assembly `TheBazaarRuntime` (memory-reading.md §1.1). [V per that doc]
- The game ships a `<exe>_Data\Managed\` folder, which points to a **Mono** build, not IL2CPP (bazaar-tracker overview §5.1). [I per that doc]
- **The game's exact Unity version is not known from anything in this repo.** The fork's table also lists Unity 6000.0.58, which suggests the game is on Unity 6 [I]. Upstream UnitySpy has **no Unity 6 table**; with `force=true` it would fall back to the 2021/2022 table (offsets.md §5). Our port must confirm the offsets against the live game rather than trust either library.
- If the game ever switches to IL2CPP, none of this applies. The port would need an IL2CPP reader instead (`GameAssembly.dll`, `global-metadata.dat`, `Il2CppClass` layouts). [V that UnitySpy has no IL2CPP path]

## 2. Minimum component list

Everything the tracker needs, mapped to its UnitySpy source and a suggested Rust module [I for the module names]:

| # | Component | UnitySpy source | Rust module | Notes |
|---|---|---|---|---|
| 1 | Find the process by name, open a handle | GUI code / `Process` | `mem::process` | `windows` crate: `CreateToolhelp32Snapshot` + `OpenProcess(PROCESS_VM_READ \| PROCESS_QUERY_LIMITED_INFORMATION)`. |
| 2 | Raw reads | `ProcessFacadeWindows.ReadProcessMemory` | `mem::reader` | `trait Memory { fn read(&self, addr: u64, buf: &mut [u8]) -> Result<()> }`, plus typed helpers. Handle bulk reads. |
| 3 | Module lookup | `ProcessFacadeWindows.GetModule` | `mem::module` | `EnumProcessModulesEx(LIST_MODULES_ALL)` + `GetModuleBaseNameW` + `GetModuleInformation`. Compare names case-insensitively. |
| 4 | Export lookup | `GetRootDomainFunctionAddressPEFormat` | `mono::pe` | Read only the headers and the export directory, not the whole DLL. **Use `AddressOfNameOrdinals`.** |
| 5 | Root-domain decode | `GetAssemblyImage` (x64 branch) | `mono::domain` | Check `48 8B 05` and `C3` before trusting `rel32`. |
| 6 | Offset table | `MonoLibraryOffsets` | `mono::offsets` | A `struct MonoOffsets` per Unity line. Start with the 2021/2022 x64 PE values (offsets.md §1) and select by the exe's `FileVersion`. |
| 7 | Assembly walk | `GetAssemblyImage` loop | `mono::domain` | GSList walk and name match on `TheBazaarRuntime`. Also keep `mscorlib` available if BCL types need resolving. |
| 8 | Class lookup | `AssemblyImage.CreateTypeDefinitions` | `mono::class` | **Lazy**: walk the class cache hash table comparing name/namespace, and stop once the classes we need are found. Cache by full name. |
| 9 | Class model | `TypeDefinition` | `mono::class` | name, namespace, parent, nested_in, flags, `sizes`, fields array, field count, runtime_info → vtable, vtable_size, class_kind, generic_class. Read the struct in one call. |
| 10 | Field model | `FieldDefinition`, `TypeInfo` | `mono::field` | `{name, type_code, attrs, offset, type_data}`. Search the parent chain. Support a `declaring_type` filter. |
| 11 | Static fields | `TypeDefinition.GetStaticValue` | `mono::statics` | `vtable + VTable + vtable_size*8`. Report a null vtable as **NotInitialized**. |
| 12 | Object header | `ManagedClassInstance` | `mono::object` | `vtable = *obj`, `klass = *vtable`. Resolve fields on the runtime class. |
| 13 | Value readers | `ProcessFacade.ReadManaged` | `mono::value` | Primitives of the **correct widths** (§3), string, `T[]`, object ref, inline struct (offset − 16), enum (`value__`). |
| 14 | Collections | not in UnitySpy | `mono::bcl` | `List<T>` (`_size`, `_items`), `Dictionary<K,V>` (`_count`/`count`, `_entries`/`entries`, skip `hashCode < 0`), `HashSet<T>`, `Nullable<T>`. Resolve field offsets by name from the class, then read elements at computed strides. |
| 15 | Game layer | the tracker's `MemoryReader` | `game::*` | The field paths in memory-reading.md §2. Out of scope for this doc. |

A suggested Rust shape [I]:

```rust
pub struct Mono<M: Memory> {
    mem: M,
    off: &'static MonoOffsets,
    image: u64,                                    // MonoImage* of TheBazaarRuntime
    classes: HashMap<String, Class>,               // lazily filled
}
pub struct Class { addr: u64, name: String, ns: String, parent: u64, flags: u32,
                   size: i32, vtable: u64, vtable_size: i32, fields: Vec<Field> }
pub struct Field { name: String, type_code: u8, attrs: u32, offset: i32, type_data: u64 }
pub enum Value { Null, Bool(bool), I32(i32), I64(i64), F32(f32), F64(f64),
                 Str(String), Obj(u64), Struct { class: u64, addr: u64 }, Array(Vec<Value>) }
```

Return **addresses**, not eagerly materialised objects. Read values only when a field is asked for. This avoids UnitySpy's eager array materialisation (type-model.md §8).

## 3. UnitySpy bugs not to copy [V]

1. **`R4`/`R8` read 2 bytes** (`ProcessFacade.cs:76-80`). Read 4 and 8 bytes.
2. **`I1`/`U1` are swapped** (`:48-52`). `I1` is `i8` and `U1` is `u8`.
3. **`I`/`U` are read as 32-bit** (`:60-66`). On x64 they are 8 bytes.
4. **The `VALUETYPE` failure path silently returns an int32** (`:88-96`). Surface the error instead.
5. **Strings have no length cap** (`:266-271`). Cap the length (the fork uses 4,194,304 UTF-16 units) and reject negative values.
6. **Export lookup ignores ordinals** (`AssemblyImageFactory.cs:136-149`).
7. **Root-domain bytes are not checked** (`:95`).
8. **`ReadAsciiString` reads a fixed 1,024 bytes** (`ProcessFacade.cs:21-22`). It can fail at a region boundary.
9. **`GetModule` returns `null` when the module is missing** (`ProcessFacadeWindows.cs:120`). The result is an unhelpful error later.
10. **`TypeDefinition.GenericTypeArguments` is wrong** (`Detail/TypeDefinition.cs:85`). It only matters if it is ported.
11. **`ReadManagedVar` offset heuristic** (`ProcessFacade.cs:237-250`). Prefer computing struct field offsets from the generic instance's own `MonoClass` (its fields are laid out for the concrete arguments) over adjusting the generic definition's offsets [I].

## 4. Mono runtime pitfalls [I unless marked]

- **Offsets are per Unity version.** Detect the version from the game exe's `FileVersion` (the Rust `pelite` crate or a small resource parser), and refuse to read, or run a self-check, when it is unknown. Do not silently fall back as UnitySpy does (offsets.md §5). [V for UnitySpy's behaviour]
- **Classes may be absent until the game touches them.** The class cache only holds loaded classes. Retry the lookup on a later tick instead of failing permanently. UnitySpy reads the cache only once. [V for the one-time read]
- **Statics need an initialised class.** A null `runtime_info` or vtable means "not ready yet", not an error.
- **The Bazaar's game types live in `TheBazaarRuntime`, not in `Assembly-CSharp`.** That is UnitySpy's default, and the upstream GUIs hard-code it. [V for the default; V per the tracker docs for the assembly name]
- **Generic classes are not found by name.** Reach `List<T>` and friends through fields, never through `GetTypeDefinition("…`1")`. UnitySpy explicitly skips such names. [V]
- **Auto-property storage** is named `<Prop>k__BackingField`. UnitySpy matches field names exactly, so request the full compiler name. Whether the fork matches `<Prop>` partially is not visible in upstream code.
- **Value-type field offsets include the 16-byte object header.** Subtract 16 when reading a struct that is stored inline (in an array, a field, or a dictionary entry). Do not subtract when reading a boxed struct through an object pointer. [V for UnitySpy's subtraction]
- **There are no consistent snapshots.** Reads race the game's main thread. For lists, re-read `_size` after reading the items and retry on a mismatch, or accept occasional torn reads. Validate pointers, for example a non-zero vtable whose `klass` resolves to the expected class name, before following them. Mono's Boehm GC does not move objects, but freed memory can be reused.
- **BCL field names depend on the Mono profile** Unity ships (e.g. `_entries` versus `entries`). Look them up by name from the class at runtime.
- **Syscall volume.** Each `ReadProcessMemory` is a kernel transition. Read whole structs and arrays in one call and decode them locally. Cache class and field metadata for the life of the attach, and drop the cache when the process exits or restarts.

## 5. Windows access pitfalls [I]

- **Handle rights.** Request only `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION`. Upstream's `Process.Handle` asks for much more (process-access.md §3), and the BazaarDBTracker resorts to always relaunching as admin. That should not be necessary for a same-user, same-integrity Steam game. Elevate only if `OpenProcess` fails with access denied.
- **Anti-cheat.** Nothing in either reference shows protection on The Bazaar's process. If a launcher or anti-cheat strips handle rights later, reads will fail with `ERROR_ACCESS_DENIED` or `ERROR_PARTIAL_COPY`. Detect that and report it to the user instead of retrying in a tight loop.
- **Process lifetime.** A game exit makes reads fail with `ERROR_PARTIAL_COPY`/`ERROR_INVALID_HANDLE`. Model that as `ProcessGone` and reconnect, as the existing tracker does (memory-reading.md §4).
- **Bitness.** The tracker is a 64-bit Rust binary reading a 64-bit game, so no WOW64 handling is needed. UnitySpy's 32-bit paths can be dropped.

## 6. What we do **not** need from UnitySpy [I]

- The GUIs, `MemoryReadingUtils`, dump-file mode and the TCP server.
- The Linux and macOS backends, Mach-O symbol lookup and the x86 table.
- `dynamic` and indexer sugar. In Rust, use typed accessors keyed by field name, resolved once and cached as offsets.
- PeNet. Version detection needs only the exe's `VS_VERSIONINFO`.

## 7. Suggested verification steps [I]

1. Attach, resolve `mono_get_root_domain`, and confirm that the domain's assembly list contains `TheBazaarRuntime`, `mscorlib` and `UnityEngine.CoreModule`.
2. Look up `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache`, the four classes the existing tracker requires (memory-reading.md §1.1). Dump their fields with offsets and type codes, and check them against the field paths in memory-reading.md §2.
3. Read one known static (e.g. the `BoardManager` singleton through its base class) and one `List`/`Dictionary` count. Cross-check them against the game UI, or against UnitySpy's Avalonia GUI pointed at `TheBazaarRuntime` (tooling.md §7).
4. Keep a small debug command in the Rust app that prints a class's fields. It replaces the UnitySpy browser when a game patch moves things.
