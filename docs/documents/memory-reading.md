# Memory Reading

How the app reads The Bazaar's state out of the running game: the generic Mono reader in `crates/unityspy-rs`, and the Bazaar layer on top of it in `crates/bazaar-tracker/src/memory/`. What a Snapshot, Board and Stash are is in [GLOSSARY.md](../../GLOSSARY.md); the module tour is in [modules.md](modules.md).

## Layers

```
TheBazaar.exe (Unity 6, Mono)
   │  ReadProcessMemory, read-only
   ▼
unityspy-rs   mem::process ─ mem::module ─ mono::pe ─ mono::domain ─ mono::class/field/statics/object/value/bcl
   │  Mono<Process>: classes, fields, statics, objects, values
   ▼
bazaar-tracker::memory   BazaarMemory (implements GameMemory) ─► BoardState, RunInit, CombatResult, ...
   │
   ▼
run::RunMachine and the tracker service (see run-state-machine.md)
```

`unityspy-rs` knows nothing about The Bazaar and compiles to an empty crate on anything but Windows (`#![cfg(windows)]`). `memory::game` is also Windows only; `memory::fake` and the trait build everywhere.

## `unityspy-rs`

### Process access

| Part | What it does |
|---|---|
| `Process::open_by_name` | Walks a Toolhelp snapshot for the first process whose exe matches the name, ignoring case and a `.exe` (`TheBazaar` matches `TheBazaar.exe`, `TheBazaarLauncher.exe` does not). No match is `Error::ProcessNotFound(name)`. |
| `Process::open` | `OpenProcess` with `PROCESS_VM_READ \| PROCESS_QUERY_LIMITED_INFORMATION` only. A refusal is `Error::OpenProcess { pid, code }` with the Windows error. The crate never writes and never asks for admin rights. The handle closes on drop. |
| `Process::is_running` | `GetExitCodeProcess` is `STILL_ACTIVE` (259). |
| `Process::image_path` | `QueryFullProcessImageNameW`; failure is `Error::ProcessGone`. |
| `file_version` | The exe's `VS_FIXEDFILEINFO` version (`a.b.c.d`). Unity stamps the player exe with the editor version, so 6000.3.11.239 reads as Unity 6000.3.11. |
| `Memory for Process` | One `ReadProcessMemory` per read. A partial read is an error (`UnexpectedEof`, "partial read: 4 of 8 bytes"). |
| `mem::module` | `K32EnumProcessModulesEx` (all modules, buffer starting at 1024 handles and grown as needed), then name, base and size of each. `find_module` matches the file name ignoring case; a miss is `Error::ModuleNotFound`. |

### The `Memory` trait

`mem::reader::Memory` has one method, `read(address, buffer)`. Every Mono structure is read through it, so tests hand the reader byte buffers instead of a game. `MemoryExt` adds typed little-endian reads on top:

- `read_u8` … `read_i64`, `read_f32` (4 bytes) and `read_f64` (8 bytes), `read_ptr` (8 bytes: the game is 64-bit).
- `read_c_string` reads NUL-terminated names in chunks of at most 256 bytes that never cross a 4 KiB page, so a name at the end of a mapped region still reads. Past 1,024 bytes with no NUL it is `Error::Implausible`.
- A failed read becomes `Error::Read { address, len, source }`.

`BufferMemory` is the test address space: pages of 4 KiB are mapped as they are written, unwritten bytes in a mapped page read as zero, and a read outside a mapped page fails, as in a real process.

### Attaching: `Mono::attach`

```
image_path ─► file_version ─► UnityVersion ─► offsets::for_version
find_module("mono-2.0-bdwgc.dll") ─► pe::find_export("mono_get_root_domain") ─► domain::root_domain
```

It returns the `Mono<Process>` and an `AttachInfo` (pid, Mono module, file version, Unity version).

