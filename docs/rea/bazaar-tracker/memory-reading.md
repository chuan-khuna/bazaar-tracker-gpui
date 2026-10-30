# BazaarDBTracker: Game Memory Reading

This is a deep dive into how `BazaarDBTracker.dll` (namespace `BazaarDBCompanion`) reads The Bazaar's live game state, and into the bundled `HackF5.UnitySpy.dll` that does the low-level work. It is static only: neither the tracker nor the game was run.

Targets:
- `BazaarDBTracker.dll`, SHA-256 `ab1734c4…f362d` (see `overview.md`).
- `HackF5.UnitySpy.dll`, SHA-256 `b59d956fb5ab4ec0f6938404134a7d507877ad5dc60e528b789b66adba08f44a`, MVID `1dae9f2b-d0a4-4958-b6c3-9fbdc66ed065`. Assembly version 1.0.0.0, `.NETStandard 2.0`, IL plus ReadyToRun, strong-named (token `a5fabbc78fde260d`). REA evidence: `ev_11eba831b68ad233e896946a99a97768b84be0b1d21f77f9228c7b7dbbc214e6`, whose parent is `ev_552b24b3…6cfa`.

## Method

There is no ILSpy on the machine. I wrote a small CIL disassembler that runs inside PowerShell 7 on `System.Reflection.Metadata`. It resolves every token, `ldstr` literal, branch target, local and exception region. I ran it over:
- every type in `HackF5.UnitySpy.dll`;
- `BazaarDBCompanion.Watcher.*`, `BazaarDBCompanion.Game.*` and `BazaarDBCompanion.Events.GameTracker` in the tracker.

The script and its outputs are in the scratchpad under `mem_agent/` (`MemDis.cs`, `unityspy.il`, `watcher.il`, `gameenums.il`, `tracker.il`). Nothing was installed and no target code was loaded.

How claims are marked:
- **[V]** means read from CIL: literals, constants, call order and branch structure.
- **[I]** means inferred. That covers mono-struct names, which I mapped from my knowledge of the Mono source, game-side type semantics, and the meaning of the flag bits.

No embedded directives or prompt-injection text were found in either binary's strings.

---

## 1. How UnitySpy attaches: process, mono module, domain, image, class

### 1.1 What the tracker actually calls [V]

`MemoryReader.TryConnect`:
1. `Process.GetProcessesByName("TheBazaar")` takes the first match. Any extra matches are disposed.
2. `new ProcessFacadeWindows(proc)` calls `OpenProcess(0x410 = PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid)`. On failure it throws a `Win32Exception`.
3. `new UnityProcessFacade(facade, MonoLibraryOffsets.Unity2021_3_2022_3_x64_PE_Offsets)`. **The offset table is hard-coded.** The tracker never calls `MonoLibraryOffsets.GetOffsets(...)`, so it does no version detection and never uses the PeNet path. `grep GetOffsets` over the tracker returns 0 hits.
4. `AssemblyImageFactory.Create(upf, "TheBazaarRuntime")`.
5. `image.GetTypeDefinition` is called for four types:
   - `"TheBazaar.Data"`. This one is required. If it is missing, the tracker logs `type TheBazaar.Data not found in TheBazaarRuntime — game build changed?`.
   - `"TheBazaar.AppState"`, `"BoardManager"` (global namespace) and `"TheBazaar.ClientCache"`. These are optional (`isinst`).
6. `OnConnected(proc)` derives `<dir>\<exe>_Data\Managed`. It logs `Connected to {name} (pid N): Unity {FileVersion}, TheBazaarRuntime.dll {mtime}Z`, then calls `GameEnums.InitializeFrom(managedDir)` (see §5).

### 1.2 Finding the mono module [V]

`UnityProcessFacade.GetMonoModule()` calls `ProcessFacadeWindows.GetModule("mono-2.0-bdwgc.dll")`:
- `EnumProcessModulesEx(handle, buf[2048*ptr], …, LIST_MODULES_ALL=3)`
- `GetModuleFileNameEx` for each module
- `GetModuleInformation` for `BaseOfDll` and `SizeInBytes`
- an exact match on the filename

### 1.3 Finding the root domain [V]

- **Windows (PE).** `ReadModule` copies the **whole module image** with a single `ReadProcessMemory(base, SizeOfImage)`. It then parses the export directory from that copy:
  - `e_lfanew = u32@0x3C`
  - `exportDir RVA = u32@(e_lfanew + 0x88)`. The offset is 0x78 for PE32. Constants: `PEFormatOffsets.Signature=60`, `ExportDirectoryIndexPE=120`, `ExportDirectoryIndexPE32Plus=136`.
  - `NumberOfFunctions = u32@(exp+20)`, `AddressOfFunctions = u32@(exp+28)` and `AddressOfNames = u32@(exp+32)`.
  - It loops `i` over 0…`NumberOfFunctions`, reads `name RVA = u32@(names + 4i)` and looks for `"mono_get_root_domain"`. The function address is then `base + u32@(functions + 4i)`.
  - **Quirk:** the function index comes from the *names* array index, and there is no `AddressOfNameOrdinals` indirection. This works only when names and functions are in the same order, as they are in Mono builds. A port should use the ordinal table.
- **Decoding the root-domain global.** This is in `GetAssemblyImage`:
  - On x64 Windows the stub is `48 8B 05 <rel32> C3` (`mov rax,[rip+rel32]; ret`), so `domainPtrAddr = fn + i32@(fn+3) + 7` and `domain = ReadPtr(domainPtrAddr)`.
  - On x64 macOS the constants are 7/11 instead (`fn + i32@(fn+7) + 11`).
  - On x86 the code is `mov eax,[abs32]`, so `domain = ReadPtr(ReadPtr(fn+1))`.
