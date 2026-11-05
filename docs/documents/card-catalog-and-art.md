# Card catalog and art

How the tracker turns a card id into a name, and how it takes each card's picture out of the player's own install of The Bazaar. The code is `crates/bazaar-tracker/src/catalog/` and `crates/bazaar-tracker/src/art/`, with the app's side in `src/shell/art.rs` and `src/tracking/names.rs`. The folder the art lands in is laid out in [data-folder.md](data-folder.md#card-art-art); how the game stores its art is in `docs/rea/the-bazaar/card-art.md` and `card-art-extraction.md`. Terms such as Board and Run are as in the [glossary](../../GLOSSARY.md).

## Two game folders

The catalog and the art come from different places.

| Folder | Holds | Found by |
|---|---|---|
| Game data: `%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar` | `GameData.db`, `cards.json`, `Player.log` | `log::path::game_folder_in(user_profile())`; with no `USERPROFILE`, there is no catalog |
| Install: e.g. `…\steamapps\common\The Bazaar` | `TheBazaar_Data\StreamingAssets\aa\` (the Addressables catalog and the bundles), `TheBazaar_Data\boot.config` | `art::find_game_dir(game_dir setting)` |

## The card catalog

`CardCatalog` maps a card id to a `CardInfo`. Lookups lower-case the id, so `TPL-PASTA` and `tpl-pasta` find the same card.

| Field | From the card's JSON | Notes |
|---|---|---|
| `id` | the row's `Id` (or the JSON's `Id` in `cards.json`) | as the game writes it |
| `name` | `Localization.Title.Text`, else `InternalName`, else the id | |
| `starting_tier` | `StartingTier` | `Bronze` … |
| `size` | `Size` | `Small`, `Medium`, `Large` |
| `card_type` | `Type` | `Item`, `Skill`, `SocketEffect`, `CombatEncounter`, … |
| `description` | `Localization.Description.Text` | |
| `art_key` | `ArtKey` | an empty string counts as none |
| `heroes` | `Heroes`, an array | the heroes whose shops sell it; `Common` (`NEUTRAL_HERO`) for a neutral card |

A string or a number is accepted for each text field.

### Where it reads from

`CardCatalog::load(root)` looks in four places for each file, and takes the one written most recently:

```
<root>\prod\cache\<file>
<root>\cache\<file>
<root>\prod\<file>
<root>\<file>
```

1. `GameData.db`, if any copy can be read. It wins even when a `cards.json` is newer.
2. Else `cards.json`: an object whose values are arrays of cards.
3. Else an empty catalog, and the log line `card catalog: no game data found, so card names will not resolve`.

A file that fails is logged at info level (`… could not be read (<why>), trying cards.json`) and the next source is tried. Finding nothing is never an error.

### Reading `GameData.db` from a copy

The game may be running and writing `GameData.db`. So `load_game_data_db`:

1. opens it with a shared read (the standard library's default on Windows lets the game keep writing);
2. copies it to `%TEMP%\bazaar_tracker_gamedata_<pid>_<nanos>.db`;
3. opens the copy read-only and runs `SELECT Id, Data FROM cards`, where `Data` is the card's JSON as a blob or as text;
4. deletes the copy when done, even on failure (`TempCopy` drops it).

A row whose JSON does not parse is skipped. A file with no `cards` table fails as a whole and falls back to `cards.json`.

### Loading off the window's thread

`CatalogHandle::start(root)` loads once, on a `card-catalog` thread. `get()` returns the catalog if it is ready and never blocks; `wait(timeout)` blocks up to `timeout`. The window waits up to 120 s on a background task, then calls `Names::set_catalog`. Until then cards are named by their template id.

### Who uses it

| User | Uses |
|---|---|
| `tracking::names::Names::card` | Name by template id; an unknown card keeps its template id, a card with no template its instance id |
| `Names::items`, `Names::catalog_items` | Every `Item` card by name, with its heroes, for the Tracked items pop-up and loading item odds ([bazaar-db.md](bazaar-db.md)) |
| `Narrator`, Events list, detail island | Card names in sentences and rows ([gpui-app.md](gpui-app.md)) |
| `art::extract` | `Type` and `ArtKey` of every card; `source()` goes into the manifest |

## Finding the install folder

`art::find_game_dir(setting)` returns a folder only if it has a `TheBazaar_Data` folder in it.

1. **`game_dir` setting.** If it holds anything but spaces, it is the only candidate. A folder without `TheBazaar_Data` gives nothing, and the app says `<folder> is not The Bazaar's install folder`.
2. **Steam libraries.** The roots are the `SteamPath` value from `reg query HKCU\Software\Valve\Steam /v SteamPath` (run with no console window) and `C:\Program Files (x86)\Steam`. Each root's `steamapps\libraryfolders.vdf` gives more libraries through its `"path"` lines (`\\` read as `\`), then the root itself. Duplicates are dropped, ignoring case. The first `<library>\steamapps\common\The Bazaar` that is the game wins.
3. **The running game.** If no library has it, the folder of the running `TheBazaar` process's executable (`Process::open_by_name`, then `QueryFullProcessImageNameW`, opened read-only). This finds an install outside Steam while the game runs. Off Windows this step finds nothing.

