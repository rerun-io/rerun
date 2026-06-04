"""Getting Started workflow: Catalog SDK regions (Python only).

The Log step lives in `tutorials/getting_started_log` because it uses only the
Logging SDK and is therefore available in Python, Rust, and C++.
"""

import math
import os
import tempfile
from pathlib import Path

import torch.multiprocessing

import rerun as rr

# Rerun's tokio runtime is not fork-safe; DataLoader workers must use `spawn`.
torch.multiprocessing.set_start_method("spawn", force=True)

# Run from a fresh temp dir so the .rrd files this snippet writes don't
# collide with other snippets executing in parallel from the same cwd.
os.chdir(tempfile.mkdtemp())

# Materialize the .rrd that the catalog regions below register against.
# Same code as the Log step's three-language snippet, repeated here so this
# file runs end-to-end.
with rr.RecordingStream(
    "rerun_example_getting_started", recording_id="run-1"
) as _rec:
    _rec.save("run-1.rrd")
    for _t in range(10):
        _rec.set_time("step", sequence=_t)
        _rec.log("/arm/shoulder", rr.Scalars(math.sin(_t * 0.5)))
        _rec.log("/arm/elbow", rr.Scalars(math.cos(_t * 0.5)))

# Start an in-process catalog server on a random port so this snippet runs
# end-to-end. In a real workflow you'd run `rerun server` in a separate
# terminal, which is what the docs show.
_server = rr.server.Server()
server_url = _server.url()


# region: setup
# `server_url` is the catalog URL — defaults to "rerun+http://127.0.0.1:51234"
# when running `rerun server` locally.
client = rr.catalog.CatalogClient(server_url)
# endregion: setup


# region: ingest
dataset = client.create_dataset("demo", exist_ok=True)
dataset.register([Path("run-1.rrd").absolute().as_uri()]).wait()
# endregion: ingest


# region: annotate
with rr.RecordingStream(
    "rerun_example_getting_started", recording_id="run-1"
) as ann:
    ann.save("run-1-properties.rrd")
    ann.send_property(
        "episode", rr.AnyValues(success=True, task="pick_and_place")
    )

dataset.register(
    [Path("run-1-properties.rrd").absolute().as_uri()], layer_name="properties"
).wait()
# endregion: annotate


# region: query
df = dataset.filter_contents(["/arm/**"]).reader(index="step")
print(
    df.select(
        "rerun_segment_id",
        "/arm/shoulder:Scalars:scalars",
        "/arm/elbow:Scalars:scalars",
    )
)
# endregion: query

# region: train
from torch.utils.data import DataLoader

from rerun.experimental.dataloader import (
    DataSource,
    Field,
    NumericDecoder,
    RerunIterableDataset,
)

ds = RerunIterableDataset(
    source=DataSource(dataset=dataset),
    index="step",
    fields={
        "shoulder": Field(
            "/arm/shoulder:Scalars:scalars", decode=NumericDecoder()
        ),
        "elbow": Field("/arm/elbow:Scalars:scalars", decode=NumericDecoder()),
    },
)

for batch in DataLoader(ds, batch_size=4):
    print(batch)
# endregion: train