- **Mach-O** (UnitySpy only; the tracker never takes this path). It reads `libmonobdwgc-2.0.dylib` **from disk**, walks the load commands until it finds `LC_SYMTAB` (cmd 2), and searches nlist_64 entries for `_mono_get_root_domain`.

### 1.4 Finding the assembly image [V]

```
asm_list = ReadPtr(domain + ReferencedAssemblies)      // MonoDomain.domain_assemblies (GSList*) [I name]
while node != 0:
    asm  = ReadPtr(node)                                // GSList.data
    name = ReadAsciiString(ReadPtr(asm + 2*ptr), 1024)  // MonoAssembly.aname.name [I]
    if name == "TheBazaarRuntime": image = ReadPtr(asm + AssemblyImage)   // MonoAssembly.image
    node = ReadPtr(node + ptr)                          // GSList.next
```

If no assembly matches, it throws `Unable to find assembly '…'`.

### 1.5 Enumerating classes [V]

`AssemblyImage` construction enumerates the **whole class cache once**. `MonoImage.class_cache` is a `MonoInternalHashTable` [I name]:

```
size  = u32 @ image + ImageClassCache + HashTableSize
table = ptr @ image + ImageClassCache + HashTableTable
for i in 0..size: for klass = ReadPtr(table + i*ptr); klass; klass = ReadPtr(klass + TypeDefinitionNextClassCache)
```

- Each `TypeDefinition` eagerly reads `bitFields`, `fieldCount`, `name`, `namespace`, `size`, `runtime_info → vtable`, `byval_arg` (as a `TypeInfo`), `vtable_size` and `class_kind`.
- Full names are built from the nested-in chain as `Ns.Outer+Inner`. Generic names (any name containing a backtick) are not indexed by name.
- `GetTypeDefinition(name)` is a dictionary lookup. Anything reached only by pointer (array element classes, generic instances, runtime types) is created lazily through `GetTypeDefinition(IntPtr)`.

### 1.6 Static field values [V]

```
vtable      = ReadPtr(ReadPtr(klass + TypeDefinitionRuntimeInfo) + RuntimeInfoDomainVTables)   // runtime_info->domain_vtables[0]
static_data = ReadPtr(vtable + VTable + vtable_size*ptr)   // the slot after the vtable's method array
value       = field.GetValue(static_data)                   // + field.offset
```

- Mono only creates the vtable after the class is initialised. Until then `runtime_info` is null and `VTable = 0` [I].
- The tracker reads the singleton base class (`BoardManager.Parent`) to get `Instance`.

### 1.7 Fields and instance values [V]

- `MonoClass.fields` is an array of `MonoClassField`, with stride `TypeDefinitionFieldSize` (0x20 on x64). Each entry holds:
  - `type*` at +0
  - `name*` at +ptr
  - (parent at +2·ptr, not read)
  - `int offset` at +3·ptr
- A field list is the class's own fields, plus the generic-definition fields for `GInst` classes, plus the parent's fields. It is sorted by name. A lookup by name returns the first match.
- **`MonoType`**: `data` (ptr) at +0 and `attrs` (u32) at +ptr.
  - `typeCode = (attrs >> 16) & 0xFF`
  - `IsStatic = attrs & 0x10`
  - `IsConstant (literal) = attrs & 0x40`
- **Reading an instance field**: the address is `obj + offset`. Fields of a **value type** (a struct read in place) use `offset - 2*ptr`, because Mono's field offsets include the 16-byte object header (vtable and sync).
- **Dispatch on `typeCode`** (`ProcessFacade.ReadManaged`):

| Type code | Read as |
|---|---|
| `BOOLEAN` / `CHAR` / `I1` … `U8` / `R4` / `R8` | Little-endian primitive. |
| `STRING` (0x0E) | `ReadManagedString(ReadPtr(addr))`. |
| `SZARRAY` (0x1D) | `ReadManagedArray`. |
| `CLASS` / `OBJECT` (0x12 / 0x1C) | `ReadPtr`, then a `ManagedClassInstance` (null when the pointer is 0). |
| `GENERICINST` (0x15) | `ReadPtr(type.data)` gives the container class. A value type becomes a `ManagedStructInstance` *in place*; otherwise it is read as a class. |
| `VALUETYPE` (0x11) | `ManagedStructInstance` in place. Enums return `value__`. If the struct read throws, it falls back to `ReadInt32` (an enum stored as int). |
| `VAR` (0x13) | Resolves the generic argument by position, then reads at an adjusted offset. |
| `I` / `U` / `PTR` / … | Throws. |

- **`ManagedClassInstance`**: `vtable = ReadPtr(obj)` and `klass = ReadPtr(vtable)`. The runtime type comes from `MonoVTable.klass` at +0.
- **Generic arguments.** For a `GENERICINST` the code works through these steps:
  - `genericClass = type.data`
  - `container = ReadPtr(genericClass)`
  - `GenericContainer` is at `container + TypeDefinitionGenericContainer`, and `type_argc = i32@(ReadPtr(that) + 4*ptr)`.
  - `inst = ReadPtr(genericClass + ptr)` (`context.class_inst`), and the argv starts at `inst + 8`.
  - The `MonoGenericInst` header is about 8 bytes [I].

### 1.8 Strings and arrays (the object layout) [V]