With no setting and nothing found, the app says "The Bazaar was not found in any Steam library. Start the game and try again, or set game_dir in settings.toml."

## How extraction works

`art::extract(options, catalog, progress)` writes one WebP per distinct picture and a `manifest.json`.

```
GameData.db ─► CardInfo { Type, ArtKey }
                   │ category(Type), file_key(ArtKey)     → items/<key>.webp
                   │ Catalog::resolve(ArtKey)             → asset path + bundle name
                   ▼
StandaloneWindows64/<bundle> ─► UnityFS blocks (LZ4) ─► CAB-<hash> serialized file
                   │ m_Container: asset path → object path ids
                   ▼
asset object ─► Texture2D / Sprite / card MonoBehaviour ─► (other bundles via PPtr)
                   ▼
decode BCn/Crunch/plain ─► flip ─► merge layers ─► shrink to 512 ─► WebP
```

### 1. Plan

- Read `TheBazaar_Data\StreamingAssets\aa\catalog.bin`. An empty card catalog stops here: "the card catalog is empty: start the game once so it writes GameData.db".
- For every card with a category and an `ArtKey`: resolve the key in the Addressables catalog. A key that is not there is a failure (`ArtKey "…" is not in the Addressables catalog`).
- Many cards share an `ArtKey`, so jobs are keyed by output path: one job per file, every card pointing at it in `cards`.
- Read the previous `manifest.json`. If its `format` differs from this run's, or `--force` is given, every image is redrawn. Otherwise only images whose file is missing are written.
- Group the jobs by bundle and report `Progress::Planned { images, to_write, bundles }`.

### 2. Bundles in parallel

- Workers: available cores minus one, kept between 1 and 4. Each holds one bundle (up to a few hundred MB) in memory.
- The biggest bundles go first, so a slow one does not finish last on its own.
- After each bundle a worker reports `Progress::Bundle { done, total, name, written, of }`.
- A failed image is recorded by output path; at the end every card pointing at it moves from `cards` to `failures`.

### 3. The manifest

Written last, atomically, to `<out>\manifest.json`:

| Key | Value |
|---|---|
| `version` | `1` |
| `generated_at` | `2026-10-31T12:00:00Z` (UTC, from `iso_utc`) |
| `build_guid` | `build-guid=` from `TheBazaar_Data\boot.config` |
| `catalog_hash` | the text of `aa\catalog.hash` |
| `game_data` | the `GameData.db` or `cards.json` the catalog came from |
| `format` | `{ "type": "webp", "quality": 82.0, "max_size": 512, "render": 4 }` |
| `key_rule` | `ArtKey, lower-cased, trailing .png removed, \/:*?"<>| replaced with _` |
| `cards` | card id (lower-cased) → `items/<key>.webp` |
| `failures` | card id → why it has no art |

