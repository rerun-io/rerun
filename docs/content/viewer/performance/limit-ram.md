---
title: Limit the viewer's memory usage
order: 100
description: Set the Viewer's memory limit, find out why memory keeps growing, and open recordings that are larger than RAM
---

### --memory-limit

The more data you log, the more RAM the Rerun Viewer uses.
When it reaches its memory limit, it drops data to stay under the limit.
The default limit is 75% of the total system RAM on native, and 2.5 GB in the web Viewer.

You can set the limit with the `--memory-limit` command-line argument, or the `memory_limit` argument of [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn).
Both accept an absolute size such as `16GB`, or a fraction of system RAM such as `50%`.

Alternatively, you can adjust the limit in the Viewer's settings.
This setting is remembered between sessions, but `--memory-limit` overrides it, and `rr.spawn` always passes that flag (defaulting to `75%`).

To find out what is using up the memory, see [Diagnose latency and performance](diagnose-performance.md).

## Why does memory keep growing?

If the Viewer uses more memory than you expect, or seems to ignore `--memory-limit`, check the following.

### The new limit was never applied

`rr.spawn()` and `rerun` do not start a new Viewer if one is already listening on the same port (9876 by default).
They connect to the running Viewer instead, so the `--memory-limit` or `memory_limit` you pass this time is ignored.
Close the old Viewer first, or start a new one on another port:

```sh
rerun --memory-limit 4GB --port auto
```

From Python, pass a different `port` to `rr.spawn`.

### Static data is never dropped

Data logged with `static=True` is never garbage-collected.
Logging static data to the same entity again replaces the old value, but static data on many different entity paths (for example a new entity path for every frame) adds up without bound.
Log data that changes over time on a timeline instead.
See [static data](../../logging-and-ingestion/data-model/static.md).

### The latest value of each component is kept

For recordings that the Viewer cannot download again (for example data streamed live from the SDK), it always keeps the latest value of every component of every entity, so that the current state stays visible.
If you log many entities, or entities with large components, this floor can be bigger than your memory limit.

### The gRPC server has its own buffer

The SDK sends data to a gRPC server that runs inside the Viewer process.
That server keeps a copy of the data for Viewers that connect later.
Its size is capped separately with `--server-memory-limit` (default `1GiB`), or the `server_memory_limit` argument of `rr.spawn` and `rr.serve_grpc`.
This buffer also counts toward `--memory-limit` when you use the default limit.
With a lower `--memory-limit`, it counts only when the process uses more memory than the default limit.

## Can I open recordings that are larger than RAM?

Yes, as long as the Viewer can read the recording on demand.
The Viewer then keeps only part of the recording in memory, downloads the chunks it needs for the current time, and drops chunks it no longer needs.
This works when you:

* Open a local `.rrd` file in the native or web Viewer.
  Local `.rrd` files are loaded through the Viewer catalog by default (**Settings** → **Viewer catalog** → **Load files via Viewer catalog**).
* Open a recording from a Rerun server, for example one started with `rerun server`.

It does **not** work for data that streams in live from the SDK, or for `.rrd` files when you have turned off **Load files via Viewer catalog**.
The Viewer loads that data fully into memory, and drops the oldest data when it reaches its memory limit.

### Make sure the file has a footer

On-demand loading needs the footer at the end of the `.rrd` file.
The SDK and the `rerun rrd` tools write the footer when they close the file.
A file can be missing its footer if the process that wrote it was killed, or if it was written by an old version of Rerun.
Without a footer, the whole file is read into memory.

To check a file, and to rewrite it with a footer:

```sh
rerun rrd verify my_recording.rrd
rerun rrd optimize my_recording.rrd -o my_recording_optimized.rrd
```

`rerun rrd optimize` also compacts the chunks, which makes on-demand loading faster.
See [Optimize chunk count](../../logging-and-ingestion/chunks-and-rrd/optimize-chunks.md).

### Serve many large files

To browse a directory of large recordings, serve it with a local Rerun server and open it from the Viewer:

```sh
rerun server -d my_dataset=/path/to/recordings
rerun rerun+http://localhost:51234
```

### The web Viewer

The web Viewer runs as 32-bit WebAssembly, so it can address at most 4 GiB of memory, and its default memory limit is 2.5 GB.
With **Load files via Viewer catalog** turned on (the default), it can still open `.rrd` files that are larger than that.