| Object | Layout (x64) |
|---|---|
| Any object | `+0x00 vtable*`, `+0x08 sync` |
| `System.String` | `+0x10 int32 length`, `+0x14 UTF-16 chars`. **Fork addition:** a length below 0 or above 4,194,304 throws `Implausible managed string length … corrupt or dangling string pointer.` |
| `T[]` (SZARRAY) | `arrayClass = ReadPtr(ReadPtr(obj))`, then `elementClass = ReadPtr(arrayClass)` (`MonoClass.element_class` at +0) [I name]. `+0x18 int32 length` (`max_length`), `+0x20 data`. The stride is `arrayClass.Size`, i.e. `sizes.element_size` [I]. Each element is read with `elementClass.byval_arg`. |

`ReadAsciiString(addr, 1024)` reads a fixed 1,024 bytes (no partial reads allowed) and cuts at the NUL. Class and field names come from here.

### 1.9 `ReadProcessMemory` wrapper [V]

- The buffer is pinned and `ReadProcessMemory` is called through a P/Invoke.
- `ERROR_PARTIAL_COPY (299)` is tolerated only when `allowPartial` is set, which happens only for whole-module reads.
- Small reads rent buffers from `ByteArrayPool`.

### 1.10 Offset tables (`MonoLibraryOffsets`) [V]

Values are decimal, with hex after the slash. **The tracker uses only the `Unity2021_3_2022_3_x64_PE` column.**

| Field (UnitySpy name) | Mono struct member [I] | 2018.4.10 x86 PE | 2019.4/2020.3 x64 PE | **2021.3/2022.3/6000.0 x64 PE** | 2019.4 x64 Mach-O |
|---|---|---|---|---|---|
| `AssemblyImage` | `MonoAssembly.image` | 68 | 96 | **96 / 0x60** | 96 |
| `ReferencedAssemblies` | `MonoDomain.domain_assemblies` | 108 | 200 | **160 / 0xA0** | 224 |
| `ImageClassCache` | `MonoImage.class_cache` | 852 | 1216 | **1232 / 0x4D0** | 1216 |
| `HashTableSize` | `MonoInternalHashTable.size` | 12 | 24 | **24 / 0x18** | 24 |
| `HashTableTable` | `MonoInternalHashTable.table` | 20 | 32 | **32 / 0x20** | 32 |
| `TypeDefinitionFieldSize` | `sizeof(MonoClassField)` | 16 | 32 | **32 / 0x20** | 32 |
| `TypeDefinitionBitFields` | `MonoClass` flags word (bit 0x4 = valuetype, 0x8 = enumtype) | 20 | 32 | **32 / 0x20** | 32 |
| `TypeDefinitionClassKind` | `MonoClass.class_kind` (byte, & 7) | 30 | 42 | **27 / 0x1B** | 36 |
| `TypeDefinitionParent` | `parent` | 32 | 48 | **48 / 0x30** | 40 |
| `TypeDefinitionNestedIn` | `nested_in` | 36 | 56 | **56 / 0x38** | 48 |
| `TypeDefinitionName` | `name` | 44 | 72 | **72 / 0x48** | 64 |
| `TypeDefinitionNamespace` | `name_space` | 48 | 80 | **80 / 0x50** | 72 |
| `TypeDefinitionVTableSize` | `vtable_size` | 56 | 92 | **92 / 0x5C** | 84 |
| `TypeDefinitionSize` | `sizes.instance_size` / `element_size` | 92 | 144 | **144 / 0x90** | 136 |
| `TypeDefinitionFields` | `fields` | 96 | 152 | **152 / 0x98** | 144 |
| `TypeDefinitionByValArg` | `_byval_arg` (inline `MonoType`) | 116 | 184 | **184 / 0xB8** | 176 |
| `TypeDefinitionRuntimeInfo` | `runtime_info` | 132 | 208 | **208 / 0xD0** | 200 |
| `TypeDefinitionMonoGenericClass` | `MonoClassGenericInst.generic_class` | 148 | 240 | **240 / 0xF0** | 232 |
| `TypeDefinitionFieldCount` | `field_count` (def) | 164 | 256 | **256 / 0x100** | 248 |
| `TypeDefinitionNextClassCache` | `next_class_cache` | 168 | 264 | **264 / 0x108** | 256 |
| `TypeDefinitionGenericContainer` | `MonoClassGtd.generic_container` | 172 | 272 | **272 / 0x110** | 264 |
| `RuntimeInfoDomainVTables` | `MonoClassRuntimeInfo.domain_vtables[0]` | 4 | 8 | **8** | 8 |
| `VTable` | `MonoVTable.vtable[]` start | 40 | 64 | **72 / 0x48** | 64 |

Other notes on the table:
- `MonoClassKind`: Def=1, GTg=2, GInst=3, GParam=4, Array=5, Pointer=6.
- `TypeCode` (UnitySpy enum) matches `MonoTypeEnum`.
- **The x64 PE "2021_3_2022_3" table also lists `Version6000_0_58`, i.e. Unity 6000.0.58.** This is a fork addition; upstream UnitySpy stops at 2022. That means the same offsets are believed to hold for Unity 6.0 Mono. [V that the list contains it; I that it is correct.]
- The version-selection logic (`GetOffsets` with best-candidate fallback via `GetBestCandidate` year/minor/patch distance, and PeNet reading `FileVersion` and the PE machine type) exists but is **dead code for this app**.

### 1.11 Other fork differences seen [V]

- Linux (ptrace, direct, dump, client) and macOS (direct, client) facades are present but unused.
- There is an implausible-string-length guard.
- `ReadManaged(VALUETYPE)` catches failures and falls back to int32.
- `AssemblyImage` keys types by address in a `ConcurrentDictionary`, which makes lookups thread-safe.

---

## 2. Field-path table

