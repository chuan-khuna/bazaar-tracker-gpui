# The Bazaar: Extracting card art for this app

This doc covers how to turn the game's bundles into one image per card id that the tracker can show. Read [card-art.md](card-art.md) first for the formats and the `ArtKey` mapping.

Target: the same build as card-art.md (Unity 6000.3.11f1, `catalog.bin` SHA-256 `d8cb8303…4b34`, `TheBazaarRuntime.dll` SHA-256 `f9ab344d…e434`). REA evidence IDs: `ev_a58c4efb0d7c09e9dc40bcda0385e4d70588faddc3e5e8ce996d2a3db71d5ec9`, `ev_ec63bafac0cb81ea2cca25ed4eb1c426a1c8e303e678cb7a0a3dc0c3181aeae9`.

How each claim is marked:
- **[V]**: verified by running the pipeline on this machine into a scratch directory (outside the repo, Python run with `-I`, no writes to the game folder).
- **[I]**: inferred or recommended, not yet built.

> **Legal and ethical note.** Card art, icons and portraits are the intellectual property of Tempo Storm. Extracting them from a copy you own, so that this app can show them **locally** to you, is a personal-use convenience. Do not commit extracted images to this repository. Do not bundle them in releases, upload them, or redistribute them in any form. Each user must run the extraction against their own install. The `card-art/` output folder belongs in the gitignored data folder, never in the source tree.

> **Built.** The pipeline below is now `scripts/extract_art.py` (`just art`). It writes `<data dir>/art/<category>/<key>.webp` plus `art/manifest.json`, not the `card-art/<card_id>.webp` layout proposed in §3.1 and §4. The folder layout and key rule are in `docs/documents/data-folder.md`. It also does the encounter background fallback. First run on this machine: 3,295 of 3,386 cards with art, 2,300 images, 73 MB, 31 s with 4 worker processes. Most of the 31 cards without art are the unresolved `ArtKey`s in card-art.md §2.3, plus 15 textures held in another bundle.

## 1. Recommendation

Use **an offline extraction step that writes `<card_id>.webp` into the app's data folder**, and have the app only read those files. Do not read bundles at runtime from Rust. [I]

Reasons:
1. The work is a one-off per game patch: 171 s for the whole catalog (§3). Reading bundles at runtime would mean decompressing 1.9 GB of LZ4HC bundles, parsing Unity 6000.3 typetrees for `CardAssetDataSO` and `Material`, and decoding BC7 and Crunch, all inside the UI process.
2. UnityPy already handles everything this build needs: UnityFS v8, stripped version, typetrees, BC7, DXT5Crunched, and `.resS` streams. It took one config line (§3.2). The Rust crates are young (§2), and the catalog parser would have to be written anyway.
3. The output is small. At ≤512 px it is about 107 MB of WebP for 3,059 cards, and it costs the app nothing beyond an image load per tile.
4. It isolates the legal surface. The app ships no art and no extractor dependency, and the user runs the step against their own install.

## 2. Tool options

| Tool | Language | Fit for this build | Notes |
|---|---|---|---|
| **UnityPy 1.25.4** | Python | **Works** [V] | Needs `UnityPy.config.FALLBACK_UNITY_VERSION = "6000.3.11f1"` because the bundles strip the engine version. Decodes BC7 and Crunch through `texture2ddecoder`. Reads typetrees, so `CardAssetDataSO.cardMaterial` and `Material.m_SavedProperties.m_TexEnvs` come out as dicts. **Does not read the Addressables `catalog.bin`.** The ~80-line parser in card-art.md §2.2 fills that gap. Run it with `python -I` from a venv. |
| AssetRipper | C# GUI/CLI | Should work [I] | Exports a whole Unity project (all 931 bundles to PNG, YAML and C# stubs). That is good for exploring but heavy for a per-patch job: it writes far more than we need, and it does not map card ids. Not tested here. |
| AssetStudio (and forks such as AssetStudioMod) | C# GUI | Likely works with a version override [I] | Interactive browsing and export by container path. The original project is unmaintained, and Unity 6 support exists only in forks. There is no catalog support. Not tested here. |
| `unity-asset-binary` / `unity-asset` 0.2.0 | Rust | Unproven [I] | `unity-asset-binary` is a parser-only crate for UnityFS (LZ4 and LZMA), SerializedFile and WebFile. Decoding lives in `unity-asset-decode`. The project marks UnityFS support as work in progress and does not name BC7. It was not tried against Unity 6000.3 typetrees. |
| `texture2ddecoder` | Rust | Works for pixels [I] | The same decoder UnityPy uses (BCn incl. BC7, ETC, ASTC, Unity Crunch). It does not parse Unity files. A Rust pipeline would pair it with a UnityFS reader. |
| bazaardb.gg images | HTTP | Context only | Third-party site keyed by card id or name. It removes the extraction step but adds a network dependency, depends on another party's hosting and terms, and could drift from the installed patch. **Not recommended for this app.** It is noted only as an alternative people use. |

