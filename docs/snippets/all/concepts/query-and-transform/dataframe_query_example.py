"""Query and display the first 10 rows of a recording."""

import atexit
import math
import os
import tempfile

from datafusion import col

import rerun as rr


# should be a cross-platform way to generate a rrd path.
RRD_PATH = tempfile.mktemp(suffix=".rrd")
atexit.register(lambda: os.unlink(RRD_PATH) if os.path.exists(RRD_PATH) else None)

# region: setup
# create some data
times = list(range(64))
scalars = [math.sin(t / 10.0) for t in times]

# log the data to a temporary recording
with rr.RecordingStream("rerun_example_dataframe_query") as rec:
    rec.save(RRD_PATH)
    rec.send_columns(
        "/data",
        indexes=[rr.TimeColumn("step", sequence=times)],
        columns=rr.Scalars.columns(scalars=scalars),
    )
# endregion: setup


# region: query
# load the demo recording in a temporary catalog
with rr.server.Server(datasets={"dataset": [RRD_PATH]}) as server:
    # obtain a dataset from the catalog
    dataset = server.client().get_dataset("dataset")

    # (optional) filter interesting data
    dataset_view = dataset.filter_contents("/data")

    # obtain a DataFusion dataframe
    df = dataset_view.reader(index="step")

    # (optional) filter rows using DataFusion expressions
    df = df.filter(col("/data:Scalars:scalars")[0] > 0.95)

    # execute the query
    print(df)  # or convert to Pandas, Polars, PyArrow, etc.
# endregion: query
