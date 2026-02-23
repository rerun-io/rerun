"""Compute viewer URLs for segments using the segment_url UDF."""

# region: setup
from __future__ import annotations

from datetime import datetime, timedelta
from pathlib import Path

import pyarrow as pa
import rerun as rr
from datafusion import lit
from rerun.utilities.datafusion.functions.url_generation import segment_url

sample_5_path = Path(__file__).parents[5] / "tests" / "assets" / "rrd" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path}, port=51234)
client = server.client()
dataset = client.get_dataset(name="sample_dataset")

# Pick 3 deterministic segment IDs and create a view filtered to them
segment_ids = sorted(dataset.segment_ids())[:3]
view = dataset.filter_segments(segment_ids)

# Build a synthetic metadata table keyed by rerun_segment_id
base_time = datetime(2023, 11, 14, 22, 13, 20)
event_times = [base_time + timedelta(seconds=i) for i in range(3)]

meta = pa.record_batch(
    {
        "rerun_segment_id": segment_ids,
        "event_time": pa.array(event_times, type=pa.timestamp("ns")),
        "range_start": pa.array(event_times, type=pa.timestamp("ns")),
        "range_end": pa.array(
            [t + timedelta(milliseconds=500) for t in event_times],
            type=pa.timestamp("ns"),
        ),
        "entity_path": ["/camera/rgb", "/observation/joint_positions", "/observation/gripper_state"],
    },
)

ctx = client.ctx
meta_df = ctx.from_arrow(meta)
# endregion: setup

# region: basic
basic = view.segment_table().select("rerun_segment_id").sort("rerun_segment_id")
basic = basic.with_column("url", segment_url(dataset))
for url in basic.select("url").to_pydict()["url"]:
    print(url)
# endregion: basic

# region: timestamp
ts = view.segment_table(join_meta=meta_df).select("rerun_segment_id", "event_time")
ts = ts.sort("rerun_segment_id")
ts = ts.with_column("url", segment_url(dataset, timestamp="event_time", timeline_name="real_time"))
for url in ts.select("url").to_pydict()["url"]:
    print(url)
# endregion: timestamp

# region: time_range
tr = view.segment_table(join_meta=meta_df).select("rerun_segment_id", "range_start", "range_end")
tr = tr.sort("rerun_segment_id")
tr = tr.with_column(
    "url",
    segment_url(
        dataset,
        time_range_start="range_start",
        time_range_end="range_end",
        timeline_name="real_time",
    ),
)
for url in tr.select("url").to_pydict()["url"]:
    print(url)
# endregion: time_range

# region: selection
sel = view.segment_table(join_meta=meta_df).select("rerun_segment_id", "entity_path")
sel = sel.sort("rerun_segment_id")
sel = sel.with_column("url", segment_url(dataset, selection="entity_path"))
for url in sel.select("url").to_pydict()["url"]:
    print(url)
# endregion: selection

# region: combined
combined = view.segment_table(join_meta=meta_df).select(
    "rerun_segment_id", "event_time", "range_start", "range_end", "entity_path"
)
combined = combined.sort("rerun_segment_id")
combined = combined.with_column(
    "url",
    segment_url(
        dataset,
        timestamp="event_time",
        timeline_name="real_time",
        time_range_start="range_start",
        time_range_end="range_end",
        selection="entity_path",
    ),
)
for url in combined.select("url").to_pydict()["url"]:
    print(url)
# endregion: combined

# region: expressions
expr = view.segment_table(join_meta=meta_df).select("rerun_segment_id", "event_time")
expr = expr.sort("rerun_segment_id")
expr = expr.with_column(
    "url",
    segment_url(
        dataset,
        timestamp="event_time",
        timeline_name="real_time",
        selection=lit("/camera/rgb"),
    ),
)
for url in expr.select("url").to_pydict()["url"]:
    print(url)
# endregion: expressions
