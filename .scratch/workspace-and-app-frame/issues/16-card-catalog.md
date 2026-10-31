# 16: Card catalog

**What to build:** `bazaar-tracker` can turn a card id into its name, starting tier, size, type and description, using the game's own card data and never locking the running game out of its files. See the spec's Card catalog decision and `docs/rea/bazaar-tracker/ui.md` §5.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** ready-for-agent

- [ ] Candidates are `GameData.db` and `cards.json` under the game's `LocalLow\Tempo Storm\The Bazaar` folder, with and without `prod\`; the newest existing file is used.
- [ ] `GameData.db` is opened shared-read, copied to a temporary file, read from the copy (`cards` table, JSON blobs), and the copy deleted.
- [ ] If `GameData.db` fails, `cards.json` (an object whose values are arrays of cards) is used.
- [ ] Each card gives name (localised title, else internal name, else id), starting tier, size, type and description; lookup by id ignores case.
- [ ] No data found gives an empty catalog and a log line, not an error.
- [ ] Loading runs once, off the caller's thread.
- [ ] Tests (catalog seam): fixture `GameData.db` and `cards.json`; case-insensitive lookup; newer file wins; broken `GameData.db` falls back; nothing found gives an empty catalog.
