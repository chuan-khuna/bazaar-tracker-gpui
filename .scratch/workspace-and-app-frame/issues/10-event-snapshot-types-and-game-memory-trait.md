# 10: Event and Snapshot types, and the game-memory trait

**What to build:** The shared vocabulary of `bazaar-tracker`: the Run, Event and Snapshot types from the glossary, the game's enum values built in, and the game-memory trait the run state machine reads through, with a scriptable fake for tests. This unblocks the run state machine and the history store without needing the live game. See the spec's Domain types and Game memory decisions and `docs/rea/bazaar-tracker/log-and-events.md` §4 and `memory-reading.md` §2, §5.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** ready-for-agent

- [ ] A Snapshot holds the board, stash and skills (each item with instance id, template id, type, tier, size, socket, enchantment, attributes and tags) and the player stats (health, max health, gold, prestige, level, income, day, hour, victories, losses, hero).
- [ ] Event kinds include run started, options presented, selected, sold, combat start, combat end and run ended, each with the fields the notes list; Events serialise to and from JSON.
- [ ] Built-in enum values for tiers, sizes, card types, enchantments, attributes, tags, heroes, inventory sections, sockets and run states turn raw numbers into names; an unknown number keeps its raw value.
- [ ] The game-memory trait has one method per read: board Snapshot, run init (with the real run id), combat result, combat frames, PvP opponent, selected hero, player rank.
- [ ] A fake implementation returns scripted values per call.
- [ ] Tests: enum names for known and unknown values; Event JSON round-trips.
