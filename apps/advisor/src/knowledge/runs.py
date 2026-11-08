"""Loading imported Scraped runs from history.db as ``RunData`` for the statistics."""

from __future__ import annotations

from collections.abc import Iterable

from sqlalchemy import Connection, text

from knowledge.stats import BoardCard, RunData


def load_runs(connection: Connection, run_ids: Iterable[str]) -> list[RunData]:
    """The runs with these ids, in the order given; ids not imported are left out."""
    ids = list(dict.fromkeys(run_ids))
    if not ids:
        return []
    runs: dict[str, RunData] = {}
    # SQLite allows many parameters, but chunk anyway so a big sample never hits the limit.
    for start in range(0, len(ids), 500):
        chunk = ids[start : start + 500]
        marks = ", ".join(f":id{i}" for i in range(len(chunk)))
        params = {f"id{i}": run_id for i, run_id in enumerate(chunk)}
        for run_id, wins, losses in connection.execute(
            text(f"SELECT run_id, wins, losses FROM imported_runs WHERE run_id IN ({marks})"), params
        ):
            runs[run_id] = RunData(run_id=run_id, wins=wins or 0, losses=losses or 0)
        for run_id, card_id, kind, slot, size in connection.execute(
            text(
                f"SELECT run_id, card_id, kind, slot, size FROM imported_run_cards "
                f"WHERE run_id IN ({marks}) ORDER BY run_id, rowid"
            ),
            params,
        ):
            if kind == "board":
                runs[run_id].board.append(BoardCard(card_id, slot, size))
            elif kind == "skill":
                runs[run_id].skills.append(card_id)
        # rowid keeps the file's order, so it is the order the cards were taken in.
        for run_id, card_id in connection.execute(
            text(f"SELECT run_id, card_id FROM imported_picks WHERE run_id IN ({marks}) AND taken = 1 ORDER BY run_id, rowid"),
            params,
        ):
            runs[run_id].taken.append(card_id)
    return [runs[run_id] for run_id in ids if run_id in runs]