- **Export lookup (`mono::pe`).** Reads only the MZ and PE headers and the export directory, not the whole image. It accepts PE32+ (`0x20B`) and PE32 (`0x10B`), refuses more than 0x10000 names as implausible, and goes from name index to `AddressOfNameOrdinals` to the function, so it is right when a name's position differs from its ordinal. A forwarded export is `Error::BadPe`.
- **Root domain (`mono::domain`).** `mono_get_root_domain` must start `48 8B 05 <rel32> C3` (`mov rax,[rip+rel32]; ret`). The bytes are checked before the `rel32` is trusted; anything else is `Error::UnexpectedRootDomainCode`. The global is at function + 7 + rel32; a null there is `Error::RootDomainNull` (Mono has not made it yet).
- **Assemblies.** The `GSList` at `MonoDomain.domain_assemblies`; each name is read from `MonoAssembly + 16`, the image from `assembly_image`. More than 4,096 nodes is `Error::Loop`. `find_assembly` matches the name exactly (`TheBazaarRuntime`).

### Offsets per Unity version

`mono::offsets` holds one `MonoOffsets` table per Unity line, 64-bit Windows only. The table is picked from the exe's major version and never guessed:

| Unity major | Table | `verified` |
|---|---|---|
| 2021, 2022 | `UNITY_2021_2022_X64` ("Unity 2021.3/2022.3 x64") | `true` |
| 6000 and up | `UNITY_6_X64` ("Unity 6 x64 (unverified)"), the same numbers | `false` |
| anything else | `Error::UnsupportedUnityVersion` | — |

Some of the numbers: `domain_assemblies` 0xA0, `assembly_image` 0x60, `image_class_cache` 0x4D0, `class_name` 0x48, `class_fields` 0x98, `class_runtime_info` 0xD0, `class_generic_class` 0xF0, `generic_class_cached_class` 0x20, `class_next_class_cache` 0x108, `vtable` 0x48. The game today is Unity 6000.3.11, so it runs on the Unity 6 table; records `2026-10-31.001` to `.003` in `docs/records/` checked it against the live game.

### Classes and fields

- **Class lookup.** `Mono::find_class(image, full_name)` takes `Namespace.Name`, a bare `Name` for the global namespace, or `Outer+Inner` for nested classes. It walks the image's class cache (a hash table: `size`, `table`, then the `next_class_cache` chain) only until the class is found, and remembers `(image, full name)` → address. A miss is `Error::ClassNotFound`. Guards: at most 2^24 buckets, 1,000,000 classes, 16 levels of nesting.
- **Class model.** `class_at` reads the whole `MonoClass` in one read and caches it by address: name, namespace, full name, parent, nesting, flags (`valuetype` 0x4, `enumtype` 0x8), size, kind (`Def`, `GenericDefinition`, `GenericInstance`, …), vtable (0 while the runtime has not initialised the class), `_byval_arg`, and its own fields. A generic instance takes its field count (and fields, if it has none) from the definition.
- **Fields.** Each `MonoClassField` gives name, `MonoType` (type code in bits 16–23 of `attrs`, `static` 0x10, `literal` 0x40) and offset. Offsets include the 16-byte object header, even for structs. `find_field` climbs the parent chain, nearest first (at most 64 levels); `declaring_type` limits it to one class, which reaches a hidden field. A miss is `Error::FieldNotFound { class, field }`. More than 8,192 fields is implausible.
- The caches live inside the `Mono`, so a new attach after a game restart starts clean.

### Statics

`read_static(class, name)` finds the field (it may be declared by a parent), refuses an instance field or a `const` (`Error::NotAStaticField`), and reads it from the declaring class's static block: the pointer at `vtable + 0x48 + vtable_size * 8`. No vtable, or a null block, is `Error::NotInitialised`. A class cached before the runtime initialised it is looked up again in the live class, so it becomes readable later. This is how `BoardManager.Instance` is reached through its `Singleton<BoardManager>` base class.

### Objects and values

- `object_at(address)` follows `vtable = *object; class = *vtable`, so fields resolve on the runtime class, not the declared one. A null vtable or class is `Error::Unexpected`.
- `read_field(object, name)` reads at `object + offset`; `read_struct_field` reads an inline struct at `address + offset − 16`.
- `read_value` returns a `Value`:

