# 03: The main island: run header, board, stash and skills

**What to build:** The main island shows the selected Run's header and its board, stash and skills at the selected moment (for now the latest one: live, or the end of a finished Run). See the spec's The moment, Board layout, Badges, Names and Header decisions and the folder-structure record's UI section.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Selecting a Run loads its Events from `history.db`; saved Events for the selected Run are appended as they arrive.
- [ ] `moment` gives the Snapshot shown: the latest Event's that has one; an empty board for a Run with none.
- [ ] The header shows start time, hero, rank and rating, the rating change when ended, the LIVE chip, the outcome, `Day N`, the record and the per-day timeline.
- [ ] Items sit on a 10-slot grid by socket (Small 1, Medium 2, Large 3); collisions and items with no socket go to the first gap; what fits nowhere is counted; `SocketEffect` cards are not drawn.
- [ ] Tiles have a tier border, the name (with the enchantment), badges (damage, shield, heal, burn, poison, regen, ammo, `x2`), the sell price bottom-left and the cooldown bottom-right, with no art; hovering shows a tooltip of everything else.
- [ ] The stash is drawn under the board and folds with its heading, saved in `stash_open`; skills are a row of tier-bordered chips under it.
- [ ] The board header says which moment this is and shows its health, gold and level.
- [ ] Names come from the card catalog, loaded once off the UI thread; until it loads, or for an unknown card, the template id (then instance id) is shown, and the view redraws when it arrives.
- [ ] Tests (board seam): moment lookup, slot layout, badges and tooltip lines, name fallbacks, header values and timeline from Events.
- [ ] By-hand checks are listed under `## Comments`.

## Comments
