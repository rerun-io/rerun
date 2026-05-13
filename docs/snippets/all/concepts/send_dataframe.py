"""Hand-craft a PyArrow `Table` with Rerun schema metadata and send it via `send_dataframe`."""

from __future__ import annotations

import pyarrow as pa

import rerun as rr

rr.init("rerun_example_send_dataframe")

# An index column…
index = pa.array([0, 1, 2], type=pa.int64())

# …and a component column. Each row is a list (one component batch per row).
positions = pa.array(
    [
        [[1.0, 0.0, 0.0]],
        [[0.0, 1.0, 0.0]],
        [[0.0, 0.0, 1.0]],
    ],
    type=pa.list_(pa.list_(pa.field("item", pa.float32(), nullable=False), 3)),
)

# Tag each column with the `rerun:*` metadata keys that `send_dataframe` recognizes.
schema = pa.schema([
    pa.field(
        "frame",
        index.type,
        metadata={b"rerun:index_name": b"frame", b"rerun:kind": b"index"},
    ),
    pa.field(
        "/points:Points3D:positions",
        positions.type,
        metadata={
            b"rerun:entity_path": b"/points",
            b"rerun:archetype": b"rerun.archetypes.Points3D",
            b"rerun:component": b"Points3D:positions",
            b"rerun:component_type": b"rerun.components.Position3D",
            b"rerun:kind": b"data",
        },
    ),
])

table = pa.Table.from_arrays([index, positions], schema=schema)
rr.send_dataframe(table)