`RENDER_VERSION` (4) is bumped whenever the way images are drawn changes, so the `format` check redraws everything on the next run.

## Key rules and categories

`file_key(ArtKey)`: trim, drop a trailing `.png` (any case), replace each of `\ / : * ? " < > |` with `_`, lower-case.

| `ArtKey` | File stem |
|---|---|
| `Icon_Skill_Backroom.png` | `icon_skill_backroom` |
| `Reward_Nice_D.PNG` | `reward_nice_d` |
| `Assets/Art/X:Y.png` | `assets_art_x_y` |
| `ef57f8899ff555b4d98d03124c1608a7` | unchanged |

`category(Type)`:

| `Type` | Folder | Picture |
|---|---|---|
| `Item` | `items` | the card material's `_MainTex`, opaque |
| `Skill`, `SocketEffect` | `skills` | a sprite or texture, with transparency |
| `EncounterStep` | `rewards` | a sprite or texture |
| `CombatEncounter` | `monsters` | portrait over background |
| `EventEncounter` | `events` | portrait over background |
| `PedestalEncounter` | `pedestals` | portrait over background |
| anything else (`PlayerEffect`, …) | — | no art |

`relative_path(card)` is `<category>/<file_key>.webp`. The app finds a card's file by the same function, so the two never disagree.

## The Addressables catalog

`art::addressables::Catalog::parse` reads the binary `catalog.bin` (magic `0x0DE38942`). Every offset points at data whose byte length is the `u32` just before it. The key table's offset is at byte 8; each entry is 8 bytes: a key and a list of locations.

| Rule | Detail |
|---|---|
| Keys | Only keys whose type name ends in `String` are kept; others are skipped |
| Strings | Bit 31 of a string id marks UTF-16; bit 30 a "dynamic" string stored as a linked list of parts, last first, joined with `/` (keys) or `.` (type names) |
| Location | `{primaryKey, internalId, providerId, dependencies, …}`. The asset path is `internalId`; the bundle is the file name of the first dependency, which must end in `.bundle` |
| `/` in keys | Each key is stored as written and with `/` replaced by `_` |
| `resolve(ArtKey)` | Tries the key as written, without its extension, then with `.png` added |

## UnityFS bundles

`art::unity::bundle` reads the game's bundles in `StandaloneWindows64`: format 8, LZ4HC, flags `0x243`, the engine version stripped to `0.0.0`.

1. Header (big-endian): `UnityFS`, format, player and engine versions, size, compressed and plain size of the block info, flags. An encrypted bundle (`0x400`) is refused.
2. From format 7 the reader aligns to 16. The block info is either next or at the end of the file (`0x80`).
3. The block info, decompressed, holds a 16-byte hash, the blocks `(size, compressed size, flags)` and the files `(offset, size, flags, path)`. Flag `0x200` aligns the data start to 16.
4. Blocks are decompressed and joined; each file is cut out of the result.

| Compression | Handling |
|---|---|
| 0 | stored |
| 2, 3 (LZ4, LZ4HC) | `lz4_flex` |
| 1 (LZMA) | refused |

A bundle holds one serialized file `CAB-<hash>` and its texture data streams (`.resS`, `.resource`). `file_names(head)` reads the file list from the first 64 KB alone, which the cross-bundle index uses.

## Serialized files and type trees

`art::unity::serialized::SerializedFile::parse` reads a `CAB-<hash>`:

- the header (versions 9 and up; 22 and up have 64-bit sizes and offsets; objects need 16 and up);
- the type table, each type with its type tree, since the game's bundles keep them (no class layouts are built in);
- the object table: path id, start, size, type;
- the external files, which name other bundles' serialized files as `archive:/CAB-<hash>/CAB-<hash>`.

