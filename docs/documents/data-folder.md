# Data folder

Where the app keeps `settings.toml`, `history.db`, `logs/` and `raw_data/`, and how to move it.

## Where it is

| Build | Folder |
|---|---|
| Debug (`just tracker run`) | `<repo>/.bazaar-tracker/` (gitignored) |
| Release | `%APPDATA%\BazaarTracker\` |
| Either, overridden | `BAZAAR_TRACKER_DATA_DIR` |

`BAZAAR_TRACKER_DATA_DIR` wins over both defaults. An empty value counts as unset. The folder is
created if it is missing. The rules are in `apps/tracker/src/shell/data_dir.rs`. A debug build fixes
`<repo>` at compile time: two folders up from the app's manifest folder, `apps/tracker`.

## Recommended: keep the release build's data in the repo too

When you build the app from this repo, point the release build at the same `.bazaar-tracker/`
folder the debug build uses. Debug and release then share one history and one set of settings, and
nothing is written to `%APPDATA%`.

`just tracker start` does this for you: it starts `target/release/bazaar-tracker-gpui.exe` (built by
`just tracker build`) with `BAZAAR_TRACKER_DATA_DIR` set to `<repo>/.bazaar-tracker`.

Run the debug and release builds one at a time: both write to the same `history.db`.

## Setting the variable yourself

For one PowerShell session, then start the app from it:

```powershell
$env:BAZAAR_TRACKER_DATA_DIR = "D:\code\bazaar-tracker-gpui\.bazaar-tracker"
.\target\release\bazaar-tracker-gpui.exe
```

For one Command Prompt session:

```bat
set BAZAAR_TRACKER_DATA_DIR=D:\code\bazaar-tracker-gpui\.bazaar-tracker
target\release\bazaar-tracker-gpui.exe
```

For your user account, permanently (open a new terminal afterwards, or sign out and in again for
apps started from Explorer):

```powershell
[Environment]::SetEnvironmentVariable("BAZAAR_TRACKER_DATA_DIR", "D:\code\bazaar-tracker-gpui\.bazaar-tracker", "User")
```

To remove it again:

```powershell
[Environment]::SetEnvironmentVariable("BAZAAR_TRACKER_DATA_DIR", $null, "User")
```

You can also set it in **Settings → System → About → Advanced system settings → Environment
Variables**, under *User variables*.

Use your own repo path in place of `D:\code\bazaar-tracker-gpui`.

## Raw data for import (`raw_data/`)

`raw_data/` holds files prepared for the app to import. The scrapers (`apps/scraper/`) write to
`.bazaar-tracker/raw_data/scraper/` in the repo: `odds/` for item odds (read by Import item odds…,
see [odds-scraper.md](odds-scraper.md)) and `builds/` for scraped runs (imported into `history.db` by
`just advisor import`, see [advisor.md](advisor.md#the-importer); the files are described in
[scrapers.md](scrapers.md)). The path is fixed in the scrapers, not read from
`BAZAAR_TRACKER_DATA_DIR`; pass `--out` to save elsewhere.

## Backups of `history.db` (`history.v<N>.bak.db`)

Before the app migrates an older `history.db` to a newer schema, it copies the file as it was to
`history.v<N>.bak.db` in the same folder, where N is the old schema version (`.bak.2.db`, `.bak.3.db`,
… if that name is taken). The app never deletes these copies; remove them yourself once the new
build works. To go back to an older build, close the app and put the copy in place of
`history.db`. See [history-db.md](history-db.md#migrations-and-user_version).

## Card art (`art/`)

`art/` holds card images taken from your own install of the game. The app does not ship them:
they are Tempo Storm's art, for showing locally only, so never commit or share them. Build the
folder with **Settings → Card art → Extract art** (or "Extract card art" in the menu or the
command palette), or from a terminal:

```powershell
just tracker art
just tracker art --game "H:\SteamLibrary\steamapps\common\The Bazaar"
```

The game is found in the Steam libraries. If it is somewhere else, set `game_dir` in
`settings.toml` for the app, or pass `--game` to `just tracker art`. The extractor is
`bazaar_tracker::art`, in Rust, so it needs nothing installed. It writes into this data folder, so
`BAZAAR_TRACKER_DATA_DIR` moves it too, or pass `--out`. The first run takes about 30 s and writes
about 65 MB. Later runs only write images that are missing, so run it again after a game patch.
`--force` redraws every image.

```
art/
  manifest.json          card id -> image, plus the game build it came from and what failed
  items/<key>.webp       opaque (the alpha in the game's item art is a mask, not transparency)
  skills/<key>.webp      skills and socket effects, with transparency
  rewards/<key>.webp     encounter steps
  monsters/<key>.webp    combat encounters (portrait drawn over background)
  events/<key>.webp      event encounters (portrait drawn over background)
  pedestals/<key>.webp   pedestal encounters (portrait drawn over background)
```

The category is the card's `Type`. `<key>` is the card's `ArtKey` from `GameData.db`,
lower-cased, with a trailing `.png` removed and any of `\/:*?"<>|` replaced with `_`. Many cards
share one `ArtKey`, so they share one file. Images are WebP, at most 512 px on the longest side. Item art is kept
square, as the game stores it. The board stretches it to the card's shape when it draws it, as the
game does: a small card is half as wide as it is tall, a medium one square, a large one half again
as wide.
Safe to delete: run `just tracker art` to rebuild. How the game stores the art is in
`docs/rea/the-bazaar/card-art.md`.
