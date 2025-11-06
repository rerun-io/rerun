---
title: Analyze data via Open Source Server
order: 4
---

The Rerun Cloud offering builds on the open source core.
Towards that end, the Open Source Server provides the capability for small scale local analysis using a similar API surface.
This supports a workflow to first develop or debug locally on a single recording, and then scale up that same workflow on the cloud for production use.

<!-- TODO(RR-2818) add link to doc -->

# Open source server

## Launching the server

The server needs to be opened in a separate window.
Launch the server using the Rerun CLI.

```console
rerun server
```

For full details run

```console
rerun server --help
```

with the most common utility opening a directory of RRDs as a dataset in the server

```console
rerun server -d <directory_containing_rrds>
```

## Connecting to the server

When launching the server the CLI will print out the host and port it is listening on
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
dataset = client.get_dataset_entry(name="oss_demo")
```

Otherwise we can create a new dataset:

```python
dataset = client.create_dataset(
    name="oss_demo",
)
```

In order to add additional recordings to a dataset we use the `register` API.

```python
# For OSS server you must register files local to your machine
# To synchronously register a single recording
dataset.register(f"file://{os.path.abspath('oss_demo.rrd')}")
# To asynchronously register many recordings
timeout_seconds = 100
tasks = dataset.register_batch([f"file://{os.path.abspath('oss_demo.rrd')}"])
tasks.wait(100)
```

### Inspecting datasets

Ultimately, we will end up rendering the data as a [DataFusion DataFrame](https://datafusion.apache.org/python/user-guide/dataframe/index.html).
However, there is an intermediate step that allows for some optimization.
This generates a `DataFrameQueryView`. <!-- TODO(nick) add link to doc -->
The `DataFrameQueryView` allows selection of the subset of interest for the dataset (index column, and content columns), filtering to specific time ranges, and managing the sparsity of the data (`fill_latest_at`).
All of these operations occur on the server prior to evaluating future queries, so avoid unnecessary computation.

```python
view = (
    dataset
        .dataframe_query_view(index="log_time", contents="/**")
        # Select only a single or subset of recordings
        .filter_partition_id(record_of_interest)
        # Select subset of time range
        .filter_range_nanos(start=start_of_interest, end=end_of_interest)
        # Forward fill for time alignment
        .fill_latest_at()
)
```

After we have identified what data we want, we can get a DataFrame.

```python
df = view.df()
```

[DataFusion](https://datafusion.apache.org/python/) provides a pythonic dataframe interface to your data as well as [SQL](https://datafusion.apache.org/python/user-guide/sql.html).
After performing a series of operations, this dataframe can be materialized and returned in common data formats. For example:

```python
pandas_df = df.to_pandas()
polars_df = df.to_polars()
arrow_table = df.to_arrow_table()
```
