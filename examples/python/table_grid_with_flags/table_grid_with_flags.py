"""
Experimental table grid with flag toggles.

Demonstrates the card/grid table layout and per-row flag annotations on a
remote table.

**TODO(#12745): This feature is experimental.** Enable it in the viewer under
Settings > Experimental > Table cards and blueprints.

The flag column is configured via Arrow field metadata
(`rerun:is_flag_column = "true"`). The Viewer treats that boolean column
as the per-row flag state: each value drives the flag icon on the grid card,
and clicking the icon updates the visible table state and upserts the new
boolean value back to the server. The table must also have a
`rerun:is_table_index` column so the upsert can target the row to update.

Usage:
    table_grid_with_flags

    # In a separate terminal, open the viewer with the URL printed by the script:
    rerun <url>
"""

from __future__ import annotations

import argparse

import pyarrow as pa

import rerun as rr
from rerun.server import Server


def main() -> None:
    parser = argparse.ArgumentParser(description="Create an experimental table grid with flag toggles.")
    parser.add_argument("--port", type=int, default=None, help="Port for the local Rerun server.")
    args = parser.parse_args()

    schema = pa.schema([
        pa.field(
            "id",
            pa.int64(),
            metadata={rr.SORBET_IS_TABLE_INDEX: "true"},
        ),
        pa.field("name", pa.utf8()),
        pa.field("category", pa.utf8()),
        pa.field("score", pa.float64()),
        pa.field(
            "flagged",
            pa.bool_(),
            metadata={"rerun:is_flag_column": "true"},
        ),
    ])

    data = {
        "id": [1, 2, 3, 4, 5],
        "name": ["Alice", "Bob", "Charlie", "Diana", "Eve"],
        "category": ["robotics", "vision", "robotics", "spatial", "vision"],
        "score": [95.0, 82.5, 91.0, 88.0, 76.5],
        "flagged": [False, False, False, False, False],
    }

    with Server(port=args.port) as srv:
        client = srv.client()
        table = client.create_table("flag_demo", schema)
        table.append(**data)

        url = f"{srv.url()}/entry/{table.id}"
        print(f"Open the viewer with:\n  rerun {url}")

        input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
