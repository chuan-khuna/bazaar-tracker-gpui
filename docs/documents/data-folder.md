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
