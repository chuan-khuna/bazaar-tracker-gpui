# 10: Event and Snapshot types, and the game-memory trait

**What to build:** The shared vocabulary of `bazaar-tracker`: the Run, Event and Snapshot types from the glossary, the game's enum values built in, and the game-memory trait the run state machine reads through, with a scriptable fake for tests. This unblocks the run state machine and the history store without needing the live game. See the spec's Domain types and Game memory decisions and `docs/rea/bazaar-tracker/log-and-events.md` §4 and `memory-reading.md` §2, §5.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** resolved

- [x] A Snapshot holds the board, stash and skills (each item with instance id, template id, type, tier, size, socket, enchantment, attributes and tags) and the player stats (health, max health, gold, prestige, level, income, day, hour, victories, losses, hero).
- [x] Event kinds include run started, options presented, selected, sold, combat start, combat end and run ended, each with the fields the notes list; Events serialise to and from JSON.
- [x] Built-in enum values for tiers, sizes, card types, enchantments, attributes, tags, heroes, inventory sections, sockets and run states turn raw numbers into names; an unknown number keeps its raw value.
- [x] The game-memory trait has one method per read: board Snapshot, run init (with the real run id), combat result, combat frames, PvP opponent, selected hero, player rank.
- [x] A fake implementation returns scripted values per call.
- [x] Tests: enum names for known and unknown values; Event JSON round-trips.

## Comments

Implemented by an agent.

- Types live where they are used: `memory::snapshot` (Item, PlayerStats, Snapshot, BoardState) and `run::event` (Run, Event, EventKind, Outcome). `enums` holds the built-in values as one module per game enum (`tier`, `card_size`, `card_type`, `hero`, ...), each with `name`, `name_or_raw` and `value_of`. Items and Events carry names as text; a number with no name is kept as its digits.
- Events are our own JSON (snake_case keys, a `type` tag, absent values left out), not the original's PascalCase. `Rerolled` is included beside the kinds the ticket lists because the notes have it. `CombatFrames` (the ManagedJson tree) is out of scope; `GameMemory::read_combat_frames` returns only the frame count, used to tell a stale simulation from the one just played.
- `BoardState` is what `read_board` returns: both sides' Snapshots plus the run state name, the selection set and every card the read saw (`entities`), which the run state machine uses to look up a card's tier and size from its id.
- `Selected` has `options_event_id` so a choice links to the options it came from (ticket 12).
- `FakeMemory` is public (not test-only) so the tracker service tests and examples can use it: `push_*` queues per call and the last result repeats, `set_*` replaces the script, `fail_*` makes reads error, and each read has a call counter.