| Type code | Read as |
|---|---|
| `BOOLEAN`, `CHAR` | 1 byte ≠ 0, `u16` |
| `I1` / `U1` … `I8` / `U8` | Their own widths and signs (`I1` signed, `U1` unsigned; UnitySpy had them swapped) |
| `I`, `U` | 8 bytes (`IntPtr`, `UIntPtr`) |
| `R4`, `R8` | 4 and 8 bytes |
| `STRING` | `length` at +16, UTF-16 from +20; length outside 0..=4,194,304 is implausible; null → `Value::Null` |
| `CLASS`, `OBJECT` | `Value::Object` or `Null` |
| `SZARRAY` | `Value::Array` (length at +24, data at +32, at most 2^24 elements); elements are read only by `array_items` |
| `GENERICINST` | The instantiated class from `MonoGenericClass.cached_class` (trusted only if it points back), else the definition; a value type is an inline `Struct` |
| `VALUETYPE` | An enum reads as its `value__`; any other struct is `Value::Struct` |
| `VAR` | `Error::UnresolvedGeneric`: a generic parameter has no concrete type |
| anything else | `Error::UnsupportedType(code)` |

Nothing is converted silently: a struct that cannot be built is an error, never an int.

### BCL collections

`mono::bcl` reads .NET collections with field offsets looked up by name, trying each spelling a Mono profile might use. Counts above 2^20 are implausible.

| Method | Fields tried | Gives |
|---|---|---|
| `read_list` | `_size`/`size`, `_items`/`items` | The first `_size` items |
| `read_dictionary` | `_count`/`count`, `_entries`/`entries`; entry `hashCode`/`hash_code`, `key`, `value` | `(key, value)` pairs, skipping entries with a negative hash code |
| `read_hash_set` | `_count`/`m_count`/`count`, `_slots`/`m_slots`/`_entries`/`entries`; `value`/`Value` | Values, skipping free slots |
| `read_nullable` | `hasValue`/`has_value`, `value` | `Some(value)` or `None` |

Entries are read with the array's element class, which for a generic struct is the instantiated class with concrete field types.

### Errors

`error::Error` is the one error type. Every missing process, module, export, assembly, class or field is its own variant and names what was missing, for example `field 'key' of 'System.Collections.Generic.Dictionary`2+Entry' has a generic parameter type, not a concrete one`.

## `GameMemory`: the contract

The run state machine reads the game only through `memory::GameMemory` (`Send`), one method per read:

| Method | Returns |
|---|---|
| `read_board` | `BoardState`: both sides' Snapshots, the run state, what is on offer, every card seen |
| `read_run_init` | `RunInit`: real run id, play mode, build id, environment |
| `read_combat_result` | `CombatResult`: winner and loser combatant id (0 player, 1 opponent), frame count |
| `read_combat_frames` | `CombatFrames`: the frame count of the last combat simulation, to tell a stale one from the one just played |
| `read_pvp_opponent` | `PvpOpponent`: name, hero, rating, level |
| `read_selected_hero` | The hero's name |
| `read_player_rank` | `PlayerRank`: rating, rank name, division |

Every read is a `MemoryResult<T> = Result<Option<T>, MemoryError>`:

- `Ok(None)`: the game has no value yet (no run, a class not initialised, a null reference). Normal, never an error, never detaches.
- `Ok(Some(..))`: the value.
- `Err(MemoryError::Disconnected)`: the game is not attached.
- `Err(MemoryError::Failed(why))`: attached, but the read went wrong.

`ConnectionChange` (`Connected { pid, unity_version, details }` or `Disconnected { reason }`) reports attach and detach; `unity_version` is the offset table's name and `AttachDetails` carries the process name, Unity version, table, `offsets_verified`, Mono module, its base and the root domain.

## `BazaarMemory`

`memory::game::BazaarMemory` is the real `GameMemory`. Every clone shares one connection behind one lock, so all memory access is serialised.

### Attach, reconnect, detach

| Constant | Value |
|---|---|
| `PROCESS_NAME` | `TheBazaar` |
| `GAME_ASSEMBLY` | `TheBazaarRuntime` |
| `RECONNECT_INTERVAL` | 500 ms |
| `MAX_BOARD_FAILURES` | 5 |
| `OPTIONAL_CLASS_RETRY` | 5 s |

