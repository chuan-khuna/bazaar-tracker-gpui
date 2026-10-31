# 07: Classes and fields

**What to build:** `just spike` looks up `TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager` and `TheBazaar.ClientCache` in `TheBazaarRuntime` and prints each one's fields with offsets, type codes and whether they are static, including fields inherited from parent classes. This is verification step 2 in the UnitySpy porting notes. See the spec's Classes and fields decision and `docs/rea/unityspy/type-model.md`.

**Blocked by:** 06 (Attach and find the Mono domain)

**Status:** ready-for-agent

- [ ] Class lookup walks the image's class cache comparing name and namespace, stops when found, and caches by full name.
- [ ] The class model (name, namespace, parent, nesting, flags, sizes, fields, field count, vtable info, class kind, generic class) is read in one call.
- [ ] The field model gives name, type code, attributes, offset and type data; lookup searches the parent chain and can filter by declaring type.
- [ ] A missing class or field gives a typed error naming it.
- [ ] Tests (memory seam): a fake class cache with several classes, a class with a parent, nested classes, and a field lookup that must climb to the parent.
- [ ] Run by hand against the game; the printed fields are checked against the field paths in `docs/rea/bazaar-tracker/memory-reading.md` §2, and any mismatch is recorded under `## Comments` in this file.
