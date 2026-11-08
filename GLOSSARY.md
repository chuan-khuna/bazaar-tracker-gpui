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

**Scraped run**:
A run by any player, saved from Bazaar DB by the run scraper, with its final board, skills and timeline. The advisor counts these; they are never shown in the runs list.
_Avoid_: Imported run, build

### What the player has

**Board**:
The player's items in play, placed on ten slots. A Small item takes one slot, a Medium two and a Large three.
_Avoid_: Hand, inventory

**Stash**:
The player's items that are held but not on the board.
_Avoid_: Storage, bag

**Offer**:
The cards the game is offering the player at one moment, to buy, take or choose between: a merchant's items, loot, or an encounter's choices.
_Avoid_: Options, choices, selection

**Snapshot**:
The board, stash, skills and player stats as they were at one moment of a run, saved with an event.
_Avoid_: Board state, capture

### The app

**Shop odds**:
For each day of a run, the share of items offered at merchants that are of each tier (Bronze, Silver, Gold, Diamond), from Bazaar DB's community-tracked runs. Kept in `history.db`; the app ships a copy.
_Avoid_: Tier odds, drop rates

**Advisor**:
The agent the player can ask about a build: which item to pick, which items work together, which shop to visit, how to arrange the board and which skill to take. It answers from the card catalog, the odds, and runs scraped from Bazaar DB.
_Avoid_: Bot, assistant, AI

**Conversation**:
One thread of messages between the player and the advisor, about one run. A run can have many conversations.
_Avoid_: Session, chat, thread

**Widget**:
A tool listed at the bottom of the runs sidebar that opens in a pop-up, such as Shop odds.
_Avoid_: Tool, plugin
