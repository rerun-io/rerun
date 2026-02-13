---
title: Send tables to Rerun
order: 300
description: Shows how to send tables as dataframes to the Rerun viewer.
---

> **Note:** The `send_table` API is currently experimental and may change in future releases.

Rerun now supports sending tabular data to the Rerun Viewer! This feature allows you to visualize and interact with dataframes (encoded as Arrow record batches) directly in the Rerun Viewer environment.

## Overview

The `send_table` API provides a straightforward way to send tabular data to the Rerun Viewer. This is particularly useful for:

- Inspecting dataframes alongside other visualizations
- Debugging data processing pipelines
- Presenting structured data in a readable format

## References

For complete examples of using `send_table`, please refer to:

- [🐍 Jupyter Notebook](https://github.com/rerun-io/rerun/blob/main/examples/notebook/notebook/send_table.ipynb)
- [🐍 Python SDK](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/send_table.py)

## Prerequisites

- Rerun SDK (Python)
- PyArrow library
- Pandas
- NumPy

Which can be installed via:

```sh
pip install rerun-sdk[notebook] pyarrow pandas numpy
```

## Basic usage

### Connecting to the Viewer

```python
from rerun.experimental import ViewerClient

# Connect to a running Rerun Viewer
client = ViewerClient(addr="rerun+http://0.0.0.0:9876/proxy")
```

### Sending a simple table

```python
import pyarrow as pa

# Create a record batch from a Python dictionary
record_batch = pa.RecordBatch.from_pydict({
    "id": [1, 2, 3],
    "url": ["https://www.rerun.io", "https://github.com/rerun-io/rerun", "https://crates.io/crates/rerun"],
})

# Send the table to the viewer with an identifier
client.send_table("My Table", record_batch)
```

### Using with Pandas dataframes

You can also send Pandas dataframes by converting them to a record batch:

```python
import pandas as pd
import numpy as np
import pyarrow as pa

# Create a sample DataFrame
dates = pd.date_range("20230101", periods=6)
df = pd.DataFrame(np.random.randn(6, 4), index=dates, columns=list("ABCD"))

# Convert to record batch and send to the viewer.
client.send_table("Pandas DataFrame", pa.RecordBatch.from_pandas(df))
```

## Using in Jupyter notebooks

Rerun provides special support for Jupyter notebooks, you can find more information here: [https://rerun.io/docs/howto/integrations/embed-notebooks]
Note that this API makes use of `rr.notebook.Viewer`:

```python
import rerun as rr
import pyarrow as pa

# For inline display
os.environ["RERUN_NOTEBOOK_ASSET"] = "inline"

# Create and display the viewer
viewer = rr.notebook.Viewer(width="auto", height="auto")
viewer.display()

# Send table directly to the inline viewer
viewer.send_table("My Table", pa.RecordBatch.from_pydict({
    "Column A": [1, 2, 3],
    "Column B": ["https://www.rerun.io", "Hello", "World"]
}))
```

You can also use the native viewer instead of the inline viewer:

```python
os.environ["RERUN_NOTEBOOK_ASSET"] = "serve-local"

# Connect to a running Rerun Viewer
client = ViewerClient(addr="rerun+http://0.0.0.0:9876/proxy")
```

## Current limitations

As this is an experimental API, there are several limitations to be aware of:

- Only a single record batch is supported per table
- Tables can't be saved/loaded from files yet (unlike `.rrd` files for recordings)
- Integration with the rest of the Rerun API is still in progress
- Rust and C++ support will be added after the API stabilizes
- The API may undergo significant changes as we iterate based on user feedback

## What's next

The `send_table` API is still evolving and we plan to tackle all of the limitations mentioned above.

We welcome your [feedback and suggestions](https://rerun.io/feedback) as we continue to improve this feature!