Notation:
- `Data` = static class `TheBazaar.Data` in `TheBazaarRuntime.dll` (Mono image `TheBazaarRuntime`).
- `AppState` = `TheBazaar.AppState`, `ClientCache` = `TheBazaar.ClientCache` and `BoardManager` = global `BoardManager`.
- `S:` marks a static read (§1.6). `.` marks an instance field.
- `<X>` is short for the auto-property backing field `<X>k__BackingField`.

Type columns:
- "UnitySpy type" is what the tracker casts or `isinst`s to (verified).
- "Game type" is inferred unless the code names it.

### 2.1 Board snapshot (`ReadBoardState`, giving `BoardStateEventArgs`)

| Value (`BoardStateEventArgs`) | Path | UnitySpy / CLR type per step | Notes |
|---|---|---|---|
| `IsStorageOpen` | `S: Data.<IsStorageOpen>` | `bool` (unboxed) | [V] |
| `RunState` | `S: Data.<CurrentState>` then `.StateName` | `ManagedClassInstance`, then int-convertible, cast to `ERunState` | A null `CurrentState` makes the whole read return null. Uses the raw int and the baked enum. [V] |
| `SelectionSet` | `S: Data.<CurrentState>.<SelectionSet>` | `List<string>`: `_size`, `_items` (`string[]`) | Skips null and empty entries. [V] |
| `CurrentEncounterId` | `S: Data.<CurrentEncounterId>` | `ManagedStructInstance` = `Nullable<Guid>`, read **raw**: `i32@0 = hasValue`; Guid `_a = i32@4`, `_b,_c = u16` from `i32@8`, `_d.._k = bytes` from `i32@12`, `i32@16` | `Util.ReadCurrentEncounterId`, called via the private `MemoryObject.ReadInt32(int)` through reflection. [V] |
| (run object) | `S: Data.<Run>` | `ManagedClassInstance` | A null run gives a null snapshot. [V] |
| Player skill order | `Run.<Player>.<Skills>` then `List`, then each item's `<InstanceId>` | `ManagedClassInstance` → `List<card>` (`_size`/`_items`) → card | Used to sort `PlayerSkills`. [V] |
| Opponent skill order | `Run.<Opponent>.<Skills>` | same | [V] |
| `PlayerHealth`, `PlayerHealthMax`, `Gold`, `PlayerPrestige`, `PlayerIncome`, `PlayerLevel` | `Run.<Player>.<Attributes>` | `Dictionary<EPlayerAttributeType,int>`: `_count`, `_entries[i]` struct; `i32@0 hashCode` (skipped if < 0), `i32@8 key`, `i32@12 value` | The key ids come from `GameEnums.ValueOf("EPlayerAttributeType", name)`, with these baked fallbacks: **Health=10, HealthMax=11, Gold=4, Prestige=9, Income=5, Level=15**. [V] Which dictionary each of the 8 `TryGetValue` calls uses is [I], but the setter order is player HP/HPmax, opponent HP/HPmax, then gold/prestige/level/income. |
| `OpponentHealth`, `OpponentHealthMax` | `Run.<Opponent>.<Attributes>` | same | [V] keys / [I] pairing |
| `Day`, `Hour`, `Victories`, `Losses` | `Run.<Day>`, `Run.<Hour>`, `Run.<Victories>`, `Run.<Losses>` | int (`Util.ReadInt`: the field must exist, then `Convert.ToInt32`) | [V] |
| `PlayerHero` | `Run.<Player>.<Hero>` | int (`EHero`) → `PlayerHeroName` (0 gives null) → `GameEnums.NameOf("EHero")`, falling back to baked `EHero` | The hero is read on the object loaded just before; it is [I] that this is Player. [V] the calls |
| `OpponentHero` | `Run.<Opponent>.<Hero>` | int → `HeroName` | [V] |
| Entities | `S: Data.Entities` (a plain field, not a backing field) | `Dictionary<?,card>`: `_count`, `_entries[i].value` (via dynamic) → `ManagedClassInstance` | The key type is unknown. The class `FullName` is stored as `typeName`. Items are compared to `"BazaarGameClient.Domain.Models.Cards.ItemCard"`, which tells us the card hierarchy lives in namespace `BazaarGameClient.Domain.Models.Cards`. [V] |
| Opponent board slots | `S: Data.<CardControllerToBoardTarget>` | `Dictionary<CardController, BoardTarget>` entries: `i32@0 hashCode ≥ 0`; `"key"` field gives the controller; `u32@16 | u32@20<<32` is a 64-bit flag value. The lowest set bit in 10..19 gives the slot (`bit − 10`). `key.<CardData>` gives the card's `<InstanceId>` and `<Size>` | Result: `instanceId → (boardPosition, slotWidth = ECardSize)`. [V] Bit meaning: [I] bits 10–19 are the opponent's ten board sockets. |

**Per-card fields** (`Util.*` on each entity `card`, all instance fields):

| Output key | Path on card | Type and read | |
|---|---|---|---|
| `templateId` | `.<TemplateId>` | `Guid` struct: fields `_a`(int) `_b`,`_c`(short) `_d`…`_k`(byte), rebuilt into a `System.Guid` | [V] |
| `instanceId` | `.<InstanceId>.<Value>` then `.ToString()` | struct wrapping a string-like value | The prefixes `itm_`, `enc_`, `ste_`, `skl_`, `com_` and `pvp_` map to a kind in `CardKind.FromInstanceId`. [V] |
| `tier` | `.<Tier>` | int → `ETier` | [V] |
| `type` | `.<Type>` | int → `ECardType` | [V] |
| `size` | `.<Size>` | int → `ECardSize` | [V] |
| `socketId` | `.<LeftSocketId>` | `Nullable<EContainerSocketId>`, raw: `i32@0 hasValue`, `i32@4 value` | [V] |
| `section` | `.<Section>` | `Nullable<EInventorySection>` (raw, as above) | [V] |
| `enchantment` | `.<Enchantment>` | `Nullable<EEnchantmentType>` (raw); emitted only for `ItemCard` | [V] |
| `attributes` | `.<Attributes>` | `Dictionary<ECardAttributeType,int>`, with entry key at 8 and value at 12, giving `{name: int}` | [V] |
| `tags` | `.<Tags>` | `HashSet<ECardTag>`: `_count`, `_slots[i]`: `i32@0 hashCode ≥ 0`, `i32@8 value` | Uses the old-style HashSet `Slot` layout. [V] |
| owner | `.<Owner>.<CombatantId>` | class, then int → `ECombatantId` | [V] |

