---
title: Send tables to Rerun
order: 400
description: Send tables as Arrow record batches and explore them in the Viewer
---

> [!NOTE]
> The `send_table` API is currently experimental and may change in future releases.

This page shows how to send tabular data (dataframes encoded as Arrow record batches) to the Rerun Viewer, where you can view and interact with it.
These tables are listed in the Sources panel, next to your recordings.
To persist a table on a catalog server instead, see [Catalog tables](../../catalog/catalog-tables.md).
To log a table as part of a recording, use [`rr.send_dataframe`](send-dataframe.md).

## Overview

The `send_table` API sends tabular data to the Rerun Viewer.
This is useful for:

- Debugging data processing pipelines
- Presenting structured data in a readable format
- Sorting and filtering tables interactively

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

snippet: howto/send_table[connect]

### Sending a simple table

snippet: howto/send_table[simple_table]

### Using with Pandas dataframes

You can also send Pandas dataframes by converting them to a record batch:

snippet: howto/send_table[pandas]

## Using in Jupyter notebooks

Rerun provides special support for Jupyter notebooks, see [Embed Rerun in notebooks](../../viewer/integrations/embed-notebooks.md).
Note that this API makes use of `rr.notebook.Viewer`:

```python
import os

import pyarrow as pa
import rerun as rr

# Embed the widget JavaScript directly in the notebook instead of fetching it from app.rerun.io
os.environ["RERUN_NOTEBOOK_ASSET"] = "inline"

# Create and display the viewer
viewer = rr.notebook.Viewer(width="auto", height="auto")
viewer.display()

# Send table directly to the inline viewer
viewer.send_table(
    "My Table",
    pa.RecordBatch.from_pydict({"Column A": [1, 2, 3], "Column B": ["https://www.rerun.io", "Hello", "World"]}),
)
```

## Current limitations

As this is an experimental API, there are several limitations to be aware of:

- Multiple record batches (or a DataFusion DataFrame) can be passed, but they are merged into a single record batch before sending
- Tables can't be saved to or loaded from files (unlike `.rrd` files for recordings)
- Only the Python SDK can send tables; the Rust and C++ SDKs cannot

## References

For complete examples of using `send_table`, please refer to:

- [🐍 Jupyter Notebook](https://github.com/rerun-io/rerun/blob/main/examples/notebook/notebook/send_table.ipynb)
- [🐍 Python SDK](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/send_table.py)
