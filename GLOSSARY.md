# Bazaar Tracker

A desktop companion for The Bazaar that follows the game while it is played and keeps a history of each run.

## Language

### Runs

**Run**:
One play-through of The Bazaar, from picking a hero to its victory or defeat.
_Avoid_: Game, match, session

**Live run**:
The run being played right now, which the tracker is following.
_Avoid_: Current run, active run

**Event**:
One thing that happened in a run and was recorded, such as options offered, an item selected, or a combat starting or ending.
_Avoid_: Log line, entry

### What the player has

**Board**:
The player's items in play, placed on ten slots. A Small item takes one slot, a Medium two and a Large three.
_Avoid_: Hand, inventory

**Stash**:
The player's items that are held but not on the board.
_Avoid_: Storage, bag

**Snapshot**:
The board, stash, skills and player stats as they were at one moment of a run, saved with an event.
_Avoid_: Board state, capture

### The app

**Shop odds**:
For each day of a run, the share of items offered at merchants that are of each tier (Bronze, Silver, Gold, Diamond), from Bazaar DB's community-tracked runs. Kept in `history.db`; the app ships a copy.
_Avoid_: Tier odds, drop rates

**Widget**:
A tool listed at the bottom of the runs sidebar that opens in a pop-up, such as Shop odds.
_Avoid_: Tool, plugin
