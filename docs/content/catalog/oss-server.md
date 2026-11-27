---
title: The open-source catalog server
order: 150
description: What the local rerun server does, what it persists, and how Hub differs
---

`rerun server` (or `rr.server.Server` in Python) runs an open-source [catalog](catalog-object-model.md) server on your machine.
It serves datasets of `.rrd` files and Lance tables to the Viewer and to the catalog SDK.
It is meant for local development and small setups.
For persistent, shared storage, use [Rerun Hub](hub.md), our commercial managed offering for production deployments. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->
Code written against the open-source server also runs against Hub (see [Run the examples on Hub](hub.md#run-the-examples-on-hub)).

## Starting the server

```sh
rerun server -d my_dataset=/path/to/recordings -t my_table=/path/to/table.lance
```

`-d` creates a dataset from every `.rrd` file in a directory, and `-t` loads a Lance table.
Both can be repeated.
Without them, the server starts with an empty catalog, and you can create datasets from the SDK.
See the [CLI reference](../reference/cli.md#rerun-server) for the port, host, CORS, and all other options.

In Python:

```python
import rerun as rr

server = rr.server.Server(
    port=51234,
    datasets={
        "my_dataset": "/path/to/recordings",  # a directory
        "other_dataset": ["a.rrd", "b.rrd"],  # a list of files
    },
)
client = server.client()
```

Connect a Viewer with `rerun rerun+http://localhost:51234`.
See [Query data out of Rerun](get-data-out.md) for querying.

## Everything is in memory

The server keeps the catalog (datasets, entries, and their IDs) in memory.
When it stops, everything you created or registered while it ran is gone.

- On restart, the server creates new IDs for its datasets and tables.
  Dataset names stay the same, but URLs that contain a dataset ID (such as the ones from [`segment_url`](../query-and-transform/dataframes/segment_url.md)) stop working.
- Segment IDs are the recording IDs of the `.rrd` files, or the `segment_id` passed to `dataset.register()`, so they stay the same as long as you load the same files the same way.
- Datasets that you create with `client.create_dataset()`, and files that you register with `dataset.register()`, must be created and registered again after a restart.

The `.rrd` files themselves stay on disk.
Files that have a footer index are read from disk on demand; files without one are loaded into memory in full.
Run [`rerun rrd optimize`](../reference/cli.md#rerun-rrd-optimize) on a file to add a footer before serving it.

## Directories are scanned once

`-d` (and a directory in `datasets=`) reads the directory once, at startup.
It only loads files that end in `.rrd`, and does not look in subdirectories.
The server does not watch the directory for new files (see [missing features](../development/roadmap.md#missing-features)).
To add new files, register them with `dataset.register()`, or restart the server.

## Only local files can be registered

`dataset.register()` on the open-source server accepts `file://` URIs of files on the server's own file system:

```python
from pathlib import Path

dataset.register([Path("recording.rrd").resolve().as_uri()]).wait()
```

URIs for object stores, such as `s3://` or `gs://`, are not supported.
To register recordings from object storage, use [Rerun Hub](hub.md), or download the files first.
