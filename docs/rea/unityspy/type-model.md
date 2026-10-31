# UnitySpy: Type model and value reading

This doc covers how UnitySpy models Mono classes, fields and types, and how it turns raw memory into values. The code is in `src/HackF5.UnitySpy/Detail/` and `ProcessFacade/ProcessFacade.cs`. The offsets referred to by name (`TypeDefinitionFields` …) are listed in [offsets.md](offsets.md).

How each claim is marked:
- **[V]**: verified from source.
- **[I]**: inferred. Mono struct and member names are [I].

## 1. Object model [V]

| UnitySpy class | Mono struct [I] | .NET analogue | File |
|---|---|---|---|
| `AssemblyImage` | `MonoImage` | `Assembly` | `Detail/AssemblyImage.cs` |
| `TypeDefinition` | `MonoClass` | `Type` | `Detail/TypeDefinition.cs` |
| `FieldDefinition` | `MonoClassField` | `FieldInfo` | `Detail/FieldDefinition.cs` |
| `TypeInfo` | `MonoType` | (part of `Type`) | `Detail/TypeInfo.cs` |
| `ManagedClassInstance` | `MonoObject` (a reference-type object) | object | `Detail/ManagedClassInstance.cs` |
| `ManagedStructInstance` | inline value-type bytes | boxed struct | `Detail/ManagedStructInstance.cs` |
| `MemoryObject` | — | base: `Address` + `Image` + typed read helpers relative to `Address` | `Detail/MemoryObject.cs` |

`TypeCode` (`Detail/TypeCode.cs`) mirrors `MonoTypeEnum`, and `MonoClassKind` (`Detail/MonoClassKind.cs`) mirrors Mono's `MonoTypeKind` [I]:

| `MonoClassKind` | Value |
|---|---|
| `Def` | 1 |
| `GTg` | 2 |
| `GInst` | 3 |
| `GParam` | 4 |
| `Array` | 5 |
| `Pointer` | 6 |

## 2. `TypeDefinition` (MonoClass) [V]

**Constructor** (`Detail/TypeDefinition.cs:40-92`). It reads these values eagerly:

| Property | Read |
|---|---|
| `bitFields` | `u32 @ +TypeDefinitionBitFields`. `IsValueType = bit 0x4`, `IsEnum = bit 0x8` (`:98-100`). |
| `fieldCount` | `i32 @ +TypeDefinitionFieldCount` |
| `Name`, `NamespaceName` | `cstr @ ptr @ +TypeDefinitionName / +TypeDefinitionNamespace` |
| `Size` | `i32 @ +TypeDefinitionSize`. This is the `sizes` union: `instance_size` for normal classes, `element_size` for array classes [I]. |
| `VTable` | `ReadPtr(ReadPtr(+TypeDefinitionRuntimeInfo) + TypeDefinitionRuntimeInfoDomainVTables)`, i.e. `runtime_info->domain_vtables[0]` [I]. It is `0` when `runtime_info` is null. |
| `TypeInfo` | A `MonoType` **inline** at `+TypeDefinitionByValArg` (`_byval_arg`). |
| `VTableSize` | `i32 @ +TypeDefinitionVTableSize`, or 0 if there is no runtime info. |
| `ClassKind` | `byte @ +TypeDefinitionClassKind & 7` |

**Lazy members:**
- `Parent` (`+TypeDefinitionParent`), `NestedIn` (`+TypeDefinitionNestedIn`), `FullName` and `Fields`.
- `GetGeneric()`: for `GInst` it reads `ReadPtr(ReadPtr(+TypeDefinitionMonoGenericClass))`, i.e. `generic_class->container_class` [I] (`:249-258`).

**`Fields`** (`GetFields`, `:187-217`) builds the list like this:
- If `fields` (`+TypeDefinitionFields`) is null, it returns the **parent's** fields.
- For `GInst` it takes the generic definition's fields. Otherwise it walks `fieldCount` entries of stride `TypeDefinitionFieldSize`, stopping early at an entry whose `type*` is null.
- It then **appends all parent fields** (recursively), so the list holds the full inheritance chain.
- It **sorts the list by name**. Declaration order is lost.