**Bucketing** [V for the branches; the meanings rely on the baked enum values below]:
- `owner == Player(0)`:
  - `type == Skill(1)` goes to `PlayerSkills`.
  - Otherwise `section == Stash(1)` goes to `PlayerStash`.
  - Everything else goes to `PlayerBoard`.
- Otherwise, `type == Skill` goes to `OpponentSkills`.
- Otherwise, if `type ∉ {Item 0, EncounterStep 3, SocketEffect 7}`, the card is kept only if `TemplateId == CurrentEncounterId` (both nullable), and it is de-duplicated by GUID.
- Then, if `section` is null or `Hand(0)`, the card goes to `OpponentBoard`.
- Non-player items with no section get `boardPosition` and `slotWidth` from the controller map.
- `EntityIndex[instanceId] = EntityRef(templateId, tierName, enchantmentName, sizeName)`.

**Post-processing:**
- `SortBySkillOrder` orders the two skill lists by `<Skills>` order.
- `ComputeOpponentVisualPositions` adds `visualPosition = (10 − Σwidths)/2 + Σwidths of sockets left of this one`. It caches widths in `_socketSlotWidths` and keeps them while the opponent's item set overlaps the previous one.

### 2.2 Combat (`ReadCombatResult` and `ReadCombatFrames`) [V]

The source is tried first from `BoardManager`, then from `AppState`:

| Source | Path |
|---|---|
| A | `BoardManager.Parent` (the singleton base class) `S: Instance` → `._lastCombatSim` |
| B | `S: AppState._cachedCombatSim` → `.<Data>`, or `.Data` |

From the sim object:
- `.Winner` and `.Loser` are ints (`ECombatantId`).
- `.Frames` is a `List`, and `_size` gives the frame count.
- `ReadCombatFrames` serialises `Frames` with `ManagedJson.Serialize(frames, budget = 300000 nodes, maxDepth = 32)` and returns `CombatFramesResult(Frames JsonNode, FrameCount, Winner, Loser, Truncated, Nodes, ElapsedMs, Diag)`.
- Diagnostic strings are joined with `"; "`: `bm-type-unresolved`, `bm-singleton-base-unresolved`, `bm-instance-null`, `bm-lastCombatSim-null`, `appstate-sim-null`, `appstate-data-null (…)`, `frames-null` and `winner/loser-unreadable (…)`.

### 2.3 Other reads

| Value | Path | Types | |
|---|---|---|---|
| PvP opponent (`PvpOpponentInfo{Name,Hero,Rating,Level}`) | `S: Data.<SimPvpOpponent>` → `.<Name>`, `.<Hero>`, `.<Rating>`, `.<Level>` | class → string / int (`EHero` → name) / int / int | [V] |
| Run init (`RunInitInfo{BzRunId,PlayMode,BuildId,Environment}`) | `S: AppState._cachedRunInitialized` → `.<RunId>`, `.<PlayMode>`, `.<BuildId>`, `.<Environment>` | class → strings | If that fails, `ReadActiveRunId`: `S: Data.<Run>` then the first existing field of `<Id>`, `<RunId>`, `Id`, `RunId`, `_id`, read as a string or as a `Guid` struct (`_a`…`_k`). [V] |
| Selected hero | `S: Data.<Run>.<Player>.<Hero>`, falling back to `S: ClientCache.RunConfig._value.<Hero>` (used only if > 0) | class → class → int | [V] |
| Player rank (`PlayerRankInfo{Rating,Rank,Division}`) | `S: ClientCache.Rank` (an observable) → `._value` (struct; only its `TypeDefinition` is used) | Field offsets `<SeasonID>`, `<Rank>`, `<Division>`, `<Rating>` taken as `Offset − 16`. Then a **raw scan**: for `k = 16, 20, …, 48` it reads `u32@(observable + k + rel)` and accepts the first `k` with SeasonID 1–10000, Rank 0–16, Division 1–10 and Rating 0–1,000,000. | This works around unreliable inline-struct offsets in a generic `Observable<T>`. [V for the code; I for the reason] |

`RankName` uses `GameEnums.NameOf("ERank")`, then the baked `ERank`, then the number.

---

## 3. Collections and `ManagedJson`

### 3.1 How the tracker reads collections [V]

All collection access goes through `ManagedObjectInstance["field"]` plus the C# `dynamic` binder (`Microsoft.CSharp`):
- `int`s use `Convert.ToInt32` through a dynamic `InvokeMember`.
- Array and entry indexing (`entries[i]`) uses a dynamic `GetIndex` on the `object[]` that UnitySpy returns.

