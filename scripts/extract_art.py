# /// script
# requires-python = ">=3.11"
# dependencies = ["UnityPy==1.25.4", "Pillow>=11"]
# ///
"""Extract The Bazaar's card art into the tracker's data folder.

    uv run --script scripts/extract_art.py --game "H:/SteamLibrary/steamapps/common/The Bazaar"

Writes `<data dir>/art/<category>/<key>.webp` and `<data dir>/art/manifest.json`, where `<key>` is
the card's `ArtKey` lower-cased with any `.png` removed (see `art_file_key`). How the game stores
the art, and why the pipeline looks like this: docs/rea/the-bazaar/card-art.md and
card-art-extraction.md.

The art is Tempo Storm's. The output is for showing locally in this app only: it lives in the
gitignored data folder and must never be committed or redistributed.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import datetime
import json
import os
import re
import shutil
import sqlite3
import struct
import sys
import tempfile
import time
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Card `Type` -> output folder. Types not listed here have no art (`PlayerEffect`).
CATEGORIES = {
    "Item": "items",
    "Skill": "skills",
    "SocketEffect": "skills",
    "EncounterStep": "rewards",
    "CombatEncounter": "monsters",
    "EventEncounter": "events",
    "PedestalEncounter": "pedestals",
}


def art_file_key(art_key: str) -> str:
    """The file stem for an `ArtKey`. The app must apply the same rule to find a card's file."""
    key = art_key.strip()
    if key.lower().endswith(".png"):
        key = key[:-4]
    return re.sub(r'[\\/:*?"<>|]', "_", key).lower()


# ---------------------------------------------------------------------------------------------
# Inputs: game folders, Unity version, GameData.db
# ---------------------------------------------------------------------------------------------

def default_data_dir() -> Path:
    """Same rule as the app (src/shell/data_dir.rs) for a repo checkout."""
    env = os.environ.get("BAZAAR_TRACKER_DATA_DIR", "").strip()
    return Path(env) if env else REPO / ".bazaar-tracker"


def default_game_dir() -> Path | None:
    env = os.environ.get("BAZAAR_GAME_DIR", "").strip()
    candidates = [Path(env)] if env else []
    candidates.append(Path(r"C:\Program Files (x86)\Steam\steamapps\common\The Bazaar"))
    return next((path for path in candidates if (path / "TheBazaar_Data").is_dir()), None)


def unity_version(game: Path) -> str:
    """The engine version, which the bundles strip from their headers. It sits in the
    `globalgamemanagers` header (offset 0x30 in 6000.3.11f1)."""
    head = (game / "TheBazaar_Data" / "globalgamemanagers").read_bytes()[:0x200]
    found = re.search(rb"\d{4,}\.\d+\.\d+[abfpx]\d+", head)
    if not found:
        sys.exit("could not read the Unity version from globalgamemanagers")
    return found.group().decode()


def build_guid(game: Path) -> str | None:
    try:
        text = (game / "TheBazaar_Data" / "boot.config").read_text(errors="replace")
    except OSError:
        return None
    found = re.search(r"^build-guid=(\w+)", text, re.MULTILINE)
    return found.group(1) if found else None


def find_game_data(explicit: Path | None, game: Path, tmp: Path) -> tuple[Path, Path]:
    """(path to read, path to report). The game may be writing the live file, so read a copy.
    Looks where the app's catalog looks, then falls back to the seed copy the game ships."""
    if explicit:
        sources = [explicit]
    else:
        root = Path(os.environ["USERPROFILE"]) / "AppData" / "LocalLow" / "Tempo Storm" / "The Bazaar"
        prod = root / "prod"
        names = [prod / "cache", root / "cache", prod, root]
        sources = sorted((d / "GameData.db" for d in names if (d / "GameData.db").is_file()),
                         key=lambda p: p.stat().st_mtime, reverse=True)
    if sources:
        copy = tmp / "GameData.db"
        shutil.copyfile(sources[0], copy)
        return copy, sources[0]
    seed = game / "TheBazaar_Data" / "StreamingAssets" / "GameData.db.zip"
    if seed.is_file():
        with zipfile.ZipFile(seed) as archive:
            member = next(n for n in archive.namelist() if n.endswith(".db"))
            archive.extract(member, tmp)
        return tmp / member, seed
    sys.exit("no GameData.db found: start the game once, or pass --game-data")


