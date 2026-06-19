"""Build a `Chunk` with `Chunk.from_columns` and send it via `send_chunks`."""

from __future__ import annotations

import rerun as rr
import rerun.experimental as rrx

rr.init("rerun_example_build_chunk")

chunk = rrx.Chunk.from_columns(
    "/points",
    indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],
    columns=rr.Points3D.columns(
        positions=[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        radii=[0.1, 0.2, 0.3],
    ),
)

# Chunks can be inspected in many ways, including a text representation of
# its content
print(chunk)

rrx.send_chunks(chunk)
