# 06: Attach and find the Mono domain

**What to build:** With The Bazaar running, `just spike` attaches to the game read-only, finds its Mono runtime and prints the assemblies loaded in the root domain, which must include `TheBazaarRuntime`, `mscorlib` and `UnityEngine.CoreModule`. This is verification step 1 in the UnitySpy porting notes and settles open question 1 of the feasibility record (the Mono offsets). See the spec's Attaching and Errors decisions and `docs/rea/unityspy/` (process-access, mono-discovery, offsets, rust-port §2–5).

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** resolved

- [x] The game process is found by name and opened with `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION` only.
- [x] Raw reads go through the `Memory` trait; a Windows implementation uses `ReadProcessMemory` and handles partial reads as errors.
- [x] The Mono module is found case-insensitively among the process's modules.
- [x] `mono_get_root_domain` is found by reading only the PE headers and export directory, resolving through `AddressOfNameOrdinals`.
- [x] The root-domain instruction bytes (`48 8B 05` … `C3`) are checked before the `rel32` is trusted.
- [x] A `MonoOffsets` table is chosen by the game exe's `FileVersion`; the 2021/2022 x64 table and the fork's Unity 6 table are included.
- [x] The domain's assembly list is walked and each assembly's name printed by `just spike`.
- [x] A missing process, module, export or assembly gives a typed error naming what was missing.
- [x] Tests (memory seam): PE export lookup from a buffer where a name's ordinal differs from its index; root-domain decoding with good and bad bytes; walking a fake assembly list.
- [x] Run by hand against the game; the printed assembly list and the Unity version found are recorded under `## Comments` in this file.

## Comments

Implemented by an agent. Everything except the by-hand run is done and tested against byte buffers (the memory seam) and, for process, module and version lookup, against the test process itself. The game was not running, so the by-hand run is still to do: start The Bazaar and run `just spike`. Record here the printed assembly list and the Unity version found. `just spike` without the game prints `dump-classes: process 'TheBazaar' is not running` and exits 1.

The Unity 6 table carries the 2021/2022 values and is marked `verified: false`; `just spike` prints "[not verified against a real game]" when it is chosen. The exe version is read from the numeric `VS_FIXEDFILEINFO` (so `2022.3.42f1` reads as 2022.3.42).

By-hand run, 2026-10-31 (`just spike`, game at the main menu):
- Process `TheBazaar` found; exe version 6000.3.11.239, so Unity 6000.3.11. The Unity 6 x64 table was chosen (still flagged unverified).
- `mono-2.0-bdwgc.dll` found and the root domain decoded.
- 214 assemblies listed, including `mscorlib`, `UnityEngine.CoreModule` and `TheBazaarRuntime`, plus `BazaarGameClient`, `BazaarGameShared` and `BazaarBattleService`.
- The Unity 6 offsets held for everything this run uses: domain, assembly list, image class cache, class model, fields, and static data on an initialised class (`TheBazaar.Data`). The generic-container and inline-struct offsets are not exercised yet (tickets 08 and 11).