**`GetField(name, declaringTypeFullName = null)`** (`:171-176`):
- It returns the first field with that exact name. If `declaringTypeFullName` is given, the field's declaring type must also match.
- This is how a shadowed field in a base class is reached. Results are cached in a `ConcurrentDictionary`.
- Lookup is by **exact** name. A C# auto-property's storage must be requested by its compiler-generated name, `<Prop>k__BackingField` [I: C# compiler convention].

`this[string]` on a `TypeDefinition` is `GetStaticValue<dynamic>` (`:139`).

## 3. `FieldDefinition` (MonoClassField) and `TypeInfo` (MonoType) [V]

**`FieldDefinition`** (`Detail/FieldDefinition.cs:20-62`). The layout is hard-coded, not taken from the table:

| Offset | Content |
|---|---|
| `+0` | `MonoType* type`, which becomes a `TypeInfo` |
| `+ptr` | `const char* name` |
| `+2·ptr` | `MonoClass* parent` (not read) |
| `+3·ptr` | `int offset` |

**`TypeInfo`** (`Detail/TypeInfo.cs:15-30`):

| Offset | Content |
|---|---|
| `+0` | `Data` (the `MonoType.data` union: `klass` / `generic_class` / `generic_param` / `array` [I]) |
| `+ptr` | `Attrs` (u32) |

Values decoded from `Attrs`:
- `TypeCode = (Attrs >> 16) & 0xFF`
- `IsStatic = Attrs & 0x10` (`FIELD_ATTRIBUTE_STATIC`)
- `IsConstant = Attrs & 0x40` (`FIELD_ATTRIBUTE_LITERAL`) [I for the attribute names]

`TryGetTypeDefinition` (`:32-46`) resolves `CLASS`, `SZARRAY`, `GENERICINST` and `VALUETYPE` to `GetTypeDefinition(ReadPtr(Data))`:
- For `CLASS`/`VALUETYPE`, `Data` is a `MonoClass*`, and reading its first pointer gives `element_class`, which for a non-array class is the class itself [I].
- For `GENERICINST`, the first pointer of `MonoGenericClass` is `container_class` [I].

## 4. Static fields [V]

`TypeDefinition.GetStaticValue<T>(name)` (`Detail/TypeDefinition.cs:144-169`):
1. It looks up the field among the type's **own** fields (declaring type = this type). It throws `ArgumentException` if the field is missing, and `InvalidOperationException` if it is not static or is a `const`.
2. It computes `staticData = ReadPtr(VTable + offsets.VTable + VTableSize * ptr)`. That is the slot just after the vtable's method pointers, which in Unity's Mono holds the class's static-field block [I].
3. It reads `ReadManaged(field.type, staticData + field.offset)`.
4. Any exception is wrapped in `Exception("Exception received when trying to get static value for field …")`.

Consequences:
- **`const` fields cannot be read.** They have no storage. The value lives only in metadata.
- If the class has **not been initialised** (no `runtime_info`), `VTable` is 0. The read then targets a small absolute address and fails with a read error rather than a clear "not initialised" message. [V for the code; I for the runtime condition]
- A static field declared on a base class must be read from the base `TypeDefinition` (`type.Parent`), because the lookup requires declaring type == this type. [V]
- Thread-static and context-static fields are not handled [I: Mono stores them elsewhere, and UnitySpy has no special case].

## 5. Instance fields [V]

`ManagedObjectInstance.GetValue<T>(name[, declaringType])` (`Detail/ManagedObjectInstance.cs:26-33`) resolves the field on the instance's `TypeDefinition`, then calls `field.GetValue(genericArgs, this.Address)`.

`FieldDefinition.GetValue` (`Detail/FieldDefinition.cs:81-85`) adjusts the offset for value types:

```
offset = (DeclaringType.IsValueType && !static) ? field.offset - 2*ptr : field.offset
value  = ReadManaged(field.type, genericArgs, address + offset)
```

Mono field offsets for instance fields include the `MonoObject` header (`vtable*` and `sync*`, i.e. 2·ptr), even for struct types. A struct stored inline has no header, hence the subtraction [I for the Mono rationale; V for the code].

The runtime type of an object comes from its header:
- **`ManagedClassInstance`** (`Detail/ManagedClassInstance.cs:20-35`): `vtable = ReadPtr(obj)`, then `klass = ReadPtr(vtable)` (`MonoVTable.klass` at +0 [I]).
- Field lookup therefore uses the **actual runtime class**, not the declared field type, so polymorphic fields resolve subclass fields correctly.

**`ManagedStructInstance`** (`Detail/ManagedStructInstance.cs`) has no header. It is a `(TypeDefinition, address)` pair supplied by the caller.

## 6. `ReadManaged`: dispatch on type code [V]

`ProcessFacade.ReadManaged(type, genericArgs, address)` (`ProcessFacade.cs:33-143`) reads the value at `address` (the field slot) according to `type.TypeCode`:

| TypeCode | Hex | Returned | What the code does | Correct? |
|---|---|---|---|---|
| `BOOLEAN` | 0x02 | `bool` | 1 byte, `!= 0` | OK |
| `CHAR` | 0x03 | `char` | 2 bytes | OK |
| `I1` | 0x04 | **`byte`** | 1 byte, unsigned | **Wrong.** `I1` is `sbyte` in Mono. The `[Description]` in `TypeCode.cs` is also swapped. |
| `U1` | 0x05 | **`sbyte`** | 1 byte, signed | **Wrong.** `U1` is `byte`. |
| `I2` / `U2` | 0x06 / 0x07 | `short` / `ushort` | 2 bytes | OK |
| `I4` / `U4` | 0x08 / 0x09 | `int` / `uint` | 4 bytes | OK |
| `I` / `U` | 0x18 / 0x19 | `int` / `uint` | **4 bytes** | **Wrong on x64.** `IntPtr`/`UIntPtr` are 8 bytes. |
| `I8` / `U8` | 0x0A / 0x0B | `long` / `ulong` | 8 bytes | OK |
| `R4` | 0x0C | `float` | reads **`sizeof(char)` = 2 bytes**, converts 4 | **Broken.** The upper 2 bytes come from the zeroed pool buffer, so the value is wrong (`:76-77`). |
| `R8` | 0x0D | `double` | reads **2 bytes**, converts 8 | **Broken** (`:79-80`). |
| `STRING` | 0x0E | `string` or `null` | §7 | OK, but there is no length bound. |
| `SZARRAY` | 0x1D | `object[]` or `null` | §8 | OK |
| `CLASS` | 0x12 | `ManagedClassInstance` or `null` | `ReadPtr(address)`; a null pointer gives `null` | OK |
| `OBJECT` | 0x1C | as `GENERICINST` | `ReadManagedGenericObject` (comment: "works in at least some cases") | Fragile |
| `GENERICINST` | 0x15 | `ManagedStructInstance` **in place** if the container class is a value type, otherwise `ManagedClassInstance` | `:224-235` | OK for the common cases |
| `VALUETYPE` | 0x11 | the enum's `value__` if `IsEnum`, otherwise `ManagedStructInstance` in place | `GetTypeDefinition(type.Data)`. Note that this uses `Data` directly, not `ReadPtr(Data)` (`:274-281`). | OK |
| `VALUETYPE` (failure) | | `int` | **Any exception while building the struct is swallowed and `ReadInt32(address)` is returned instead** (`:88-96`) | Masks errors |
| `VAR` | 0x13 | depends | `ReadManagedVar` (§9) | Experimental ("Really not sure this is the way to do it") |
| `ARRAY`, `ENUM`, `MVAR`, `PTR`, `BYREF`, `FNPTR`, `TYPEDBYREF`, … | | — | throws `ArgumentException("Cannot read values of type …")` | Unsupported |