| Collection (Mono BCL, .NET Framework 4.x layout) | Fields used | Element read |
|---|---|---|
| `List<T>` | `_size` (int), `_items` (`T[]`) | Elements `0.._size-1` |
| `Dictionary<K,V>` | `_count`, `_entries` (`Entry[]`) | Struct `{int hashCode @0, int next @4, K key @8, V value @8+sizeof(K) (aligned)}`. A negative `hashCode` marks a free slot. Entries are read either by name (`"key"`, `"value"`, `"hashCode"`) or at raw offsets 0/8/12/16/20 with `ReadInt32` / `ReadUInt32`. |
| `HashSet<T>` | `_count`, `_slots` (falling back to `_entries` in ManagedJson) | `Slot {int hashCode @0, int next @4, T value @8}` |
| `Nullable<T>` | Raw `i32@0` hasValue, `i32@4` value (Util); by name `hasValue` / `has_value` and `value` (ManagedJson) | |
| `string` | §1.8 | UTF-16 |
| `T[]` | §1.8 | Element stride = array class `element_size` |

**Raw struct reads.** `Util` obtains the **non-public** `ReadInt32(int)` / `ReadUInt32(int)` of UnitySpy's `MemoryObject` through `GetType().GetMethod(name, Instance | Public | NonPublic)`, cached in `Util._readInt32`. It calls them with byte offsets relative to the start of the struct.

### 3.2 `ManagedJson` (combat frame serialiser) [V]

`Serialize(root, budget, maxDepth)` returns `(JsonNode, Truncated, NodesEmitted)` and catches every exception.

`Convert(o, depth)` works like this:
1. If `budget ≤ 0`, set `Truncated` and return null. If `o` is null, return null.
2. `string`, `bool`, `float`, `double`, `decimal` and `char` become JSON values.
3. Enums and integer primitives become `long`.
4. If `depth ≥ maxDepth`, set `Truncated` and return null.
5. An `IManagedObjectInstance` goes to `ConvertObject`. Anything else is serialised with `ToString()`.

`ConvertObject` dispatches on `TypeDefinition.Name`:

| Name starts with | Output |
|---|---|
| `Nullable` | The inner value, or null when `hasValue` / `has_value` is false. |
| `List`, `IList` or `IReadOnlyList` | A JSON array of `_items[0.._size)`. |
| `Dictionary` | A JSON object. It reads `_entries` up to `_count`, skips entries with `hashCode < 0`, uses `KeyToString(key)` as the property name, and skips duplicates via `ContainsKey`. |
| `HashSet` | A JSON array from `_slots` or `_entries`, reading `value` or `Value`. It scans at most `_count + 64` slots. |
| anything else | `ReadFields`: `{"$type": Name, <field>: …}` over all non-static fields. `CleanName` turns `<Foo>k__BackingField` into `Foo`. |

- `KeyToString`: primitives use `ToString`, enums use their int64 value, and objects try `<Value>k__BackingField` / `value` / `Value`, then fall back to the compact JSON.
- Every node costs 1 from `_budget`, and `NodesEmitted` counts them.

---

## 4. Cadence, attach, detach and threads

- **Connection loop** [V]: `GameManager.Start` runs `Task.Run(ConnectionLoop)` and then `ILogReader.Start(ct)`. `ConnectionLoop` repeats every **500 ms** (`Task.Delay(500, ct)`):
  - It takes `lock(_memoryLock)`. If the reader is not connected, it calls `TryConnect()`.
  - If the connected state changed, it raises `ConnectionChanged(bool)`. On a disconnect it also calls `GameTracker.Reset()`.
  - It exits on `OperationCanceledException` or `ObjectDisposedException`.
