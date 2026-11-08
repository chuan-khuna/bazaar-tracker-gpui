"""Read the data a Bazaar DB page was built from, out of the page's React Server Components payload.

Bazaar DB is a Next.js site: each page carries the props its components were rendered with as a
"flight" payload, pushed in pieces by `self.__next_f.push([1, "…"])` scripts. The payload is a list
of rows, `<hex id>:<value>\\n`, where a value is JSON (or `T<hex byte length>,<text>` for a long
text, or a letter and JSON for a module import or a hint). A string in a value that starts with `$`
points elsewhere: `$<id>` is row `<id>`, `$<id>:a:0:b` a part of it (React sends an object once and
points to it after), `$undefined` is a missing value and `$$…` is a string that starts with `$`.
"""

from __future__ import annotations

import json
import re
from collections.abc import Callable, Iterator
from typing import Any

_PUSH = re.compile(r"self\.__next_f\.push\((\[.*?\])\)</script>", re.S)
_REF = re.compile(r"\$([0-9a-f]+)((?::[^:]+)*)")
_DECODER = json.JSONDecoder()


def flight_from_html(html: str) -> str:
    """The flight payload of a page's HTML, as the page's own script would put it together."""
    text = []
    for push in _PUSH.findall(html):
        piece = json.loads(push)
        if len(piece) > 1 and isinstance(piece[1], str):
            text.append(piece[1])
    return "".join(text)


class Flight:
    def __init__(self, text: str) -> None:
        self.rows: dict[str, Any] = {}
        self._resolved: dict[str, Any] = {}
        self._parse(text)

    def _parse(self, text: str) -> None:
        pos = 0
        while pos < len(text):
            if text[pos] == "\n":
                pos += 1
                continue
            colon = text.find(":", pos)
            if colon < 0:
                break
            row_id, pos = text[pos:colon], colon + 1
            if text.startswith("T", pos):  # T<hex length in UTF-8 bytes>,<text>
                comma = text.index(",", pos)
                size = int(text[pos + 1:comma], 16)
                start = comma + 1
                end = start + size  # never short: a char is at least one byte
                while len(text[start:end].encode("utf-8")) > size:
                    end -= 1
                self.rows[row_id] = text[start:end]
                pos = end
                continue
            while pos < len(text) and text[pos].isalpha() and text[pos].isupper():
                pos += 1  # I[…] (an import), HL[…] (a hint), E{…} (an error): kept as their JSON
            try:
                value, pos = _DECODER.raw_decode(text, pos)
            except json.JSONDecodeError:
                newline = text.find("\n", pos)
                pos = len(text) if newline < 0 else newline
                continue
            self.rows[row_id] = value

    def row(self, row_id: str) -> Any:
        if row_id not in self._resolved:
            self._resolved[row_id] = None  # a row that points to itself reads as null
            self._resolved[row_id] = self.resolve(self.rows.get(row_id))
        return self._resolved[row_id]

    def resolve(self, value: Any) -> Any:
        """`value` with every `$` reference in it followed."""
        if isinstance(value, dict):
            return {key: self.resolve(item) for key, item in value.items()}
        if isinstance(value, list):
            return [self.resolve(item) for item in value]
        if not isinstance(value, str) or not value.startswith("$"):
            return value
        if value == "$undefined":
            return None
        if value.startswith("$$"):
            return value[1:]
        if value.startswith("$D"):
            return value[2:]  # a date, as ISO text
        if value.startswith("$n"):
            return int(value[2:])
        match = _REF.fullmatch(value)
        if not match:
            return value  # a component, a promise or another kind of reference: left as it is
        found = self.row(match.group(1))
        for key in match.group(2).split(":")[1:]:
            if _is_element(found) and key in _ELEMENT_FIELDS:
                found = found[_ELEMENT_FIELDS[key]]
            elif isinstance(found, list) and key.isdigit() and int(key) < len(found):
                found = found[int(key)]
            elif isinstance(found, dict) and key in found:
                found = found[key]
            else:
                return None
        return found

    def find(self, wanted: Callable[[dict], bool]) -> Iterator[dict]:
        """Every object in the payload `wanted` says yes to, with its references followed."""
        for row_id, value in self.rows.items():
            for found in _objects(value, wanted):
                yield self.resolve(found)


# A React element is sent as `["$", type, key, props]`; a reference names its parts by field.
_ELEMENT_FIELDS = {"type": 1, "key": 2, "props": 3}


def _is_element(value: Any) -> bool:
    return isinstance(value, list) and len(value) == 4 and value[0] == "$"


def _objects(value: Any, wanted: Callable[[dict], bool]) -> Iterator[dict]:
    if isinstance(value, dict):
        if wanted(value):
            yield value
            return
        for item in value.values():
            yield from _objects(item, wanted)
    elif isinstance(value, list):
        for item in value:
            yield from _objects(item, wanted)