There is no maintained `unity-rs` crate that covers the full UnityFS → typetree → BC7 path end to end, as of this writing [I].

## 3. The pipeline

### 3.1 Steps

1. **Read the card list.** Open `GameData.db` the way the catalog already does (`SELECT Id, Data FROM cards`, shared-read and copied to a temp file), and take `Id`, `Type` and `ArtKey` from each JSON. [V]
2. **Resolve keys.** Parse `StreamingAssets/aa/catalog.bin` (card-art.md §2.2) into `key → (internalId, type, bundle)`. Index each key both as stored and with `/`→`_`. Try `ArtKey`, then `ArtKey` without its extension, then `ArtKey + ".png"`. 3,310 of 3,386 cards resolve. [V]
3. **Group by bundle.** Open each bundle once with UnityPy, and index `env.container` by lower-cased path. [V]
4. **Follow the chain per card type** [V]:
   - `CardAssetDataSO` (items): `read_typetree()["cardMaterial"]` → Material → `m_TexEnvs["_MainTex"]` → Texture2D. **Convert to RGB and drop the alpha** (card-art.md §2.5).
   - `Sprite` (skills, rewards): `m_RD.texture` → Texture2D. Keep RGBA.
   - `Texture2D` (some rewards): use as is.
   - `CombatAssetDataSO` / `MerchantAssetDataSO` / `QuestAssetDataSO`: `portraitTextureReference` (a Sprite, so follow `m_RD.texture`). If its `m_PathID` is 0, **fall back to `backgroundTextureReference`**. The fallback is not yet implemented and accounts for the 250 failures in §3.3.
5. **Decode, downscale and write.** Decode with `tex.image`, downscale to at most 512 px with Lanczos, and write `<data dir>/card-art/<card_id lower-case>.webp` (quality 82). Decode each shared texture once, because many cards share an art key (the top key is used by 90 cards). [V]
6. **Write a manifest.** Write `card-art/manifest.json` with `{ catalog_hash, build_guid, gamedata_mtime, generated_at, cards: { id: { art_key, source_path, kind } } }`. [I]

### 3.2 Core of the prototype

The prototype that produced the numbers in §3.3 is about 70 lines of Python. It was run from a scratch dir and is not committed. Its essential calls:

```python
import UnityPy
UnityPy.config.FALLBACK_UNITY_VERSION = "6000.3.11f1"   # bundles carry "0.0.0"

env  = UnityPy.load(bundle_path)
objs = {o.path_id: o for o in env.objects}
obj  = {p.lower(): o for p, o in env.container.items()}[internal_id.lower()]

if obj.type.name == "Sprite":
    tex = objs[obj.read_typetree()["m_RD"]["texture"]["m_PathID"]]; mode = "RGBA"
elif obj.type.name == "Texture2D":
    tex = obj; mode = "RGBA"
else:                                    # CardAssetDataSO
    so  = obj.read_typetree()
    mat = objs[so["cardMaterial"]["m_PathID"]].read_typetree()
    tex = objs[dict(mat["m_SavedProperties"]["m_TexEnvs"])["_MainTex"]["m_Texture"]["m_PathID"]]
    mode = "RGB"                         # alpha is the item cut-out mask, not transparency

img = tex.read().image.convert(mode)
img.thumbnail((512, 512)); img.save(out_dir / f"{card_id.lower()}.webp", "WEBP", quality=82)
```

### 3.3 Measured cost

These figures are from the full run on this machine: single-threaded Python 3.12, game on an HDD library drive. [V]