- **`tick`** is one step. Detached: open the process, `Mono::attach`, find `TheBazaarRuntime` and the class `TheBazaar.Data`, then look for the optional classes. Success resets the failure count and sends `Connected`. A failure is logged at `info` once and not again until its message changes. Attached: if the process is no longer running, detach with "the game process exited".
- **`run_connection_loop`** calls `tick` every 500 ms until a stop flag is set, sleeping in 25 ms steps so a stop is noticed quickly. The tracker runs it on the `tracker-game` thread.
- **`TheBazaar.Data` is required.** If the game has not loaded it yet the attempt fails and the next tick tries again.
- **Optional classes** (`TheBazaar.AppState`, `BoardManager`, `TheBazaar.ClientCache`) may be missing; before each read the missing ones are looked for again, at most every 5 s.
- **Each read** (`with_game`): no connection is `Disconnected`. `Error::NotInitialised` becomes `Ok(None)`. An error while the process has died detaches and returns `Disconnected`. Only `read_board` counts towards the five-strikes rule: five failed board reads in a row detach with "five board reads in a row failed". A board read that got a board or `Ok(None)` resets the count.
- Detaching drops the `Mono` (and its class caches), resets the count and sends `Disconnected { reason }`. The next tick attaches afresh.

### What it reads

Auto-properties are read through their compiler-made storage, `<Name>k__BackingField` (`prop("IsStorageOpen")`), with the bare name as a fallback.

| Read | Path |
|---|---|
| Board | `TheBazaar.Data` statics `<CurrentState>` and `<Run>` (either null → `Ok(None)`); `CurrentState.StateName` and `SelectionSet` (a `List<string>`); static `<IsStorageOpen>` and `<CurrentEncounterId>` (a `Nullable<Guid>`); `Run.Player` and `Run.Opponent`; the static `Entities` dictionary of every card |
| Player stats | `Player.Attributes` dictionary (`Health`, `HealthMax`, `Gold`, `Prestige`, `Income`, `Level`), `Player.Hero` (0 is none); `Run.Day`, `Hour`, `Victories`, `Losses` |
| Skill order | `Player.Skills` and `Opponent.Skills` lists, by instance id |
| Run init | `AppState._cachedRunInitialized` (`RunId`, `PlayMode`, `BuildId`, `Environment`); if that has no id, the run object's `<Id>`, `<RunId>`, `Id`, `RunId` or `_id` |
| Combat result, frames | `BoardManager.Instance._lastCombatSim`, else `AppState._cachedCombatSim.Data`; `Winner`, `Loser`, and `Frames._size` |
| PvP opponent | `TheBazaar.Data.<SimPvpOpponent>`: `Name`, `Hero`, `Rating`, `Level` |
| Selected hero | `Run.Player.Hero` if above 0, else `ClientCache.RunConfig._value.<Hero>` |
| Rank | `ClientCache.Rank` (an observable): the field offsets of `SeasonID`, `Rank`, `Division`, `Rating` from the `_value` struct's class, then a scan of starts 16 to 48 in steps of 4 for the first record that is plausible (season 1–10,000, rank 0–16, division 1–10, rating 0–1,000,000) |

Each card in `Entities` is read for `<TemplateId>`, `<InstanceId>` (a `System.Guid` is formatted as text; another struct is read through its `Value` field), `Type`, `Size`, `Tier`, `<LeftSocketId>` and `<Section>` (inline nullables: a flag byte, then a 32-bit value at +4), `<Enchantment>` (may be absent), the `Attributes` dictionary, the `Tags` set and `Owner.CombatantId`. A dictionary or set whose generic types do not resolve is read at the original tracker's layout (hash at 0, key at 8, value at 12; set value at 8). Numbers become the game's enum names; a number with no name is kept as its digits.

Cards are sorted into the two sides:

| Owner | Card | Goes to |
|---|---|---|
| Player (0) | Skill (type 1) | `player.skills` |
| Player | Section Stash (1) | `player.stash` |
| Player | anything else | `player.board` |
| Opponent (1) | Skill | `opponent.skills` |
| Opponent | Item (0), EncounterStep (3), SocketEffect (7), or the current encounter's template (once per template) | `opponent.board`, if its section is Hand (0) or none |

Other encounter cards are leftovers and are skipped. Every card read, kept or not, also goes into `BoardState.entities`. Skills are sorted into the game's order; unknown ones go last.

### `Snapshot`, `BoardState`, `Item`, `PlayerStats`

