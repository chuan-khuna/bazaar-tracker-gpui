# 16: Card catalog

**What to build:** `bazaar-tracker` can turn a card id into its name, starting tier, size, type and description, using the game's own card data and never locking the running game out of its files. See the spec's Card catalog decision and `docs/rea/bazaar-tracker/ui.md` §5.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** resolved

- [x] Candidates are `GameData.db` and `cards.json` under the game's `LocalLow\Tempo Storm\The Bazaar` folder, with and without `prod\`; the newest existing file is used.
- [x] `GameData.db` is opened shared-read, copied to a temporary file, read from the copy (`cards` table, JSON blobs), and the copy deleted.
- [x] If `GameData.db` fails, `cards.json` (an object whose values are arrays of cards) is used.
- [x] Each card gives name (localised title, else internal name, else id), starting tier, size, type and description; lookup by id ignores case.
- [x] No data found gives an empty catalog and a log line, not an error.
- [x] Loading runs once, off the caller's thread.
- [x] Tests (catalog seam): fixture `GameData.db` and `cards.json`; case-insensitive lookup; newer file wins; broken `GameData.db` falls back; nothing found gives an empty catalog.

## Comments

Implemented by an agent.

- `catalog::CardCatalog::load(<The Bazaar folder>)` never fails: it returns an empty catalog and logs a line if nothing could be read. `get(id)` ignores case; `CardInfo` has the id, name (localised title, else internal name, else id), starting tier, size, type and description. `catalog::CatalogHandle::start(root)` loads once on its own thread; `get()` never blocks and `wait(timeout)` does.
- Candidates, per file name: `prod\cache\`, `cache\`, `prod\` and the folder itself. The original's notes only name the two `cache` folders; the ticket's "with and without prod\" is covered by also trying the folder itself. The newest existing `GameData.db` is tried first; if it cannot be read (not a database, no `cards` table), the newest `cards.json` is used.
- `GameData.db` is opened with the standard library's default sharing (which lets the game keep it open for writing), copied to a temporary file in the temp folder, read from the copy and the copy deleted (a test checks nothing is left behind).
- Checked against the real data on this machine with `cargo test -p bazaar-tracker catalog::tests::the_real_game_data_loads -- --ignored --nocapture`: 3,386 cards from `prod\cache\GameData.db`, with names, tiers, sizes and types filled in. Many items have no `Localization.Description` in their card data, so their description is `None`.
- Fixture: `tests/fixtures/catalog/cards.json`; the `GameData.db` fixtures are built by the tests with SQLite.
