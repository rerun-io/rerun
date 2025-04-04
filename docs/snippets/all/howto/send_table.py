"""Sets the recording properties."""

import rerun as rr
import pyarrow as pa
from rerun_bindings import ViewerClient

client = ViewerClient(addr="rerun+http://0.0.0.0:9876")
client.send_table(
    "Hello from Python",
    pa.RecordBatch.from_pydict({"Column A": [1, 2, 3], "Column B": ["https://www.rerun.io", "Hello", "World"]}),
)
