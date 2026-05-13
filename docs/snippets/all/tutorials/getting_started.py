"""Getting Started workflow: Catalog SDK regions (Python only).

The Log step lives in `tutorials/getting_started_log` because it uses only the
Logging SDK and is therefore available in Python, Rust, and C++.
"""

import math
from pathlib import Path

import rerun as rr

# Materialize the .rrd that the catalog regions below register against.
# Same code as the Log step's three-language snippet, repeated here so this file runs end-to-end.
with rr.RecordingStream("rerun_example_getting_started", recording_id="run-1") as _rec:
    _rec.save("run-1.rrd")
    for _t in range(10):
        _rec.set_time("t", duration=_t)
        _rec.log("/arm/shoulder", rr.Scalars(math.sin(_t * 0.5)))
        _rec.log("/arm/elbow", rr.Scalars(math.cos(_t * 0.5)))


# region: setup
client = rr.catalog.CatalogClient("rerun+http://127.0.0.1:51234")
# endregion: setup


# region: ingest
dataset = client.create_dataset("demo", exist_ok=True)
dataset.register([Path("run-1.rrd").absolute().as_uri()]).wait()
# endregion: ingest


# region: annotate
with rr.RecordingStream("rerun_example_getting_started", recording_id="run-1") as ann:
    ann.save("run-1-properties.rrd")
    ann.send_property("episode", rr.AnyValues(success=True, task="pick_and_place"))

dataset.register([Path("run-1-properties.rrd").absolute().as_uri()], layer_name="properties").wait()
# endregion: annotate


# region: query
df = dataset.filter_contents(["/arm/**"]).reader(index="t")
print(df.select("rerun_segment_id", "/arm/shoulder:Scalars:scalars", "/arm/elbow:Scalars:scalars"))
# endregion: query