The float bug is plain in the source: `ReadBufferValue(address, sizeof(char), ConversionUtils.ToSingle)`. The pooled buffer is 16 bytes and zero-filled (see [process-access.md](process-access.md) §2.1). So a `float` keeps only its low 16 bits (low mantissa), which almost always decodes to a tiny denormal near 0, and a `double` similarly keeps only its low 2 bytes. `GetSize` repeats the mistake for `R4`/`R8` (`:319-321`). [V for the code; I for the numeric effect]

## 7. Strings [V]

`ReadManagedString` (`ProcessFacade.cs:252-272`):

```
str    = ReadPtr(fieldSlot);  if 0 → null
length = i32 @ str + 2*ptr                          // MonoString.length (after the vtable and sync header)
chars  = (2*length) bytes @ str + 2*ptr + 4         // UTF-16LE
```

- There is **no bound check** on `length`. A dangling pointer can request gigabytes, or a negative size. The BazaarDBTracker fork added a cap of 4,194,304 (see overview §7).
- Class, field and assembly names are **not** managed strings. They are C strings read with `ReadAsciiString` (1,024 bytes, ASCII, cut at the first NUL). Non-ASCII identifiers would come out mangled [I].

## 8. Arrays (`T[]`, SZARRAY) [V]

`ReadManagedArray` (`ProcessFacade.cs:192-214`):

```
arr          = ReadPtr(fieldSlot);  if 0 → null
arrayClass   = ReadPtr(ReadPtr(arr))                // vtable->klass
elementClass = ReadPtr(arrayClass)                  // MonoClass.element_class                  [I]
count        = i32 @ arr + 3*ptr                    // MonoArray.max_length (after vtable, sync, bounds) [I]
data         = arr + 4*ptr
for i in 0..count:
    result[i] = ReadManaged(elementClass.byval_arg, genericArgs, data + i * arrayClass.Size)
```

- The stride is `arrayClass.Size`, i.e. `sizes.element_size` for an array class [I]. Reference-type elements are therefore pointers, read as `CLASS`. Struct elements are read in place as `ManagedStructInstance`, and enums give their `value__`.
- A `T[]` of `float`/`double` hits the R4/R8 bug.
- **Every element is read eagerly**, one or more syscalls each. A `List<T>` with capacity 10,000 is read in full even if `_size` is 3, because `_items` is read as a whole array.
- There is no count bound. The `max_length` field is read as a 32-bit value even on x64, where it is pointer-sized. Values above 2³¹ would misread, but they do not occur in practice [I].
- Multi-dimensional arrays (`ARRAY`, 0x14) are unsupported.

## 9. Generics [V]

**GENERICINST fields.** When a field's type code is `GENERICINST`, the `FieldDefinition` constructor (`Detail/FieldDefinition.cs:35-61`) collects that field's type arguments into `genericTypeArguments`:

```
genericClass = type.Data                             // MonoGenericClass*                      [I]
container    = ReadPtr(genericClass)                 // container_class
argc         = i32 @ ReadPtr(container + TypeDefinitionGenericContainer) + 4*ptr   // MonoGenericContainer.type_argc [I]
inst         = ReadPtr(genericClass + ptr)           // context.class_inst : MonoGenericInst*
argv[i]      = ReadPtr(inst + 8 + i*ptr)             // MonoGenericInst.type_argv[]  (8 = id + bitfield)  [I]
```

**How the arguments travel.** They are passed down through `GetValue`, `ReadManaged` and the resulting `Managed*Instance`, and from there to that instance's own field reads. A field's own arguments take priority over the ones inherited from its parent instance (`FieldDefinition.cs:83-84`).

**VAR fields** (a field of type `T` inside a generic class) go through `ReadManagedVar` (`ProcessFacade.cs:237-250`):

