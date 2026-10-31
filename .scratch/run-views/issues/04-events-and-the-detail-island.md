# 04: The events list, and event selection driving the board and detail

**What to build:** The main island lists the selected Run's Events; selecting one moves the board, stash and skills to the nearest Snapshot at or before it and fills the detail island with what is specific to the Event. See the spec's The moment, Event rows, Detail and Scrolling decisions.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] One row per Event: `m:ss` into the Run, `D<day> H<hour>` (day only when it changes), the kind and a short description; Selected and Sold rows are sub-rows; combat results use the win or loss colour.
- [ ] Clicking an Event selects it (highlighted); clicking it again clears the selection. Selection is per Run and is cleared when another Run is selected.
- [ ] With an Event selected, `moment` gives the nearest Snapshot at or before it (CombatStart contributes its player side), and the board header names that Event.
- [ ] The list scrolls, and stays at the bottom as Events arrive while no Event is selected and the Run is live.
- [ ] The detail island shows: for Options, the options as cards with the pick marked and "Picked 1 of N"; for Selected, its options Event's cards; for CombatStart, the opponent's facts and both boards; for CombatEnd, the result and the opponent board of its CombatStart; for Run started, Run ended and Rerolled, their facts. It says so if the Event is no longer in the Run.
- [ ] The empty states remain when no Event is selected.
- [ ] Tests (events seam): row text for every kind, sub-rows, day shown on change, `m:ss`, nearest-Snapshot lookup from an Event with none of its own, detail for each kind including a missing options or start Event, selection clearing.
- [ ] By-hand checks are listed under `## Comments`.

## Comments
