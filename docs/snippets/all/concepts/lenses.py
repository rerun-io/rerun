"""Use lenses to extract struct fields and reroute data to another entity."""

import pyarrow as pa

import rerun as rr
from rerun.experimental import (
    Chunk,
    DeriveLens,
    LazyChunkStream,
    MutateLens,
    Selector,
    send_chunks,
)

rr.init("rerun_example_lenses", spawn=True)

# region: log_data
# Build a chunk with a struct-typed component.
imu_data = pa.StructArray.from_arrays(
    [
        pa.array([1.0, 2.0, 3.0], type=pa.float64()),
        pa.array([4.0, 5.0, 6.0], type=pa.float64()),
        pa.array([0, 10_000_000, 20_000_000], type=pa.int64()),
    ],
    names=["x", "y", "elapsed"],
)
status_data = pa.array(["ok", "ok", "warn"], type=pa.utf8())
chunk = Chunk.from_columns(
    "/sensor/imu",
    indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],
    columns=rr.DynamicArchetype.columns(
        archetype="Imu", components={"accel": imu_data, "status": status_data}
    ),
)
# endregion: log_data

# Extract the "x" field as a Scalar on the same entity.
extract_x = DeriveLens("Imu:accel").to_component(
    rr.Scalars.descriptor_scalars(), ".x"
)

# region: derive_lens
# Extract the "y" field to a different entity and the "elapsed" field as a
# new timeline.
extract_y = (
    DeriveLens("Imu:accel", output_entity="/new_entity/accel_y")
    .to_component(rr.Scalars.descriptor_scalars(), ".y")
    .to_timeline("sensor_elapsed", "duration_ns", ".elapsed")
)
# endregion: derive_lens

# region: mutate_lens
# Simplify the accel struct to just its "x" field in-place.
simplify_accel = MutateLens("Imu:accel", ".x")
# endregion: mutate_lens

# region: pipe_example
# Use pipe to apply a custom transformation after extracting a field.
extract_scaled_x = DeriveLens(
    "Imu:accel", output_entity="/new_entity/accel_scaled_x"
).to_component(
    rr.Scalars.descriptor_scalars(),
    Selector(".x").pipe(lambda arr: pa.compute.multiply(arr, 9.81)),
)
# endregion: pipe_example

# Apply all lenses via the ChunkStream API and send the resulting chunks.
stream = LazyChunkStream.from_iter([chunk])
results = stream.lenses(
    [extract_x, extract_y, simplify_accel, extract_scaled_x],
    output_mode="forward_unmatched",
)
send_chunks(results)