```
n      = i32 @ type.Data + ptr                       // MonoGenericParam.num                    [I]
offset = Σ_{i<n} (GetSize(args[i].TypeCode) − ptr)
value  = ReadManaged(args[n], null, address + offset)
```

- The offset correction assumes that each earlier type argument changed the field layout by `size − ptr`. That is a heuristic. It is right for some `KeyValuePair`/`Entry`-like shapes and wrong in general, because it ignores alignment and assumes the slot was sized for a pointer [I].
- It also only works when the instance was reached through a field whose declared type carried the arguments. An object reached as `object`/`CLASS` has no arguments, so its `T` fields throw on `args[n]` [I].

**The `TypeDefinition` copy of this code is buggy.** `TypeDefinition`'s constructor has a copy of the argument-collection code that **adds `new TypeInfo(image, monoGenericInsPtr)` for every argument** instead of using the per-argument pointer (`Detail/TypeDefinition.cs:82-86`, compare `FieldDefinition.cs:52-56`). `TypeDefinition.GenericTypeArguments` is therefore wrong for `GENERICINST` classes. No read path in the library uses it. [V]

## 10. Enums [V]

- A `VALUETYPE` whose class has the enum flag is unwrapped to its `value__` field, i.e. its underlying integer (`ProcessFacade.cs:280`).
- There is **no name mapping**. To get enum names you need the enum's static literal fields. Those are `IsConstant` and cannot be read from memory, because constant values live in the metadata tables, not in Mono heap structs [I]. The BazaarDBTracker resolves enum names from the game's DLL on disk instead (`GameEnums`, see [../bazaar-tracker/memory-reading.md](../bazaar-tracker/memory-reading.md) §5).

## 11. Collections

UnitySpy has **no** helpers for BCL collections. [V: there are no `List`, `Dictionary` or `HashSet` references in the library] A caller reads their private fields like any other object. The patterns below are [I], from the Mono/Unity BCL (a .NET Framework 4.x reference-source layout) and from the BazaarDBTracker's usage.

| Collection | Fields to read | Then |
|---|---|---|
| `List<T>` | `_size` (int), `_items` (`T[]`) | Take `_items[0.._size)`. UnitySpy reads the whole backing array. |
| `Dictionary<K,V>` | `count`/`_count`, `entries`/`_entries` (`Entry[]`) | Each entry is a struct `{hashCode, next, key, value}`. Skip entries with `hashCode < 0` (free slots). The field names differ between BCL versions (with or without the `_` prefix) [I], so look them up from the `TypeDefinition` rather than hard-coding them. |
| `HashSet<T>` | `m_count`/`_count`, `m_slots`/`_slots` | `Slot {hashCode, next, value}` |
| `Nullable<T>` | `hasValue`/`has_value`, `value` | Read in place as a struct. |

When a `Dictionary` is reached through a field typed `Dictionary<K,V>`, the generic arguments flow down to the `Entry` struct reads. Its `key`/`value` fields are `VAR`-typed, so they go through the §9 heuristic. For key or value types that are not pointer-sized, reading the entry with raw offsets is more reliable. The BazaarDBTracker does exactly that (memory-reading.md §3.1). [I]

## 12. Caching, `dynamic` and consistency [V]

- **Cached**: `TypeDefinition`s (by address and by name), each type's field list, and field lookups by name.
- **Never cached**: values. Each indexer access re-reads memory.
- All indexers return `dynamic`, so the C# runtime binder resolves chains like `image["A"]["s_inst"]["field"]`. Typos surface as runtime exceptions.
- There is no snapshot or pause. Mono keeps running while a chain is followed, so a structure can change half-way through a read: a `List` can grow (new `_items`, stale `_size`) and an object can become garbage. Unity's Mono uses Boehm GC (`bdwgc`), which **does not move objects** [I]. Pointers therefore stay valid while an object is alive, but a freed object's memory can be reused.
