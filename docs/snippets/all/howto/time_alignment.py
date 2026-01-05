# region: setup
from __future__ import annotations

from pathlib import Path

import numpy as np
import rerun as rr
from datafusion import col, functions as F

sample_5_path = Path(__file__).parent.parent.parent.parent / "data" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
CATALOG_URL = server.address()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="sample_dataset")
# endregion: setup

# region: extract_timepoints
cheaper_column = (
    dataset.filter_segments("ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s")
    .filter_contents("/observation/joint_positions")
    .reader(index="real_time")
)

min_max = cheaper_column.aggregate(
    "rerun_segment_id", [F.min(col("real_time")).alias("min"), F.max(col("real_time")).alias("max")]
)

min_time = min_max.to_arrow_table()["min"].to_numpy().flatten()
max_time = min_max.to_arrow_table()["max"].to_numpy().flatten()
desired_timestamps = np.arange(min_time[0], max_time[0], np.timedelta64(100, "ms"))  # 10Hz
# endregion: extract_timepoints

# region: time_align
# Select columns of interest
# specify desired timestamps
# forward fill to specified time for alignment
fixed_hz = (
    dataset.filter_segments("ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s")
    .filter_contents(["/observation/joint_positions", "/camera/ext1/**"])
    .reader(index="real_time", using_index_values=desired_timestamps, fill_latest_at=True)
)

# Filter out partially sparse rows (since one column may start before the other)
fixed_hz_filtered = fixed_hz.filter(
    col("/observation/joint_positions:Scalars:scalars").is_not_null(),
    col("/camera/ext1:VideoStream:sample").is_not_null(),
)
# endregion: time_align