- **No periodic state polling.** Memory is read **on demand**:
  - `GameTracker` holds the `Func<>` delegates (`ReadBoardSafe`, `ReadCombatResultSafe`, `ReadCombatFramesSafe`, `ReadRunInitSafe`, `ReadPvpOpponentSafe`, `ReadSelectedHeroSafe` and `ReadPlayerRankSafe`).
  - Each one takes `_memoryLock`, returns null if disposed, calls `TryConnect` if not connected, and then reads.
  - Reads are triggered by `Player.log` lines through debounced timers in `GameTracker`:
    - `ScheduleSettleLocked(500 | 350)` and `ScheduleCombatCaptureLocked(500 | 350)` from `HandleStateChanged` and `HandleGameSimSummary`;
    - a deferred combat outcome `Task.Delay(1200)`;
    - `RankSettleDelayMs = 4000` after the run ends.
  - (These are [V] constants; the exact event wiring is the events agent's scope.)
- **`IsConnected`** [V] = `_process != null && !_process.HasExited && _dataType != null`. A `Win32Exception` or `InvalidOperationException` counts as false.
- **Detaching** [V]:
  - `Disconnect()` disposes the facade (`CloseHandle`) and the `Process`, nulls the four type definitions, and clears the opponent caches.
  - It is called in these cases:
    1. at the start of every `TryConnect`;
    2. on **process death**, i.e. `IsProcessDeath(ex)`: any `Win32Exception` in the `InnerException` chain;
    3. after **5 consecutive** failed `ReadBoardState` calls;
    4. from `GameManager.Dispose` under the lock.
  - The next 500 ms tick or the next `Read*Safe` reconnects. A reconnect rebuilds the whole class cache.
- **Connect failures** are logged once per distinct message (`LogConnectFailure` de-duplicates them).
- **Threads**: the connection loop runs on the thread pool. Reads run on timer continuations (thread pool) and on the log-reader callback thread. `GameEnums.Load` runs on `Task.Run`. **All memory access is serialised by `GameManager._memoryLock`**, and UnitySpy's caches are concurrent dictionaries.
- **Elevation**: at startup the app relaunches itself as admin (`overview.md` §4). For same-user `PROCESS_VM_READ` this is likely unnecessary, unless the game runs elevated or is protected [I].

---

## 5. `GameEnums`

[V]:
- `InitializeFrom(managedDir)` returns if the directory does not exist. A one-shot `Interlocked.Exchange(_initStarted)` guard ensures it runs once per app lifetime; it does not re-run on reconnect. It then calls `Task.Run(() => Load(managedDir))`.
- `Load`:
  1. `Directory.GetFiles(managedDir, "*.dll")`, then finds `BazaarGameShared.dll` (case-insensitive). If it is missing, it logs `GameEnums: BazaarGameShared.dll not found — baked enums stay in effect.`
  2. Builds `new MetadataLoadContext(new PathAssemblyResolver(allDlls), "mscorlib")` (with a null core-assembly fallback) and `LoadFromAssemblyPath`. It never executes game code.
  3. For each of the 13 `WantedEnums`, it does `asm.GetType(fullName)`. If the type is an enum, it takes the `GetFields(Public | Static)` literals, computes `GetRawConstantValue → Int64`, and builds `Names: long→string` and `Values: string→long`.
  4. Publishes `_maps` (volatile) and logs `GameEnums: N enums mapped from game metadata (unresolved, baked fallback: …)`.
- The 13 wanted enums:
  - `BazaarGameShared.Domain.Core.Types.`: `ECardAttributeType`, `ECardTag`, `EHero`, `ETier`, `ECardType`, `ECardSize`, `EEnchantmentType`, `EInventorySection`, `EContainerSocketId`, `ECombatantId`, `EPlayerAttributeType`
  - `BazaarGameShared.Domain.Runs.ERunState`
  - `BazaarGameShared.TempoNet.Enums.ERank`
- API:
  - `Name<T>(T)` returns `NameOf(typeof(T).Name, value)`, falling back to `value.ToString()` on the baked enum.
  - `ValueOf(enum, name)` returns `int?`, or null if the enum is unmapped.
- **Important.** The live map affects **names only**. The control flow in `ReadBoardState` compares raw ints against **baked** constants: Skill=1, Stash=1, Hand=0, Player=0, and Item/EncounterStep/SocketEffect = 0/3/7. If the game renumbers these, the bucketing silently breaks while the names stay right.

**Baked fallback values** [V]:
- `ECardType`: Item 0, Skill 1, CombatEncounter 2, EncounterStep 3, EventEncounter 4, PedestalEncounter 5, PvpEncounter 6, SocketEffect 7.
- `ECardSize`: Small 1, Medium 2, Large 3.
- `ETier`: Bronze 0, Silver 1, Gold 2, Diamond 3, Legendary 4.
- `ERank`: Bronze 0, Silver 1, Gold 2, Diamond 3, Legendary 4.
- `EHero`: Common 0, Pygmalien 1, Vanessa 2, Stelle 3, Jules 4, Dooley 5, Mak 6, Karnok 7, Hero8 8.
- `ECombatantId`: Player 0, Opponent 1.
- `EInventorySection`: Hand 0, Stash 1.
- `EContainerSocketId`: Socket_0…Socket_9 = 0…9.
- `EEnchantmentType`: Deadly 0, Fiery 2, Golden 3, Heavy 4, Icy 5, Obsidian 7, Radiant 8, Restorative 9, Shielded 10, Shiny 11, Toxic 13, Turbo 14, Mossy 15. There are gaps at 1, 6 and 12.
- `ERunState`: Choice 0, Combat 1, Encounter 2, EndRunDefeat 3, EndRunVictory 4, LevelUp 5, Loot 6, NewRun 7, Pedestal 8, PVPCombat 9, Shutdown 10.
- `ECardTag`: Weapon 0, Property 1, Food 2, Potion 3, Tool 4, Vehicle 5, Aquatic 6, Friend 7, Core 8, Unsellable 9, Unstashable 10, Merchant 11, Event 12, Combat 13, Loot 14, Ray 15, Dinosaur 16, Apparel 17, Toy 18, Tech 19, Dragon 20, Ingredient 21, Relic 22, Reagent 23, Map 24, Key 25, Drone 26.
- `ECardAttributeType`: Ammo 0 … RepairTargets 99. The full list:
  - Ammo 0, AmmoMax 1, ReloadAmount 2, ReloadTargets 3, Cooldown 4, CooldownMax 5, ChargeAmount 6, ChargeTargets 7
  - Haste 9, HasteAmount 10, HasteTargets 11, Slow 12, SlowAmount 13, SlowTargets 14, Freeze 15, FreezeAmount 16, FreezeTargets 17
  - BurnApplyAmount 18, BurnRemoveAmount 19, PoisonApplyAmount 20, PoisonRemoveAmount 21
  - Multicast 24, Lifesteal 25, CritChance 26, DamageAmount 27, DamageCrit 28, HealAmount 29, HealCrit 30
  - JoyApplyAmount 31, JoyRemoveAmount 32, JoyCrit 33, ShieldApplyAmount 34, ShieldRemoveAmount 35, ShieldCrit 36
  - BuyPrice 37, SellPrice 38, Counter 39, Custom_0…Custom_8 40–48
  - ForceUseTargets 49, EnchantTargets 50, UpgradeTargets 51, DisableTargets 52, BurnCrit 53, PoisonCrit 54, DestroyTargets 55
  - RegenApplyAmount 56, RegenRemoveAmount 57, RegenCrit 58, TransformTargets 59
  - FlatCooldownReduction 60, PercentCooldownReduction 61, EnchantRemoveTargets 62, Flying 63, FlyingTargets 64
  - PercentChargeReduction 65, PercentHasteReduction 66, PercentSlowReduction 67, PercentFreezeReduction 68, DestroyImmunity 69
  - QuestCompletedCount 70, Quest_1…Quest_12 71–82, Heated 90, Chilled 91, RepairTargets 99
- `EPlayerAttributeType` has **no baked enum**, only the inline fallbacks listed in §2.1.

For a Rust port, the equivalent of the live enum load is to parse `BazaarGameShared.dll` metadata (TypeDef, Field, Constant tables). Candidate crates are `dotnetdll` or `pelite` plus a small ECMA-335 reader.

---

## 6. Porting notes (Rust)

### 6.1 Windows

- **Process and module discovery.** Use the `windows` crate:
  - `CreateToolhelp32Snapshot` or `EnumProcesses` to find `TheBazaar.exe`;
  - `OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION)`;
  - `EnumProcessModulesEx(LIST_MODULES_ALL)` with `GetModuleBaseNameW` and `GetModuleInformation`;
  - or `Module32FirstW/NextW` on a snapshot.
- **The reader.** Write a `Reader` trait with `read(addr, &mut [u8])`, plus typed helpers for `u32/i32/u64/ptr`, a C string (stop at NUL, and read in page-safe chunks rather than a fixed 1,024 bytes) and a UTF-16 managed string with a length cap.
- **Finding `mono_get_root_domain`.** Read the remote export directory and use the **ordinal table**. Decode `48 8B 05 rel32` and assert the opcode bytes, rather than assuming them blindly.
- **Mono model.** Port the §1.10 x64 table as a `struct MonoOffsets` with one `const` per Unity line, and key it on the game's `FileVersion`. The tracker logs that version on connect but never validates it.
- **Laziness.** Do not enumerate the whole class cache up front. Hash-walk only to find the ~5 named classes, then resolve fields lazily by name. This replaces UnitySpy's eager build of every `TypeDefinition`.
- **Typed readers.** Implement readers for `List`, `Dictionary` (`Entry` layout plus `hashCode ≥ 0`), `HashSet` `Slot`, `Nullable` and `Guid`, using the layouts in §3.1. Prefer computing value-type sizes from `MonoClass.instance_size − 16` over hard-coding them.
- **Static fields.** These need `runtime_info → domain_vtables[0] → vtable[vtable_size]`. Treat a null vtable as "not yet initialised", not as an error.
- **Errors.** Map them to `ProcessGone` (`ERROR_PARTIAL_COPY` or `ERROR_INVALID_HANDLE` from a dead PID, or a process exit) versus `ReadFailed`. Reconnect on the first and count the second, mirroring the tracker's 5-strike rule.
- **Threads.** A single reader thread, or a `Mutex<Reader>`, matches the tracker's `_memoryLock`. A GPUI app can run it on a background executor.

### 6.2 macOS on Apple Silicon (brief, mostly [I])

- **The app itself is Windows-only.** It hard-wires `ProcessFacadeWindows` and the x64 PE offsets. UnitySpy's Mach-O path covers only Unity 2019/2020 x64 (`libmonobdwgc-2.0.dylib`).
- **Mono vs IL2CPP.** It is **unknown** whether The Bazaar's macOS build is Mono or IL2CPP. Check for `Contents/Resources/Data/Managed/*.dll` (Mono) versus `GameAssembly.dylib` plus `il2cpp_data/Metadata/global-metadata.dat` (IL2CPP).
  - **If Mono on arm64**: the struct offsets probably match the x64 column, since LP64 gives the same pointer size, but this must be verified. `mono_get_root_domain` will be `adrp x8, page; ldr x0, [x8, #off]; ret`, so decode ADRP+LDR instead of the RIP-relative form. Get the symbol from the on-disk dylib's `LC_SYMTAB`, as UnitySpy does, plus the dyld slide. Find the slide with `task_info(TASK_DYLD_INFO)` and the image list.
  - **If IL2CPP**: everything is different. Classes are `Il2CppClass` (from `il2cpp_class_from_name` or the type-info table), statics are `klass->static_fields`, and the field layouts come from `global-metadata.dat` plus the binary (Il2CppDumper-style). The `List`/`Dictionary` layouts are the IL2CPP (.NET-Core-ish) BCL, e.g. `_entries` without `_slots`.
- **Memory access.** Use `task_for_pid` (`mach2` crate) and `mach_vm_read_overwrite` / `mach_vm_region`. This needs root, or the `com.apple.security.cs.debugger` entitlement, plus a target without hardened runtime or with `get-task-allow`. Steam and notarised games usually ship with hardened runtime, so expect to need SIP or entitlement workarounds. That makes the macOS path fragile for end users.

---

## 7. Unknowns and limits

- **Game-side types.** These are named only through field strings. The container types of `Data.Entities` (key type), `Run`, `Player`, `CurrentState` and the sim class are not recorded. Only `BazaarGameClient.Domain.Models.Cards.ItemCard` is named.
- **Flag semantics.** That bits 10–19 of `CardControllerToBoardTarget` are opponent sockets is inferred from the `bit − 10` code.
- **Which Unity/Mono version the game ships.** It cannot be known statically without the game files. The tracker trusts the 2021.3/2022.3/6000.0 table without checking.
- **Attribute dictionary pairing.** Which dictionary (player or opponent) each of the eight attribute `TryGetValue` calls uses is inferred from the order.
- **Mono struct-member names** in §1.10 are my mapping, not strings in the binary. The numeric offsets are verified.
- **Control flow.** Inside dynamic-binder-heavy methods (`ReadBoardState`, `ManagedJson.ReadDictionary`) I read it from CIL without a decompiler. Fine-grained null and short-circuit details may differ slightly.
- **Runtime behaviour** (e.g. whether `IsStorageOpen` or the rank scan actually succeed against the current build) was not observed.