| Type | What it holds |
|---|---|
| `Item` | `instance_id` (such as `itm_EuUtMwz`; the prefix says what kind of card), `template_id`, `card_type`, `tier`, `size` (each `Unknown` when unread), `socket`, `enchantment`, `attributes` (name → `i32`, such as `DamageAmount` → 30), `tags` |
| `PlayerStats` | `health`, `health_max`, `gold`, `prestige`, `level`, `income`, `day`, `hour`, `victories`, `losses`, `hero`; each `None` until the game sets it |
| `Snapshot` | `board`, `stash`, `skills`, `stats` of one side. Serialised with each Event |
| `BoardState` | `player` and `opponent` Snapshots, `run_state` (`Choice`, `Combat`, `EndRunVictory`, …), `selection_set` (ids on offer), `is_storage_open`, `entities`. `entity(id)` finds a card; `player_item_ids()` lists board, stash and skills |

## `FakeMemory`

`memory::fake::FakeMemory` is a scriptable `GameMemory` for tests of the machine, the tracker service and anything else that reads the game. Each read has its own script:

```rust
let memory = FakeMemory::new();
memory.push_board(board_with_gold(1)); // next call
memory.push_board(None);               // then "not available yet"
memory.set_selected_hero("Jules".to_owned()); // every call from now on
memory.fail_board("boom");             // Err(Failed("boom")) from now on
memory.board_calls();                  // how many times it was read
```

Pushed results come in order, then the last repeats. A read with no script is `Ok(None)`. Clones share the scripts, so a test can change what the game says after handing the memory over.

## Checking against the live game

| Command | What it does |
|---|---|
| `just app spike [process]` | `unityspy-rs/examples/dump-classes.rs`. Attaches (default `TheBazaar`), prints pid, exe and Unity version, the offset table (flagged when unverified), the Mono module and root domain, every assembly, then every field (own and inherited, offset, type code, `static`/`const`) of `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache`. Last it reads `BoardManager.Instance` and the size of `Data.Entities` |
| `just app snapshot [-- --watch]` | `bazaar-tracker/examples/snapshot.rs`. Tries to attach every 500 ms for up to 10 s, then prints the run id, selected hero, run state, storage, offer, both sides' stats, board, stash and skills with each card's details, the PvP opponent, rank and last combat. `--watch` prints again every 2 s and reports attach and detach |

At the main menu `BoardManager.Instance` is not initialised and `Entities` is empty: there is no run, which is expected.

## After a game update

1. Run `just app spike` at the main menu, then during a run.
   - `UnsupportedUnityVersion` means a new Unity line: add a `MonoOffsets` table and a branch in `offsets::for_version`.
   - `UnexpectedRootDomainCode`, `ModuleNotFound` or `ExportNotFound` mean the Mono runtime changed shape.
   - A `ClassNotFound` or a missing field in the dump means the game renamed something: fix the name in `memory/game.rs`.
2. Run `just app snapshot -- --watch` in a shop, a PvE combat and a PvP fight, and compare gold, health, level, day, items and the offer with the screen.
3. Watch for detaches every five polls: that is the five-strikes rule firing because every board read fails. The error names the field and class.
4. Write the run up as a record in `docs/records/`, as `2026-10-31.001` to `.003` did.

## Tests

- `unityspy-rs`: `mem/*_tests.rs` (exe and module name matching, partial reads, typed reads, C strings at a page end, opening and reading this test process, `kernel32.dll`'s version) and `mono/*_tests.rs` (export through its ordinal, root-domain bytes, assembly walk and loops, class lookup and caching, nested classes, statics through the vtable and a base class, runtime-class fields, value widths, strings, arrays, enums, generic instances, `List`/`Dictionary`/`HashSet`/`Nullable` under each profile's field names). They build fake Mono memory with `mono::testing::Fake` (`image`, `class` with `ClassSpec`/`FieldSpec`, `object`, `string`, `array`), laid out at the 2021/2022 offsets.
- `bazaar-tracker`: `memory/game_tests.rs` (five-strikes counting, `Guid` text, rank plausibility, skill order, `NotInitialised` → `Ok(None)`, `<Name>k__BackingField`, staying detached with no game, stopping the connection loop) and `memory/memory_tests.rs` (`FakeMemory` scripts, `BoardState` helpers).

No test needs the game. Run them with `just app test`.