`read(object)` walks the type tree into a `Value`: `Bool`, `Int`, `UInt`, `Float`, `Str`, `Bytes`, `List`, `Pair`, `Object`.

| Rule | Detail |
|---|---|
| Node strings | In the tree's own buffer, or, when the offset's top bit is set, in Unity's common strings (`common_strings.rs`, Unity 6000.3) |
| Parents | A node's parent is the nearest node before it one level up |
| Alignment | Meta flag `0x4000` aligns to 4 after the field. An array of aligned elements aligns once, after its last element |
| Byte arrays | `vector<UInt8>`, `SInt8`, `char` and `TypelessData` come back as `Bytes` |
| Strings | A length, the bytes, align to 4 |

Class ids used: `Texture2D` 28, `Material` 21, `MonoBehaviour` 114, `AssetBundle` 142, `Sprite` 213. The `AssetBundle` object's `m_Container` maps each asset path (lower-cased) to its object path ids.

### Following references

A PPtr is `(m_FileID, m_PathID)`. File id 0 is the same file. Any other id names an external `CAB-<hash>`; the first time a reference leaves its bundle, `cab_index` reads the file list of every `.bundle` in the folder to learn which bundle holds which `CAB`. Each other bundle opened is kept for the rest of that bundle's jobs. Decoded textures are cached by `(CAB, path id)`, since many cards share one.

### From asset to picture

An address can list several objects (a texture and its sprite); the first one that gives a picture is used.

| Asset object | Picture |
|---|---|
| `Texture2D` | its pixels |
| `Sprite` | the texture at `m_RD.texture` |
| `MonoBehaviour` with `cardMaterial` (items) | the material's `m_SavedProperties.m_TexEnvs` entry `_MainTex`. The material's other textures (enchantment and effect masks, effect layers) are left out. Alpha is dropped |
| other `MonoBehaviour` (encounters) | `backgroundTextureReference`, then `portraitTextureReference` drawn over it; a layer of another size is stretched to the first (Lanczos3). `encounterCarpet`, the board's floor, is left out |

## Texture2D decoding

`art::unity::texture::decode` reads `m_Width`, `m_Height` (each 1–16384) and `m_TextureFormat`. The pixels are in `image data`, or, when that is empty, in the bundle's `.resS` file named by `m_StreamData.path` at `offset` and `size`.

| `m_TextureFormat` | Decoded by |
|---|---|
| 25 BC7 | `texture2ddecoder::decode_bc7` |
| 10 DXT1 | `decode_bc1` |
| 12 DXT5 | `decode_bc3` |
| 28 DXT1Crunched, 29 DXT5Crunched | `decode_unity_crunch` |
| 1 Alpha8, 3 RGB24, 4 RGBA32, 5 ARGB32, 14 BGRA32 | copied, reordered to RGBA |
| anything else | `texture format N is not supported` |

The decoder writes B, G, R, A; the bytes are reordered to RGBA. Unity stores the bottom row first, so every image is flipped.

## WebP output

`render(picture, opaque, max_size)` shrinks the longest side to 512 px (Lanczos3) and keeps the shape; a smaller image is not enlarged. Item art is opaque (RGB): the alpha of the game's item art is a cut-out mask, not transparency. Everything else keeps its alpha. The `webp` crate encodes it at quality 82.

| Source | Output |
|---|---|
| 1024 × 1024 item | 512 × 512, RGB |
| 1024 × 512 item | 512 × 256, RGB |
| 256 × 256 skill | 256 × 256, RGBA |

Each file is written as `<name>.partial` and renamed over the target, so a crash never leaves half an image; the manifest is written the same way.

## Running it

From a terminal, `just app art` runs `cargo run --release -p bazaar-tracker --example extract_art`:

