"""Sets the recording properties."""

import pyarrow as pa
from rerun.experimental import ViewerClient

client = ViewerClient(addr="rerun+http://0.0.0.0:9876/proxy")
client.send_table(
    "Hello from Python",
    pa.RecordBatch.from_pydict({
        "id": [1, 2, 3],
        "url": ["https://www.rerun.io", "https://github.com/rerun-io/rerun", "https://crates.io/crates/rerun"],
    }),
)
