# region: setup
from __future__ import annotations

from pathlib import Path

import datafusion as dfn
import numpy as np
import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F, lit

sample_5_path = Path(__file__).parent.parent.parent.parent / "data" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
CATALOG_URL = server.address()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="sample_dataset")
observations = dataset.filter_contents(["/observation/**"]).reader(index="real_time")
# endregion: setup

# region: group_by
first_last = observations.aggregate(
    col("rerun_segment_id"),
    [
        F.first_value(col("real_time")).alias("start"),
        F.last_value(col("real_time")).alias("end"),
    ],
)

# Sort for consistency here
first_last = first_last.sort("start")
pa.table(first_last)["start"][0]
# endregion: group_by

# region: join_query
joints = dataset.filter_contents(["/observation/joint_positions"])

# Find the earliest joint position in each episode (cast to unix epoch nanoseconds for easier math later)
joint_min_t = (
    joints.reader(index="real_time")
    .with_column("joint_epoch_ns", col("real_time").cast(pa.int64()))
    .select("rerun_segment_id", "joint_epoch_ns")
    .aggregate(
        col("rerun_segment_id"),
        F.min(col("joint_epoch_ns")).alias("joint_min_t"),
    )
)

cameras = dataset.filter_contents(["/camera/**"])

# Find the earliest camera frame in each episode (cast to unix epoch nanoseconds for easier math later)
camera_min_t = (
    cameras.reader(index="real_time")
    .with_column("camera_epoch_ns", col("real_time").cast(pa.int64()))
    .select(
        "rerun_segment_id",
        col("real_time").cast(pa.int64()).alias("camera_epoch_ns"),
    )
    .aggregate(
        col("rerun_segment_id"),
        F.min(col("camera_epoch_ns")).alias("camera_min_t"),
    )
)

# Join the two dataframes
min_t = camera_min_t.join(
    joint_min_t.with_column_renamed("rerun_segment_id", "segment_id"),
    left_on="rerun_segment_id",
    right_on="segment_id",
    how="left",
)
delta_t = min_t.select(
    col("rerun_segment_id"),
    (col("camera_min_t") - col("joint_min_t")).alias("start_delta_t"),
)
THRESHOLD_S = 1
NANO_S = 1_000_000_000
outliers = delta_t.filter(
    dfn.Expr.between(col("start_delta_t"), -THRESHOLD_S * NANO_S, THRESHOLD_S * NANO_S, negated=True),
)
outliers = outliers.with_column("start_delta_t_s", col("start_delta_t") / 1_000_000_000.0)

print(f"{outliers.count()=}\n", f"{joint_min_t.count()=}\n", f"{camera_min_t.count()=}", sep="")
# endregion: join_query

# region: sub_episodes
# Grab a dataframe
all_data = (
    dataset.filter_contents(["/action/**", "/observation/**"])
    .reader(index="real_time", fill_latest_at=True)
    .filter(
        col(
            "/observation/joint_positions:Scalars:scalars"
        ).is_not_null()  # filter out rows where there is no observation
    )
)

# Drop heavy columns for performance
light_slice = all_data.select(
    "rerun_segment_id",
    "real_time",
    "/observation/gripper_position:Scalars:scalars",
)

# Define criteria for sub-episode start/end
THRESHOLD = 0.1
light_slice = light_slice.with_column(
    "gripper_open",
    col("/observation/gripper_position:Scalars:scalars") > [THRESHOLD],
)

# Find start and end
light_slice = light_slice.with_column(
    "prev_gripper_open",
    F.lag(
        col("gripper_open"), default_value=False, partition_by=[col("rerun_segment_id")], order_by=[col("real_time")]
    ),
)
light_slice = light_slice.with_column(
    "gripper_change",
    col("gripper_open").cast(pa.int8()) - col("prev_gripper_open").cast(pa.int8()),
)

slice_times = light_slice.with_column(
    "start",
    F.case(col("gripper_change")).when(lit(1), col("real_time")).otherwise(lit(None)),
).with_column(
    "end",
    F.case(col("gripper_change")).when(lit(-1), col("real_time")).otherwise(lit(None)),
)

# Helper because pyarrow timestamps didn't have a nice min/max utility
max_ts = pa.scalar(np.iinfo(np.int64).max, type=pa.timestamp("ns"))
min_ts = pa.scalar(np.iinfo(np.int64).min + 1_000_000_000, type=pa.timestamp("ns"))

# This generates the column for the last observed start time
slice_dense_times = (
    slice_times.select("rerun_segment_id", "real_time", "start", "end")
    .with_column(
        "dense_start",
        F.last_value(col("start")).over(
            dfn.expr.Window(
                window_frame=dfn.expr.WindowFrame("rows", None, 0),
                order_by=col("real_time"),
                partition_by=col("rerun_segment_id"),
                null_treatment=dfn.common.NullTreatment.IGNORE_NULLS,
            )
        ),
    )
    .fill_null(value=max_ts, subset=["dense_start"])
)

# This generates the column for the next observed end time (by finding the last_value in reversed order)
slice_dense_times = slice_dense_times.with_column(
    "dense_end",
    F.last_value(col("end")).over(
        dfn.expr.Window(
            window_frame=dfn.expr.WindowFrame("rows", None, 0),
            order_by=col("real_time").sort(ascending=False),
            partition_by=col("rerun_segment_id"),
            null_treatment=dfn.common.NullTreatment.IGNORE_NULLS,
        )
    ),
).fill_null(value=min_ts, subset=["dense_end"])

slice_dense_times = slice_dense_times.select("rerun_segment_id", "real_time", "dense_start", "dense_end")

sub_episodes = slice_dense_times.filter(
    dfn.Expr.between(col("real_time"), col("dense_start"), col("dense_end")),
)

print(f"{sub_episodes.count()=}")
# endregion: sub_episodes
