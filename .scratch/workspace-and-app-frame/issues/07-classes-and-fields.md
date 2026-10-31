# 07: Classes and fields

**What to build:** `just spike` looks up `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache` in `TheBazaarRuntime` and prints each one's fields with offsets, type codes and whether they are static, including fields inherited from parent classes. This is verification step 2 in the UnitySpy porting notes. See the spec's Classes and fields decision and `docs/rea/unityspy/type-model.md`.

**Blocked by:** 06 (Attach and find the Mono domain)

**Status:** ready-for-human

- [x] Class lookup walks the image's class cache comparing name and namespace, stops when found, and caches by full name.
- [x] The class model (name, namespace, parent, nesting, flags, sizes, fields, field count, vtable info, class kind, generic class) is read in one call.
- [x] The field model gives name, type code, attributes, offset and type data; lookup searches the parent chain and can filter by declaring type.
- [x] A missing class or field gives a typed error naming it.
- [x] Tests (memory seam): a fake class cache with several classes, a class with a parent, nested classes, and a field lookup that must climb to the parent.
- [ ] Run by hand against the game; the printed fields are checked against the field paths in `docs/rea/bazaar-tracker/memory-reading.md` §2, and any mismatch is recorded under `## Comments` in this file.

## Comments

Implemented by an agent; everything except the by-hand run is done and tested against fake class caches (memory seam). `just spike` now also prints the fields of `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache` (offset, type code, static or const, declaring class, parents included). The game was not running, so still to do by hand: run `just spike` against the game, check the output against the field paths in `docs/rea/bazaar-tracker/memory-reading.md` §2, and record any mismatch here.

The class model also reads `_byval_arg` (offset 0xB8) because array elements are read by their class's type; `MonoOffsets` gained `class_byval_arg` for it. For a generic instance the field count comes from the generic definition, as Mono does.
