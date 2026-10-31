# 06: Attach and find the Mono domain

**What to build:** With The Bazaar running, `just spike` attaches to the game read-only, finds its Mono runtime and prints the assemblies loaded in the root domain, which must include `TheBazaarRuntime`, `mscorlib` and `UnityEngine.CoreModule`. This is verification step 1 in the UnitySpy porting notes and settles open question 1 of the feasibility record (the Mono offsets). See the spec's Attaching and Errors decisions and `docs/rea/unityspy/` (process-access, mono-discovery, offsets, rust-port §2–5).

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** ready-for-agent

- [ ] The game process is found by name and opened with `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION` only.
- [ ] Raw reads go through the `Memory` trait; a Windows implementation uses `ReadProcessMemory` and handles partial reads as errors.
- [ ] The Mono module is found case-insensitively among the process's modules.
- [ ] `mono_get_root_domain` is found by reading only the PE headers and export directory, resolving through `AddressOfNameOrdinals`.
- [ ] The root-domain instruction bytes (`48 8B 05` … `C3`) are checked before the `rel32` is trusted.
- [ ] A `MonoOffsets` table is chosen by the game exe's `FileVersion`; the 2021/2022 x64 table and the fork's Unity 6 table are included.
- [ ] The domain's assembly list is walked and each assembly's name printed by `just spike`.
- [ ] A missing process, module, export or assembly gives a typed error naming what was missing.
- [ ] Tests (memory seam): PE export lookup from a buffer where a name's ordinal differs from its index; root-domain decoding with good and bad bytes; walking a fake assembly list.
- [ ] Run by hand against the game; the printed assembly list and the Unity version found are recorded under `## Comments` in this file.
