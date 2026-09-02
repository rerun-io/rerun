"""
Experimental table card layout with flag toggles.

Demonstrates the card layout and per-row flag annotations on a remote table.

**TODO(#12745): This feature is experimental.**

A registered table blueprint enables the card view and configures the boolean
`flagged` column as an editable flag field. Each value drives the flag icon on
the card, and clicking the icon updates the visible table state and upserts the
new boolean value back to the server. The table must also have a
`rerun:is_table_index` column so the upsert can target the row to update.

Usage:
    table_grid_with_flags

    # In a separate terminal, open the viewer with the URL printed by the script:
    rerun <url>
"""

from __future__ import annotations

import argparse
from pathlib import Path
from tempfile import TemporaryDirectory

import pyarrow as pa

import rerun as rr
import rerun.blueprint as rrb
from rerun import bindings
from rerun.recording_stream import RecordingStream
from rerun.server import Server


def save_flag_blueprint(path: Path) -> None:
    with RecordingStream._from_native(
        bindings.new_blueprint(
            application_id="embedded",
            make_default=False,
            make_thread_default=False,
            default_enabled=True,
        ),
    ) as blueprint_stream:
        blueprint_stream.save(str(path))
        blueprint_stream.set_time("blueprint", sequence=0)
        blueprint_stream.log(
            "/table/layouts/cards/fields/flagged",
            rrb.experimental.TableColumn(
                editable=True,
                cell_kind=rrb.components.TableCellKind.Flag,
            ),
        )
        blueprint_stream.log(
            "/table/layouts/cards",
            rrb.experimental.CardLayout(
                field_order=["flagged"],
                title="name",
            ),
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Create an experimental table card layout with flag toggles.")
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
        pa.field("flagged", pa.bool_()),
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

        with TemporaryDirectory() as blueprint_dir:
            blueprint_path = Path(blueprint_dir) / "flags.rbl"
            save_flag_blueprint(blueprint_path)
            table.register_blueprint(blueprint_path.absolute().as_uri())

        url = f"{srv.url()}/entry/{table.id}"
        print(f"Open the viewer with:\n  rerun {url}")

        input("Press Enter to stop the server…")


if __name__ == "__main__":
    main()
