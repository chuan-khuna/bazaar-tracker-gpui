# Feasibility of rewriting BazaarDBTracker in Rust (GPUI)

## What was discussed

We checked whether BazaarDBTracker 1.0.0-beta.7 can be rebuilt in Rust with GPUI, using the reverse-engineering notes in `docs/rea/bazaar-tracker/` and `docs/rea/unityspy/` as the source.

## Verdict

It is feasible on Windows. Every part of the app has a Rust library that already covers it. The notes record the regexes, SQL statements, timer constants, mono offsets and field paths exactly as the original uses them, which is enough to build from. The main risk is the memory reader: it has to be written from scratch, and nobody has yet checked it against the live game.

## Each part of the app

| Part | Rust libraries | Difficulty | Notes |
|---|---|---|---|
| App shell (single-instance lock, admin relaunch, settings, logging) | `windows`, `serde`, `dirs`, `tracing` | Low | `app-shell.md` §10 maps each piece to Rust. |
| Updates | `velopack` (Rust SDK) | Low | Works the same way as the C# version, using the same feed layout. Open question: does the portable build count as installed, i.e. does it get updates at all? |
| Following `Player.log` and parsing it | `regex`, polling or `notify` | Low | All 7 regexes and the truncation/rotation check are written out. |
| `GameTracker` (the run state machine) | std threads plus channels | Medium | It is subtle: 500/350/1200/4000 ms timers cancelled by generation counters, events built at send time rather than at log time, and runs that start with a `local-` id and are rebound to the real run id later. Hide memory access behind a trait so recorded logs can drive tests. |
| Mono memory reader | `windows` (`ReadProcessMemory`, toolhelp) | High | No Rust library does this, so it is custom code. `unityspy/rust-port.md` lists 15 components and 11 UnitySpy bugs to avoid copying. It also needs readers for `List`, `Dictionary`, `HashSet` and `Nullable`, and a rewrite of `ManagedJson` (the combat-frame serializer). |
| `GameEnums` (reads the game's enum values from `BazaarGameShared.dll`) | — | Medium | Rust has nothing like .NET's MetadataLoadContext. Either write a small parser for the DLL's .NET metadata (the Constant table) or rely on the built-in enum values. The original falls back to built-in values anyway. |
| History database | `rusqlite` with the `bundled` feature | Low | Keeps the existing `history.db` readable. Timestamps must match .NET's format exactly (7 fractional digits) because queries compare them as strings. The rebind code renames rows in an order that breaks under foreign keys. |
| Card catalog | `rusqlite` + `serde_json` | Low | |
| Uploading runs | `reqwest`, `flate2`, `tokio` | Low–medium | GPUI runs its own async executor, not tokio, so either give the uploader its own tokio thread or use GPUI's HTTP client. What the server sends back is unknown. |
| Secret key | `keyring`, plus Windows' `CryptUnprotectData` | Low | `CryptUnprotectData` is only for importing the key an existing install saved. |
| UI | `gpui` + `gpui-component` | Medium | The original's XAML layout files aren't in the binary, so the layout was rebuilt from compiled code and fine visual details will be matched by eye. Core GPUI has no popover, checkbox, password field or tooltip; `gpui-component` covers them. |

Very rough size: about 2–3k lines for the memory reader, 1.5k for the state machine, 2–3k for the UI and 1.5k for everything else.

## macOS

A version that only reads `Player.log` is practical, but it gets just the outline of a run. The real run id, the options offered, boards, gold and HP, combat outcomes and rank all come from memory, and runs without a real id can't be uploaded. Reading memory on macOS needs `task_for_pid` plus entitlement or SIP workarounds, so Mac is treated as log-only. A log-only build must start a new run on each `RunInitialized` line (`log-and-events.md` §5).

## Decided

- The rewrite is feasible and Windows is the primary target.
- Build in this order, so the riskiest part is settled first:
  1. A command-line spike that attaches to the game, finds the 4 classes the tracker needs (`TheBazaar.Data`, `TheBazaar.AppState`, `BoardManager`, `TheBazaar.ClientCache`) and lists their fields.
  2. Log following, parsing and the state machine, tested against recorded logs with memory faked.
  3. The history database and card catalog.
  4. The GPUI interface.
  5. The uploader and updater.

## Open questions

1. **Mono offsets.** The game's Unity version isn't known. UnitySpy upstream has no Unity 6 offset table; the fork's table lists Unity 6 (6000.0.58) but takes it on trust without a check. Run the 4-step check in `unityspy/rust-port.md` §7 against the live game. If the game moves to IL2CPP, the memory reader has to be redesigned.
2. **Admin rights.** Nobody has confirmed they're needed. Opening the game with `PROCESS_VM_READ` alone probably works for a Steam game running as the same user.
3. **The Bazaar DB API.** The upload format is known, but what the server expects isn't. The original sends `User-Agent: BazaarDBCompanion/<ver>`, and copying that would make our client look like theirs. Ask the Bazaar DB team before turning on run sync.
4. **Anti-cheat and terms of service.** Reading game memory carries the same risk as the original does. The notes found no protection on the game today.
