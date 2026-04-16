"""
Experimental grid view with flag toggling.

Demonstrates the grid view card layout and per-row flag annotations on a
remote table.

**This feature is experimental.** Enable it in the viewer under
Settings > Experimental > Grid view.

The flag column is configured via Arrow field metadata
(``rerun:is_flag_column = "true"``). The table must also have a
``rerun:is_table_index`` column so that flag changes can be persisted
back to the server via upsert.

Usage:
    pixi run uvpy rerun_py/tests/example_grid_view_flagging.py

    # In a separate terminal, open the viewer with the URL printed by the script:
    pixi run rerun <url>
"""

from __future__ import annotations

import pyarrow as pa
from rerun.server import Server


def main() -> None:
    schema = pa.schema([
        pa.field(
            "id",
            pa.int64(),
            metadata={"rerun:is_table_index": "true"},
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

    port = 1234
    with Server(port=port) as srv:
        client = srv.client()
        table = client.create_table("flag_demo", schema)
        table.append(**data)

        url = f"rerun+http://localhost:{port}/entry/{table.id}"
        print(f"Open the viewer with:\n  pixi run rerun {url}")

        input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
