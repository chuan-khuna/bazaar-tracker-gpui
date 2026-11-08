"""The odds folder: one `<template_id>.json` per item.

Each entry in `odds` has exactly the fields of the tracker's `SavedItemOdds`
(`crates/bazaar-tracker/src/history/item_odds.rs`), with `body` kept as the text Bazaar DB sent,
so the tracker can read it with `bazaardb::item_odds::parse_json` and save it as it is:

    {
      "format": 1,
      "template_id": "…", "item_name": "Pasta Maker", "url_id": "qnwx…",
      "owned_tiers": ["None", "Bronze", "Silver", "Gold"],
      "odds": [
        { "template_id": "…", "hero": "Jules", "tier": "None", "url_id": "qnwx…",
          "item_name": "Pasta Maker", "body": "{\\"itemName\\":…}", "saved_ms": 1793534002000 }
      ]
    }

`owned_tiers` are the buttons of the page's "Currently owned tier" group for the item (`None` is
"Not owned"), kept so a later run knows which tiers are still missing; the tracker ignores it.

`index.json` lists every file, so a reader need not list the folder.

`skipped.json` keeps every ask that got no odds (the page's search had no item of that name, or
Bazaar DB sent none), so a later run leaves it out unless asked to try again:

    {
      "format": 1,
      "skipped": [
        { "item_name": "3D Printer", "template_ids": ["…"], "hero": "Dooley", "tier": "None",
          "reason": "3D Printer is not in the page's search", "suggestions": ["…"], "skipped_ms": 1793534002000 }
      ]
    }

It is not an odds file: the index leaves it out, and the tracker never reads it.
"""

from __future__ import annotations

import json
import logging
import os
import time
from pathlib import Path

FORMAT = 1
NOT_OWNED = "None"

log = logging.getLogger("scrape-odds")


SKIPPED_FILE = "skipped.json"
NOT_ODDS = {"index.json", SKIPPED_FILE}


class Store:
    def __init__(self, folder: Path) -> None:
        self.folder = folder
        folder.mkdir(parents=True, exist_ok=True)
        self._skipped = self._read_skipped()

    def _read_skipped(self) -> list[dict]:
        path = self.folder / SKIPPED_FILE
        try:
            return list(json.loads(path.read_text(encoding="utf-8")).get("skipped", []))
        except FileNotFoundError:
            return []
        except (ValueError, AttributeError):
            log.warning(f"store: [path]{path}[/] is not valid JSON; it will be replaced")
            return []

    def skipped(self) -> list[dict]:
        return list(self._skipped)

    def was_skipped(self, template_id: str, hero: str, tier: str) -> bool:
        return any(template_id in entry.get("template_ids", []) and (entry.get("hero"), entry.get("tier")) == (hero, tier)
                   for entry in self._skipped)

    def skip(self, item_name: str, template_ids: list[str], hero: str, tier: str, reason: str, suggestions: list[str]) -> None:
        """Keep an ask that got no odds, replacing any earlier one for the same items, hero and tier."""
        self._forget(template_ids, hero, tier)
        self._skipped.append({
            "item_name": item_name, "template_ids": template_ids, "hero": hero, "tier": tier,
            "reason": reason, "suggestions": suggestions, "skipped_ms": int(time.time() * 1000),
        })
        self._write_skipped()

    def _forget(self, template_ids: list[str], hero: str, tier: str) -> bool:
        before = len(self._skipped)
        self._skipped = [entry for entry in self._skipped
                         if not (set(entry.get("template_ids", [])) & set(template_ids) and (entry.get("hero"), entry.get("tier")) == (hero, tier))]
        return len(self._skipped) != before

    def forget(self, template_ids: list[str], hero: str, tier: str) -> None:
        """Drop the skipped entry for these items, hero and tier, if there is one."""
        if self._forget(template_ids, hero, tier):
            self._write_skipped()

    def _write_skipped(self) -> None:
        self._skipped.sort(key=lambda entry: (entry.get("hero", ""), entry.get("tier", ""), entry.get("item_name", "")))
        _write(self.folder / SKIPPED_FILE, {"format": FORMAT, "skipped": self._skipped})

    def path(self, template_id: str) -> Path:
        return self.folder / f"{template_id.lower()}.json"

    def read(self, template_id: str) -> dict | None:
        path = self.path(template_id)
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None
        except ValueError:
            log.warning(f"store: [path]{path}[/] is not valid JSON; it will be replaced")
            return None

    def has(self, template_id: str, hero: str, tier: str) -> bool:
        """Whether odds are saved for the item as asked for. An answer saved under another hero
        (Bazaar DB's choice) does not count."""
        document = self.read(template_id) or {}
        return any(entry.get("hero") == hero and entry.get("tier") == tier for entry in document.get("odds", []))

    def owned_tiers(self, template_id: str) -> list[str] | None:
        """The owned tiers the item has, as the page offered them; for a file saved before they were
        kept, `None` plus every tier but the highest of Bazaar DB's `itemTiers` (the page's rule).
        `None` when nothing is saved for the item."""
        document = self.read(template_id)
        if not document:
            return None
        if document.get("owned_tiers"):
            return list(document["owned_tiers"])
        for entry in document.get("odds", []):
            try:
                tiers = json.loads(entry.get("body") or "{}").get("itemTiers") or []
            except ValueError:
                continue
            return [NOT_OWNED, *tiers[:-1]]
        return None

    def saved_tiers(self, template_id: str, hero: str) -> set[str]:
        document = self.read(template_id) or {}
        return {entry.get("tier") for entry in document.get("odds", []) if entry.get("hero") == hero}

    def has_every_tier(self, template_id: str, hero: str) -> bool:
        """Whether odds are saved for the item and hero at every owned tier it has."""
        tiers = self.owned_tiers(template_id)
        return tiers is not None and set(tiers) <= self.saved_tiers(template_id, hero)

    def set_owned_tiers(self, template_id: str, tiers: list[str]) -> None:
        document = self.read(template_id)
        if document is not None and document.get("owned_tiers") != tiers:
            document["owned_tiers"] = tiers
            _write(self.path(template_id), document)

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
            **({"owned_tiers": document["owned_tiers"]} if document.get("owned_tiers") else {}),
            "odds": odds,
        }
        _write(self.path(template_id), document)
        if self._forget([template_id], entry["hero"], entry["tier"]):
            self._write_skipped()

    def write_index(self) -> int:
        items = []
        for path in sorted(self.folder.glob("*.json")):
            if path.name in NOT_ODDS:
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
