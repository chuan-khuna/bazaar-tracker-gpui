# Data folder

Where the app keeps `settings.toml`, `history.db` and `logs/`, and how to move it.

## Where it is

| Build | Folder |
|---|---|
| Debug (`just run`) | `<repo>/.bazaar-tracker/` (gitignored) |
| Release | `%APPDATA%\BazaarTracker\` |
| Either, overridden | `BAZAAR_TRACKER_DATA_DIR` |

`BAZAAR_TRACKER_DATA_DIR` wins over both defaults. An empty value counts as unset. The folder is
created if it is missing. The rules are in `src/shell/data_dir.rs`.

## Recommended: keep the release build's data in the repo too

When you build the app from this repo, point the release build at the same `.bazaar-tracker/`
folder the debug build uses. Debug and release then share one history and one set of settings, and
nothing is written to `%APPDATA%`.

`just app` does this for you: it starts `target/release/bazaar-tracker-gpui.exe` (built by
`just build`) with `BAZAAR_TRACKER_DATA_DIR` set to `<repo>/.bazaar-tracker`.

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

## Card art (`art/`)

`art/` holds card images taken from your own install of the game. The app does not ship them:
they are Tempo Storm's art, for showing locally only, so never commit or share them. Build the
folder with:

```powershell
just art --game "H:\SteamLibrary\steamapps\common\The Bazaar"
```

`--game` can be left out if `BAZAAR_GAME_DIR` is set or the game is in Steam's default library.
The script is `scripts/extract_art.py`, run through `uv`, which fetches its Python packages. It
writes into this data folder, so `BAZAAR_TRACKER_DATA_DIR` moves it too, or pass `--out`. The
first run takes about 30 s and writes about 75 MB. Later runs only write images that are missing,
so run it again after a game patch. Add `--force` to rewrite every image.

```
art/
  manifest.json          card id -> image, plus the game build it came from and what failed
  items/<key>.webp       opaque (the alpha in the game's item art is a mask, not transparency)
  skills/<key>.webp      skills and socket effects, with transparency
  rewards/<key>.webp     encounter steps
  monsters/<key>.webp    combat encounters (portrait, else background)
  events/<key>.webp      event encounters (portrait, else background)
  pedestals/<key>.webp   pedestal encounters
```

The category is the card's `Type`. `<key>` is the card's `ArtKey` from `GameData.db`,
lower-cased, with a trailing `.png` removed and any of `\/:*?"<>|` replaced with `_`. Many cards
share one `ArtKey`, so they share one file. Images are WebP, at most 512 px on the longest side.
Safe to delete: run `just art` to rebuild. How the game stores the art is in
`docs/rea/the-bazaar/card-art.md`.