def read_cards(db_path: Path) -> list[dict]:
    db = sqlite3.connect(f"file:{db_path.as_posix()}?mode=ro", uri=True)
    try:
        cards = []
        for card_id, data in db.execute("SELECT Id, Data FROM cards"):
            card = json.loads(data)
            cards.append({"id": str(card_id).lower(), "type": card.get("Type") or "", "art_key": card.get("ArtKey") or ""})
        return cards
    finally:
        db.close()


# ---------------------------------------------------------------------------------------------
# The Addressables binary catalog (catalog.bin): format in card-art.md §2.2
# ---------------------------------------------------------------------------------------------

class AddressablesCatalog:
    MAGIC = 0x0DE38942

    def __init__(self, path: Path):
        self.buf = path.read_bytes()
        magic, _version, keys, *_rest = struct.unpack_from("<iiIIIIII", self.buf, 0)
        if magic & 0xFFFFFFFF != self.MAGIC:
            sys.exit(f"{path} is not an Addressables binary catalog (magic {magic & 0xFFFFFFFF:#x})")
        self.index: dict[str, dict] = {}
        for k in range(self._size(keys) // 8):
            name_off, set_off = struct.unpack_from("<II", self.buf, keys + 8 * k)
            try:
                key = self._key(name_off)
            except (struct.error, UnicodeDecodeError):
                continue
            if not isinstance(key, str):
                continue
            locations = [self._u32(set_off + 4 * j) for j in range(self._size(set_off) // 4)]
            if not locations:
                continue
            entry = self._location(locations[0])
            if entry is None:
                continue
            self.index.setdefault(key, entry)
            self.index.setdefault(key.replace("/", "_"), entry)

    def _u32(self, offset: int) -> int:
        return struct.unpack_from("<I", self.buf, offset)[0]

    def _size(self, offset: int) -> int:
        """Every block is preceded by its byte length."""
        return self._u32(offset - 4)

    def _string(self, sid: int, sep: str = "") -> str | None:
        if sid == 0xFFFFFFFF:
            return None
        offset = sid & 0x3FFFFFFF
        if sid & 0x40000000:  # dynamic: a linked list of parts, stored last part first
            parts = []
            while True:
                part, nxt = struct.unpack_from("<II", self.buf, offset)
                parts.append(self._string(part) or "")
                if nxt == 0xFFFFFFFF:
                    break
                offset = nxt
            return sep.join(reversed(parts))
        raw = self.buf[offset:offset + self._size(offset)]
        return raw.decode("utf-16-le") if sid & 0x80000000 else raw.decode("ascii", "replace")

    def _key(self, offset: int):
        type_id, object_id = struct.unpack_from("<II", self.buf, offset)
        _assembly, cls = struct.unpack_from("<II", self.buf, type_id)
        cls_name = self._string(cls, ".") or ""
        if cls_name.endswith("String"):
            return self._string(self._u32(object_id), "/")
        return None

    def _location(self, offset: int) -> dict | None:
        _pk, internal, _prov, deps, _hash, _extra, type_off = struct.unpack_from("<IIIIiII", self.buf, offset)
        dep_list = [self._u32(deps + 4 * k) for k in range(self._size(deps) // 4)] if deps != 0xFFFFFFFF else []
        if not dep_list:
            return None
        bundle_internal = struct.unpack_from("<IIIIiII", self.buf, dep_list[0])[1]
        bundle = (self._string(bundle_internal, "/") or "").replace("\\", "/").rsplit("/", 1)[-1]
        if not bundle.endswith(".bundle"):
            return None
        asset_type = self._string(struct.unpack_from("<II", self.buf, type_off)[1], ".") or ""
        return {"id": self._string(internal, "/"), "type": asset_type, "bundle": bundle}

    def resolve(self, art_key: str) -> dict | None:
        """Try the key as stored, without its extension, and with `.png` (card-art.md §2.3)."""
        for candidate in (art_key, art_key.rsplit(".", 1)[0], art_key + ".png"):
            hit = self.index.get(candidate) or self.index.get(candidate.replace("_", "/"))
            if hit:
                return hit
        return None


# ---------------------------------------------------------------------------------------------
# Bundle work (runs in worker processes, one bundle per task)
# ---------------------------------------------------------------------------------------------

class Skip(Exception):
    pass


def _init_worker(unity: str) -> None:
    import warnings
    import UnityPy

    warnings.filterwarnings("ignore")
    UnityPy.config.FALLBACK_UNITY_VERSION = unity


def _follow(pptr: dict | None, objs: dict):
    """The object a PPtr points at, or None if it is empty. References into other bundles are
    not followed."""
    if not pptr or not pptr.get("m_PathID"):
        return None
    if pptr.get("m_FileID", 0) != 0:
        raise Skip("texture is in another bundle")
    obj = objs.get(pptr["m_PathID"])
    if obj is None:
        raise Skip("reference not in bundle")
    return obj


def _texture_of(obj, objs):
    """(Texture2D object, Pillow mode) for an addressable asset: card-art-extraction.md §3.1."""
    kind = obj.type.name
    if kind == "Texture2D":
        return obj, "RGBA"
    if kind == "Sprite":
        return _follow(obj.read_typetree()["m_RD"]["texture"], objs), "RGBA"
    tree = obj.read_typetree()
    if "cardMaterial" in tree:  # items: CardAssetDataSO -> Material -> _MainTex
        material = _follow(tree["cardMaterial"], objs)
        if material is None:
            raise Skip("item has no card material")
        envs = dict(material.read_typetree()["m_SavedProperties"]["m_TexEnvs"])
        texture = _follow(envs.get("_MainTex", {}).get("m_Texture"), objs)
        # The alpha of item art is the cut-out mask, not transparency (card-art.md §2.5).
        return texture, "RGB"
    # Encounters: portrait, else background.
    errors = []
    for field in ("portraitTextureReference", "backgroundTextureReference"):
        try:
            target = _follow(tree.get(field), objs)
        except Skip as error:
            errors.append(str(error))
            continue
        if target is None:
            continue
        if target.type.name == "Sprite":
            target = _follow(target.read_typetree()["m_RD"]["texture"], objs)
        return target, "RGBA"
    raise Skip(errors[0] if errors else "no portrait or background")


def extract_bundle(bundle_path: str, jobs: list[dict], max_size: int, quality: int) -> list[dict]:
    """Write each job's image. Returns one result per job: {out, ok, error?, bytes?}."""
    import UnityPy
    from PIL import Image

    results = []
    try:
        env = UnityPy.load(bundle_path)
    except Exception as error:  # noqa: BLE001 - report and carry on with other bundles
        return [{"out": job["out"], "ok": False, "error": f"bundle failed to load: {error}"[:200]} for job in jobs]
    objs = {obj.path_id: obj for obj in env.objects}
    container = {path.lower(): obj for path, obj in env.container.items()}
    decoded: dict[tuple[int, str], "Image.Image"] = {}
    for job in jobs:
        try:
            obj = container.get(job["internal_id"].lower())
            if obj is None:
                raise Skip("asset not in bundle")
            texture, mode = _texture_of(obj, objs)
            if texture is None:
                raise Skip("no texture")
            cache_key = (texture.path_id, mode)
            if cache_key not in decoded:
                image = texture.read().image.convert(mode)
                image.thumbnail((max_size, max_size), Image.LANCZOS)
                decoded[cache_key] = image
            out = Path(job["out"])
            out.parent.mkdir(parents=True, exist_ok=True)
            partial = out.with_suffix(".partial.webp")
            decoded[cache_key].save(partial, "WEBP", quality=quality, method=4)
            os.replace(partial, out)
            results.append({"out": job["out"], "ok": True, "bytes": out.stat().st_size})
        except Exception as error:  # noqa: BLE001 - one bad asset must not stop the bundle
            results.append({"out": job["out"], "ok": False, "error": str(error)[:200] or type(error).__name__})
    return results


# ---------------------------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="Extract The Bazaar's card art for the tracker (local use only).")
    parser.add_argument("--game", type=Path, default=default_game_dir(), help="The Bazaar install folder (or set BAZAAR_GAME_DIR)")
    parser.add_argument("--out", type=Path, default=None, help="output folder (default: <data dir>/art)")
    parser.add_argument("--game-data", type=Path, default=None, help="GameData.db to read (default: the game's LocalLow cache)")
    parser.add_argument("--max-size", type=int, default=512, help="longest side in pixels (default 512)")
    parser.add_argument("--quality", type=int, default=82, help="WebP quality (default 82)")
    parser.add_argument("--jobs", type=int, default=max(1, min(4, (os.cpu_count() or 2) - 1)), help="worker processes")
    parser.add_argument("--force", action="store_true", help="rewrite images that already exist")
    args = parser.parse_args()

    if args.game is None or not (args.game / "TheBazaar_Data").is_dir():
        sys.exit("pass --game <The Bazaar install folder> (the one holding TheBazaar_Data)")
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    started = time.time()
    game: Path = args.game
    aa = game / "TheBazaar_Data" / "StreamingAssets" / "aa"
    bundles_dir = aa / "StandaloneWindows64"
    out_dir: Path = args.out or default_data_dir() / "art"
    out_dir.mkdir(parents=True, exist_ok=True)

    unity = unity_version(game)
    catalog = AddressablesCatalog(aa / "catalog.bin")
    with tempfile.TemporaryDirectory(prefix="bazaar_art_") as tmp:
        db_copy, db_source = find_game_data(args.game_data, game, Path(tmp))
        cards = read_cards(db_copy)
    print(f"Unity {unity}, {len(catalog.index)} catalog keys, {len(cards)} cards from {db_source}")

    manifest_cards: dict[str, str] = {}
    failures: dict[str, str] = {}
    jobs_by_out: dict[str, dict] = {}
    for card in cards:
        category = CATEGORIES.get(card["type"])
        if not category or not card["art_key"]:
            continue
        hit = catalog.resolve(card["art_key"])
        if hit is None:
            failures[card["id"]] = f"ArtKey {card['art_key']!r} not in the Addressables catalog"
            continue
        rel = f"{category}/{art_file_key(card['art_key'])}.webp"
        manifest_cards[card["id"]] = rel
        jobs_by_out.setdefault(rel, {"out": str(out_dir / rel), "internal_id": hit["id"], "bundle": hit["bundle"]})

    pending = {rel: job for rel, job in jobs_by_out.items() if args.force or not Path(job["out"]).is_file()}
    by_bundle: dict[str, list[dict]] = collections.defaultdict(list)
    for job in pending.values():
        by_bundle[job["bundle"]].append(job)
    print(f"{len(jobs_by_out)} images for {len(manifest_cards)} cards; {len(pending)} to write "
          f"from {len(by_bundle)} bundles ({len(jobs_by_out) - len(pending)} already present)")

    failed_outs: dict[str, str] = {}
    written = written_bytes = 0
    with concurrent.futures.ProcessPoolExecutor(max_workers=args.jobs, initializer=_init_worker, initargs=(unity,)) as pool:
        # Biggest bundles first, so the slow ones do not finish last on their own.
        order = sorted(by_bundle, key=lambda b: (bundles_dir / b).stat().st_size if (bundles_dir / b).exists() else 0, reverse=True)
        futures = {pool.submit(extract_bundle, str(bundles_dir / bundle), by_bundle[bundle], args.max_size, args.quality): bundle for bundle in order}
        for done, future in enumerate(concurrent.futures.as_completed(futures), 1):
            bundle = futures[future]
            try:
                results = future.result()
            except Exception as error:  # noqa: BLE001 - e.g. a worker ran out of memory
                results = [{"out": job["out"], "ok": False, "error": f"worker failed: {error}"[:200]} for job in by_bundle[bundle]]
            ok = sum(r["ok"] for r in results)
            written += ok
            written_bytes += sum(r.get("bytes", 0) for r in results)
            for result in results:
                if not result["ok"]:
                    failed_outs[result["out"]] = result["error"]
            print(f"[{done}/{len(futures)}] {bundle}: {ok}/{len(results)}", flush=True)

    # Cards whose image failed are left out of the manifest, with the reason recorded.
    for card_id, rel in list(manifest_cards.items()):
        reason = failed_outs.get(str(out_dir / rel))
        if reason:
            failures[card_id] = reason
            del manifest_cards[card_id]

    catalog_hash_file = aa / "catalog.hash"
    manifest = {
        "version": 1,
        "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "unity_version": unity,
        "build_guid": build_guid(game),
        "catalog_hash": catalog_hash_file.read_text().strip() if catalog_hash_file.is_file() else None,
        "game_data": {"path": str(db_source), "modified": datetime.datetime.fromtimestamp(db_source.stat().st_mtime, datetime.timezone.utc).isoformat(timespec="seconds")},
        "format": {"type": "webp", "quality": args.quality, "max_size": args.max_size},
        "key_rule": "ArtKey, lower-cased, trailing .png removed, \\/:*?\"<>| replaced with _",
        "categories": CATEGORIES,
        "cards": dict(sorted(manifest_cards.items())),
        "failures": dict(sorted(failures.items())),
    }
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")

    reasons = collections.Counter(failures.values())
    print(f"\nwrote {written} images ({written_bytes / 1e6:.1f} MB) in {time.time() - started:.0f}s; "
          f"{len(manifest_cards)} cards have art, {len(failures)} do not")
    for reason, count in reasons.most_common(8):
        print(f"  {count:5} {reason}")
    print(f"manifest: {out_dir / 'manifest.json'}")


if __name__ == "__main__":
    main()
