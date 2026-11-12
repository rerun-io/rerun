---
title: The open-source catalog server
order: 700
description: What the local rerun server does, what it persists, and how Hub differs
---

`rerun server` (or `rr.server.Server` in Python) runs an open-source [catalog](catalog-object-model.md) server on your machine.
It serves datasets of `.rrd` files and Lance tables to the Viewer and to the catalog SDK.
It is meant for local development and small setups.
For persistent, shared storage, use [Rerun Hub](../../hub.md).

## Starting the server

```sh
rerun server -d my_dataset=/path/to/recordings -t my_table=/path/to/table.lance
```

You can run `rerun server` without any options.
Without `-d` and `-t`, the server starts with an empty catalog, and you can create datasets from the SDK.

- `-d, --dataset [NAME=]DIR` creates a dataset from every `.rrd` file in a directory.
  Without `NAME=`, the dataset is named after the directory.
  Can be repeated.
- `-t, --table [NAME=]PATH` loads a Lance table.
  Can be repeated.
- `-p, --port` sets the port (default `51234`), and `--host` the address to listen on (default `0.0.0.0`).
- `--cors-allow-origin` allows web Viewers on other origins than `localhost`, `127.0.0.1`, and `rerun.io` (and its subdomains).

See the [CLI reference](../../reference/cli.md#rerun-server) for all options.

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
  Dataset names stay the same, but URLs that contain a dataset ID (such as the ones from [`segment_url`](../dataframes/segment_url.md)) stop working.
- Segment IDs are the recording IDs of the `.rrd` files, so they stay the same as long as you load the same files.
- Datasets that you create with `client.create_dataset()`, and files that you register with `dataset.register()`, must be created and registered again after a restart.

The `.rrd` files themselves stay on disk.
Files that have a footer index are read from disk on demand; files without one are loaded into memory in full.

## Directories are scanned once

`-d` (and a directory in `datasets=`) reads the directory once, at startup.
It only loads files that end in `.rrd`, and does not look in subdirectories.
The server does not watch the directory for new files (see [missing features](../../development/roadmap.md#missing-features)).
To add new files, register them with `dataset.register()`, or restart the server.

## Only local files can be registered

`dataset.register()` on the open-source server accepts `file://` URIs of files on the server's own file system:

```python
from pathlib import Path

dataset.register([Path("recording.rrd").resolve().as_uri()]).wait()
```

URIs for object stores, such as `s3://` or `gs://`, are not supported.
To register recordings from object storage, use Rerun Hub, or download the files first.

## Compared to Rerun Hub

|                  | Open-source server                  | Rerun Hub                                 |
| ---------------- | ----------------------------------- | ----------------------------------------- |
| Catalog storage  | In memory, lost on restart          | Persistent                                |
| Registration     | Local `file://` paths only          | Object storage, such as `s3://`           |
| Authentication   | None                                | Login or token (see `rerun auth`)         |
| Deployment       | One process that you run yourself   | Managed service                           |

See [Rerun Hub](../../hub.md) for more.
