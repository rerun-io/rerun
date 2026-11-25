"""Send tables to a running Viewer."""

# region: connect
from rerun.experimental import ViewerClient

# Connect to a running Rerun Viewer
client = ViewerClient.connect(url="rerun+http://127.0.0.1:9876/proxy")
# endregion: connect

# region: simple_table
import pyarrow as pa

# Create a record batch from a Python dictionary
record_batch = pa.RecordBatch.from_pydict({
    "id": [1, 2, 3],
    "url": [
        "https://www.rerun.io",
        "https://github.com/rerun-io/rerun",
        "https://crates.io/crates/rerun",
    ],
})

# Send the table to the viewer with an identifier
client.send_table("My Table", record_batch)
# endregion: simple_table

# region: pandas
import numpy as np
import pandas as pd

# Create a sample DataFrame
dates = pd.date_range("20230101", periods=6)
df = pd.DataFrame(
    np.random.default_rng(0).standard_normal((6, 4)),
    index=dates,
    columns=list("ABCD"),
)

# Convert to record batch and send to the viewer.
client.send_table("Pandas DataFrame", pa.RecordBatch.from_pandas(df))
# endregion: pandas
