---
title: Analyze data via Open Source Server
order: 4
---

The Rerun Cloud offering builds on the open source core.
Towards that end, the Open Source Server provides the capability for small scale local analysis using a similar API surface.
This supports a workflow to first develop or debug locally on a single recording, and then scale up that same workflow on the cloud for production use.

<!-- TODO(RR-2818) add link to doc -->

## Launching the server

### Commandline

The server needs to be opened in a separate window.
Launch the server using the Rerun CLI:

```console
rerun server
```

You can also pass a directory containing RRDs to be opened as a dataset in the server:

```console
rerun server -d directory_containing_rrds/
```

For all available options, run:

```console
rerun server --help
```

### SDK

The server can also be managed with a python object.
The server will shutdown when the object goes out of scope.

```python
server = rr.server.Server()
client = server.client()
# Or as a context manager
with rr.server.Server() as srv:
    client = srv.client()
```

## Connecting to the server

When launching the server, the CLI will print out the host and port it is listening on
(defaulting to: `localhost:51234`).

### From the viewer

Either specify the network location with the CLI at launch:

```console
rerun connect localhost:51234
```

or open the command palette in the viewer (`cmd/ctrl + P` or via the menu) and enter/select `Add Redap server`.
Set the scheme to `http` and enter the hostname and port in the dialog.

### From the SDK

```python
import rerun as rr
CATALOG_URL = "rerun+http://localhost:51234"
client = rr.catalog.CatalogClient(CATALOG_URL)
```

## Querying the server

Everything below assumes that the server has been launched and a client has been constructed based on instructions above.

### Datasets overview

A dataset is a collection of recordings that can be queried against.
If we have already created a dataset, we can retrieve it via:

```python
dataset = client.get_dataset(name="oss_demo")
```

Otherwise we can create a new dataset:

```python
dataset = client.create_dataset(
    name="oss_demo",
)
```

We can list all of the existing datasets with:

```python
client.datasets()
```

In order to add additional recordings to a dataset we use the `register` API.

```python
# For OSS server you must register files local to your machine
# To synchronously register a single recording
dataset.register(Path("oss_demo.rrd").resolve().as_uri()).wait()
# To asynchronously register many recordings
handle = dataset.register([Path("oss_demo.rrd").resolve().as_uri()])
handle.wait(timeout_secs=100)
```

### Inspecting datasets

Ultimately, we will end up rendering the data as a [DataFusion DataFrame](https://datafusion.apache.org/python/user-guide/dataframe/index.html).
You can use `filter_segments()` and `filter_contents()` to create a `DatasetView` that selects a subset of the dataset, then call `reader(index=...)` to get a DataFrame. <!-- NOLINT -->
These filtering operations occur on the server prior to evaluating future queries, avoiding unnecessary computation.

```python
from datafusion import col

# Create a view filtering to specific segments and content
view = (
    dataset
        .filter_segments(segments_of_interest)
        .filter_contents(["/camera/**", "/lidar/**"])
)

# Get a DataFrame with an index and optional latest-at fill
df = view.reader(index="log_time", fill_latest_at=True)

# Row-level filtering is done on the DataFrame using DataFusion
df = df.filter(col("log_time") >= start_of_interest)
```

[DataFusion](https://datafusion.apache.org/python/) provides a pythonic dataframe interface to your data as well as [SQL](https://datafusion.apache.org/python/user-guide/sql.html).
After performing a series of operations, this dataframe can be materialized and returned in common data formats. For example:

```python
pandas_df = df.to_pandas()
polars_df = df.to_polars()
arrow_table = df.to_arrow_table()
```