| | |
|---|---|
| Cards attempted (resolved keys) | 3,310 |
| Images written | **3,059** |
| Failures | 251: 250 encounter assets with no local portrait (needs the background fallback) and 1 item whose `cardMaterial` PathID is not in its own bundle (it is probably an external reference that was not followed) |
| Wall time | **171 s**. Per bundle 0.1–21.6 s, with the hero card bundles at 7–12 s each |
| Output size | **107 MB** (≤512 px WebP q82). Item art is 30–240 KB per card and skill icons are 7–9 KB |
| Full-resolution alternative | 1024² item art as WebP is ≈100–240 KB per card, so roughly 250–300 MB in total [I] |
| Python venv | UnityPy 1.25.4, Pillow 13, texture2ddecoder 1.0.6, created with `uv venv` outside the repo [V]. Its size was not measured. |

Running one process per bundle would cut wall time to well under a minute on a multi-core machine [I].

### 3.4 Redoing it after a patch

- **What to watch** [I]:
  - `TheBazaar_Data/StreamingAssets/aa/catalog.hash` (32 hex chars). It changes with every content build. This build's value is `d87b6caf…` from `catalog.bin`.
  - `boot.config` `build-guid`.
  - The mtime of `GameData.db`, because new cards can arrive through the DB before new art does.
- **When to run** [I]: on app start, compare the three values with `card-art/manifest.json`. If they differ, show "Card art is out of date: re-extract" instead of extracting silently.
- **Incremental runs** [I]: skip bundles whose file size and mtime are unchanged and whose cards are already present. Delete files for card ids no longer in the DB.
- Patches that change the Unity version need the `FALLBACK_UNITY_VERSION` updated. Read it from `globalgamemanagers` (offset 0x30 holds `"6000.3.11f1"` in this build) instead of hard-coding it [V for the offset].

## 4. Where it plugs into this app [I]

| Piece | File | Change |
|---|---|---|
| Card model | `crates/bazaar-tracker/src/catalog/mod.rs` | Add `art_key: Option<String>` to `CardInfo`, set in `parse_card` from `json.get("ArtKey").and_then(text_of)`. Add a matching `"ArtKey"` to one entry of `crates/bazaar-tracker/tests/fixtures/catalog/cards.json`, which has no `ArtKey` today. |
| Art lookup | new `crates/bazaar-tracker/src/catalog/art.rs` | `CardArt { dir: PathBuf, present: HashSet<String> }`, built once by listing `<data dir>/card-art/*.webp`, with `fn path(&self, card_id: &str) -> Option<PathBuf>` (lower-cased id). Ids in the DB are lower-case GUIDs, while the fixture uses an upper-case one, so normalise. No bundle code in the app. |
| Data folder | `docs/documents/data-folder.md`, `src/shell/data_dir.rs` | Document a `card-art/` subfolder next to `history.db` and `logs/`. It is user-generated, gitignored with the rest of `.bazaar-tracker/`, and safe to delete. |
| View model | `src/tracking/run/board.rs` | `Tile` and `SkillChip` gain `art: Option<PathBuf>` (or `card_id`). Set it in `tile()` and the skill builder from `item.template_id` / `skill.template_id`, which already feed `names.card(…)`. |
| Board UI | `src/ui/run/board.rs` | In `render_tile`, add `img(path).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)` as the first child, under the existing badges, sell price and cooldown overlays. Keep the tier `border_color` and add a dark gradient under the text for legibility. In `render_skills`, add a 16–20 px icon at the left of each chip. When `art` is `None`, keep today's coloured box. |
| Tooling | `justfile` | A `just card-art` recipe that runs the extractor with `python -I` in a venv under the data folder or a cache dir, never under `src/`. |

GPUI's `img()` accepts a file path. Whether `gpui-pre 0.3.9` was built with WebP decoding enabled has not been checked. If it was not, write PNG instead, at about 2–3 times the size [I].

## 5. Unknowns

- Whether any item art is missing from its RGB (card-art.md §2.5 and §5). Two of about 30 inspected items had smeared colour, so a visual review, or a comparison against bazaardb.gg images, is needed to size the problem.
- The encounter background fallback is not yet run. Whether the background or the portrait is the better image for an event card was not judged.
- None of the Rust crates were tested against this build.
- Not tested: AssetRipper and AssetStudio on Unity 6000.3, and WebP support in GPUI.
