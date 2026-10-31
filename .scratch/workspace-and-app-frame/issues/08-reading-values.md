# 08: Reading values

**What to build:** `unityspy-rs` can read real values: static fields, object fields, primitives, strings, arrays, inline structs, enums and the BCL collections. `just spike` reads the `BoardManager` singleton (through its base class) and one `List` or `Dictionary` count, which can be cross-checked against the game's screen. This is verification step 3 in the UnitySpy porting notes. See the spec's Values decision and `docs/rea/unityspy/rust-port.md` §2–4.

**Blocked by:** 07 (Classes and fields)

**Status:** ready-for-agent

- [ ] Statics are read through the vtable; a null vtable is a distinct "not initialised" error.
- [ ] Objects resolve their runtime class from the header and read fields on it.
- [ ] Primitives are read at their correct widths and signedness (`R4` 4 bytes, `R8` 8, `I1` signed, `U1` unsigned, `I`/`U` 8 bytes).
- [ ] Strings are capped at 4,194,304 UTF-16 units and reject negative lengths; arrays, object references, inline structs (offset − 16) and enums (`value__`) are supported.
- [ ] A failed value-type read is an error, never a silent int.
- [ ] `List<T>`, `Dictionary<K,V>` (skipping entries with a negative hash code), `HashSet<T>` and `Nullable<T>` are read, with their field offsets resolved by name.
- [ ] Tests (memory seam): one per value kind and collection, and one per UnitySpy bug in the porting notes' bug list.
- [ ] Run by hand against the game; the values read are recorded under `## Comments` in this file.
