# ruff: noqa: E402

from __future__ import annotations

import atexit
import shutil
import tempfile
import pathlib
import pyarrow as pa


TMP_DIR = pathlib.Path(tempfile.mkdtemp())
atexit.register(lambda: shutil.rmtree(TMP_DIR) if TMP_DIR.exists() else None)


# region: setup
import rerun as rr

from pathlib import Path

sample_5_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
client = server.client()
dataset = client.get_dataset(name="sample_dataset")

print(
    dataset.segment_table()
    .select(
        "rerun_segment_id",
        "rerun_layer_names",
    )
    .sort("rerun_segment_id")
)
# endregion: setup

# region: add_tracking_error
import numpy as np
from datafusion import col

# Query action (commanded) and observation (actual) joint positions
joints = dataset.filter_contents(["/action/joint_positions", "/observation/joint_positions"]).reader(index="real_time")

# Compute tracking error: L2 norm of (commanded - actual) joint positions
segment_ids = pa.table(joints.select("rerun_segment_id").distinct())["rerun_segment_id"].to_numpy()
rrd_paths = []

for seg_id in segment_ids:
    # Filter to this segment and collect as a PyArrow table for efficient extraction to NumPy
    segment_data = pa.table(
        joints.filter(col("rerun_segment_id") == seg_id).select(
            "real_time",
            "/action/joint_positions:Scalars:scalars",
            "/observation/joint_positions:Scalars:scalars",
        )
    )

    timestamps = segment_data["real_time"].to_numpy()

    actions = np.vstack(segment_data["/action/joint_positions:Scalars:scalars"].to_numpy())
    observations = np.vstack(segment_data["/observation/joint_positions:Scalars:scalars"].to_numpy())

    # Compute L2 tracking error per timestep
    tracking_error = np.linalg.norm(actions - observations, axis=1)

    # Create derived RRD with tracking error timeline
    rrd_path = TMP_DIR / f"{seg_id}_tracking_error.rrd"
    rrd_paths.append(rrd_path)

    with rr.RecordingStream(application_id="rerun_example_tracking_error", recording_id=seg_id) as rec:
        rec.save(rrd_path)
        rr.send_columns(
            "/derived/tracking_error",
            indexes=[rr.TimeColumn("real_time", timestamp=timestamps)],
            columns=rr.Scalars.columns(scalars=tracking_error),
        )


# Register derived RRDs as a new layer
dataset.register([p.as_uri() for p in rrd_paths], layer_name="tracking_error").wait()
# endregion: add_tracking_error

# region: check_layer_names
segment_table = (
    dataset.segment_table()
    .select(
        "rerun_segment_id",
        "rerun_layer_names",
    )
    .sort("rerun_segment_id")
)
print(segment_table)
# endregion: check_layer_names

# region: add_quality_property
# Query the tracking error we just added and compute a quality metric
from datafusion import functions as F

tracking = dataset.filter_contents(["/derived/tracking_error"]).reader(index="real_time")
quality_stats = pa.table(
    tracking.aggregate(
        col("rerun_segment_id"),
        [F.avg(col("/derived/tracking_error:Scalars:scalars")[0]).alias("mean_error")],
    )
    .with_column("tracking_good", col("mean_error") < 0.13)
    .select("rerun_segment_id", "tracking_good")
)

# Create RRDs with just the property
rrd_paths = []
for seg_id, tracking_good in zip(quality_stats["rerun_segment_id"], quality_stats["tracking_good"]):
    rrd_path = TMP_DIR / f"{seg_id}_quality.rrd"
    rrd_paths.append(rrd_path)

    with rr.RecordingStream(application_id="rerun_example_quality", recording_id=seg_id) as rec:
        rec.save(rrd_path)
        rec.send_property("quality", rr.AnyValues(tracking_good=tracking_good))

# Register as a separate layer
dataset.register([p.as_uri() for p in rrd_paths], layer_name="quality").wait()
# endregion: add_quality_property

# region: verify
# The segment table now shows both layers and the derived property
segment_table = (
    dataset.segment_table()
    .select(
        "rerun_segment_id",
        "rerun_layer_names",
        "property:quality:tracking_good",
    )
    .sort("rerun_segment_id")
)
print(segment_table)
# endregion: verify

# region: manifest
manifest = (
    dataset.manifest()
    .select(
        "rerun_segment_id",
        "rerun_layer_name",
        "property:quality:tracking_good",
    )
    .sort("rerun_segment_id", "rerun_layer_name")
)
print(manifest)
# endregion: manifest
