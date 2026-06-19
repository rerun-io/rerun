"""Build chunks from an Arrow `RecordBatch` with `Chunk.from_record_batch`."""

from __future__ import annotations

import pyarrow as pa

import rerun.experimental as rrx

# region: body
# Create an index column.
frame = pa.array([0, 1, 2], type=pa.int64())

# Create two component columns.
positions_datatype = pa.list_(
    pa.list_(pa.field("item", pa.float32(), nullable=False), 3)
)
left = pa.array(
    [[[1.0, 0.0, 0.0]], [[2.0, 0.0, 0.0]], [[3.0, 0.0, 0.0]]],
    type=positions_datatype,
)
right = pa.array(
    [[[0.0, 1.0, 0.0]], [[0.0, 2.0, 0.0]], [[0.0, 3.0, 0.0]]],
    type=positions_datatype,
)

# The `/entity:Archetype:component` column-name convention tells
# `from_record_batch` which entity and component each column maps to.
batch = pa.RecordBatch.from_arrays(
    [frame, left, right],
    names=["frame", "/left:Points3D:positions", "/right:Points3D:positions"],
)

chunks = rrx.Chunk.from_record_batch(batch, index="frame")

for chunk in chunks:
    print(chunk)
# endregion: body