| Option | Default | Meaning |
|---|---|---|
| `--game <folder>` | `find_game_dir("")` | The install folder |
| `--out <folder>` | `$BAZAAR_TRACKER_DATA_DIR\art`, else `<repo>\.bazaar-tracker\art` | Where to write |
| `--force` | off | Redraw every image, not only missing ones |

Any other argument exits with code 2; a game or profile not found exits with 1. It prints the plan, a line per bundle (`[3/40] skills_assets_all.bundle: 5/5`), then a summary and the eight most common failure reasons with their counts.

From the app, **Settings → Card art → Extract art** (or the "Extract card art" command) calls `shell::art::start(data_dir, game_dir)`:

| Step | Detail |
|---|---|
| Thread | A `card-art` thread finds the install folder, loads its own `CardCatalog` from the game data folder, and calls `art::extract` into `<data>\art` with the default `Options` (512 px, quality 82, no force) |
| Progress | Each `Progress` arrives over an `async_channel`; the overlay shows `Reading the game's card data…`, `Extracting… 3 of 40 bundles`, then `Finishing…` |
| One at a time | A second request while one runs does nothing |
| Done | The app reloads `ArtInfo` and `ArtIndex`, so tiles show art at once, and notes `Card art ready for N cards`; a failure shows `Could not extract card art: <why>` |
| Log | `card art: from <game> into <out>` and `card art: wrote N images (B bytes) in Ss; C cards have art, F do not` |

`ArtInfo::load` reads `manifest.json` for the overlay: `cards` (the size of `cards`), `missing` (the size of `failures`) and `generated_at`, shown as `3310 cards have art · extracted 2026-10-31`. A missing or broken manifest reads as "Not extracted yet."

## How the app shows the art

`tracking::names::ArtIndex::load(<data>\art)` lists every `<category>/<name>.webp` once, at startup and after each extraction. A missing folder gives an empty index. The window never reads a bundle.

`Names::art(template_id)` gives a path when:

1. the catalog knows the card;
2. its `Type` does not end in `Encounter` (monsters, events and pedestals are extracted but not drawn);
3. `relative_path(card)` is in the index.

`tracking::run::board` puts that path on each `Tile` and `SkillChip`, for the board, stash, skills, the Live island's offers and a combat's boards. `ui/run/board.rs` draws it:

| Card | Drawn as |
|---|---|
| Item | the square art stretched to the tile (`ObjectFit::Fill`): Small 0.5 × as wide as tall, Medium square, Large 1.5 ×, as the game does. The name moves to the tooltip, and a dark band keeps the price and cooldown readable |
| Skill | the icon filling a square (`ObjectFit::Cover`) |
| No art | the card's name written on the tile |

## Tests

| File | Covers |
|---|---|
| `crates/bazaar-tracker/src/catalog/catalog_tests.rs` | Fields and heroes from `cards.json`; the name fallback; lookups ignore case; `GameData.db` read from a copy that is then deleted, even while open; the newest of four candidates wins; `GameData.db` beats a newer `cards.json`; broken or table-less databases fall back; nothing found is an empty catalog; odd card JSON is skipped; the handle loads once, off the caller's thread |
| `crates/bazaar-tracker/src/art/art_tests.rs` | `file_key`; each `Type`'s folder; `render` shape, shrink and alpha; `library_paths` from a `libraryfolders.vdf`; `iso_utc`; layers drawn bottom to top |
| `crates/bazaar-tracker/src/art/unity/serialized_tests.rs` | A type tree with a string, an aligned bool and a vector of pairs; byte vectors; common strings by offset |
| `src/shell/art_tests.rs` | Counting cards and failures in a manifest; a broken or missing manifest; progress lines |
| `src/tracking/names_tests.rs` | Names before and after the catalog loads; art found by type and `ArtKey` only when its file is there; encounters without art; the item list |

Run them with `just app test`. `cargo test -p bazaar-tracker the_real_game_data_loads -- --ignored` reads this machine's real game data. Nothing in the tests touches the real bundles; a full extraction is checked by hand with `just app art`.
