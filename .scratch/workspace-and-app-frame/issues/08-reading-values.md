# 08: Reading values

**What to build:** `unityspy-rs` can read real values: static fields, object fields, primitives, strings, arrays, inline structs, enums and the BCL collections. `just spike` reads the `BoardManager` singleton (through its base class) and one `List` or `Dictionary` count, which can be cross-checked against the game's screen. This is verification step 3 in the UnitySpy porting notes. See the spec's Values decision and `docs/rea/unityspy/rust-port.md` §2–4.

**Blocked by:** 07 (Classes and fields)

**Status:** ready-for-human

- [x] Statics are read through the vtable; a null vtable is a distinct "not initialised" error.
- [x] Objects resolve their runtime class from the header and read fields on it.
- [x] Primitives are read at their correct widths and signedness (`R4` 4 bytes, `R8` 8, `I1` signed, `U1` unsigned, `I`/`U` 8 bytes).
- [x] Strings are capped at 4,194,304 UTF-16 units and reject negative lengths; arrays, object references, inline structs (offset − 16) and enums (`value__`) are supported.
- [x] A failed value-type read is an error, never a silent int.
- [x] `List<T>`, `Dictionary<K,V>` (skipping entries with a negative hash code), `HashSet<T>` and `Nullable<T>` are read, with their field offsets resolved by name.
- [x] Tests (memory seam): one per value kind and collection, and one per UnitySpy bug in the porting notes' bug list.
- [ ] Run by hand against the game; the values read are recorded under `## Comments` in this file.

## Comments

Implemented by an agent; everything except the by-hand run is done and tested against fake memory laid out like the real structures. Tests cover each value kind, each collection, and the UnitySpy bugs in the porting notes: `R4`/`R8` widths, `I1`/`U1` signedness, 8-byte `I`/`U`, the silent int on a failed value type, unbounded and negative strings, ordinal-blind export lookup (ticket 06), unchecked root-domain bytes (ticket 06), the fixed 1,024-byte C string (ticket 06 reader tests), the missing-module `null` (typed error), and struct offsets taken from the instantiated class instead of the generic-argument heuristic (the dictionary and hash set tests). The `TypeDefinition.GenericTypeArguments` bug (item 10) is not ported, so it has no test.

Still to do by hand: run `just spike` against the game. It now ends with the `BoardManager` singleton (read through its base class) and the entry count of `TheBazaar.Data.Entities`; cross-check the count against the screen and record the values here.

A field whose type is still a generic parameter (`VAR`) reads as `UnresolvedGeneric` rather than being guessed. If the real game's `Dictionary` entry classes turn out to have `VAR` field types, the by-hand run will show it, and the game layer (ticket 11) can fall back to the raw entry layout the original tracker used (hash at 0, key at 8, value at 8 + key size).
