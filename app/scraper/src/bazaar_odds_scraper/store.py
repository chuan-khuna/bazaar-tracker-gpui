"""The odds folder: one `<template_id>.json` per item.

Each entry in `odds` has exactly the fields of the tracker's `SavedItemOdds`
(`crates/bazaar-tracker/src/history/item_odds.rs`), with `body` kept as the text Bazaar DB sent,
so the tracker can read it with `bazaardb::item_odds::parse_json` and save it as it is:

    {
      "format": 1,
      "template_id": "…", "item_name": "Pasta Maker", "url_id": "qnwx…",
      "odds": [
        { "template_id": "…", "hero": "Jules", "tier": "None", "url_id": "qnwx…",
          "item_name": "Pasta Maker", "body": "{\\"itemName\\":…}", "saved_ms": 1793534002000 }
      ]
    }

`index.json` lists every file, so a reader need not list the folder.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

FORMAT = 1


class Store:
    def __init__(self, folder: Path) -> None:
        self.folder = folder
        folder.mkdir(parents=True, exist_ok=True)

    def path(self, template_id: str) -> Path:
        return self.folder / f"{template_id.lower()}.json"

    def read(self, template_id: str) -> dict | None:
        path = self.path(template_id)
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None
        except ValueError:
            print(f"store: {path} is not valid JSON; it will be replaced")
            return None

    def has(self, template_id: str, hero: str, tier: str) -> bool:
        """Whether odds are saved for the item as asked for. An answer saved under another hero
        (Bazaar DB's choice) does not count."""
        document = self.read(template_id) or {}
        return any(entry.get("hero") == hero and entry.get("tier") == tier for entry in document.get("odds", []))

    def url_id(self, template_id: str) -> str | None:
        return (self.read(template_id) or {}).get("url_id") or None

    def save(self, entry: dict) -> None:
        """Keep `entry`, replacing any for the same hero and tier."""
        template_id = entry["template_id"]
        document = self.read(template_id) or {}
        odds = [e for e in document.get("odds", []) if (e.get("hero"), e.get("tier")) != (entry["hero"], entry["tier"])]
        odds.append(entry)
        odds.sort(key=lambda e: (e["hero"], e["tier"]))
        document = {
            "format": FORMAT,
            "template_id": template_id,
            "item_name": entry["item_name"],
            "url_id": entry["url_id"] or document.get("url_id", ""),
            "odds": odds,
        }
        _write(self.path(template_id), document)

    def write_index(self) -> int:
        items = []
        for path in sorted(self.folder.glob("*.json")):
            if path.name == "index.json":
                continue
            try:
                document = json.loads(path.read_text(encoding="utf-8"))
            except ValueError:
                continue
            items.append({
                "file": path.name,
                "template_id": document.get("template_id", ""),
                "item_name": document.get("item_name", ""),
                "url_id": document.get("url_id", ""),
                "odds": [{"hero": e.get("hero"), "tier": e.get("tier")} for e in document.get("odds", [])],
            })
        _write(self.folder / "index.json", {"format": FORMAT, "items": items})
        return len(items)


def _write(path: Path, document: dict) -> None:
    """Write through a temporary file, so a stopped run never leaves half a file."""
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(temporary, path)
