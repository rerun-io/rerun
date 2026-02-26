"""Efficiently time align multirate columns."""

# region: setup
from __future__ import annotations

from pathlib import Path

import numpy as np
import rerun as rr
from datafusion import col

sample_5_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
CATALOG_URL = server.url()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="sample_dataset")
# endregion: setup

# region: extract_timepoints
view = dataset.filter_segments("ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s").filter_contents("/observation/joint_positions")
ranges = view.get_index_ranges().to_arrow_table()

min_time = ranges["real_time:start"].to_numpy().flatten()
max_time = ranges["real_time:end"].to_numpy().flatten()
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
