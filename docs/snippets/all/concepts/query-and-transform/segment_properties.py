"""Query and display the first 10 rows of a recording."""

import atexit

from datafusion import col

import shutil
from pathlib import Path
import os
import tempfile

import rerun as rr

RRD_DIR = Path(tempfile.mkdtemp())
atexit.register(lambda: shutil.rmtree(RRD_DIR) if os.path.exists(RRD_DIR) else None)

# region: setup
rrd_paths = [RRD_DIR / f"recording_{i}.rrd" for i in range(5)]
for i, rrd_path in enumerate(rrd_paths):
    with rr.RecordingStream("rerun_example_property") as rec:
        rec.save(rrd_path)
        rec.log("data", rr.Points2D(positions=[[i, i]]))

        # properties can be any rerun data
        rec.send_property("location", rr.GeoPoints(lat_lon=[[46.5, 6.5]]))

        # custom data can be logged with `AnyValues`
        rec.send_property(
            "info",
            rr.AnyValues(
                index=i,
                is_odd=i % 2 == 1,
            ),
        )

        # recording name is part of the built-in properties
        rr.send_recording_name(f"segment_{i}")
# endregion: setup


# region: segment_table
# load the demo recording in a temporary catalog
with rr.server.Server(datasets={"dataset": rrd_paths}) as server:
    # obtain a dataset from the catalog
    dataset = server.client().get_dataset("dataset")

    segment_table = dataset.segment_table()

    # sort and select columns of interest
    segment_table = segment_table.sort(col("property:RecordingInfo:name")[0]).select(
        "rerun_segment_id",
        "property:RecordingInfo:name",
        "property:RecordingInfo:start_time",
        "property:info:index",
        "property:info:is_odd",
        "property:location:GeoPoints:positions",
    )

    print(segment_table)
    # endregion: segment_table

    # region: filter
    interesting_segments = segment_table.filter(col("property:info:is_odd")[0])

    print(interesting_segments)

    # endregion: filter

    # region: query
    df = dataset.filter_contents("__properties/**").reader(index=None)
    df = df.sort("property:RecordingInfo:name").select(
        "rerun_segment_id", "property:RecordingInfo:name", "property:info:index"
    )

    print(df)
    # endregion: query
