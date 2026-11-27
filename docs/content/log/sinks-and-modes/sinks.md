---
title: Sinks
order: 100
description: Sinks are the destinations logged data goes to, such as files or a Viewer
---

Sinks control where your Rerun data goes. They are the output destinations for your logged data.

Data can go to a live Viewer, a file on disk, memory, or several destinations at once.

## Available sink types

Rerun provides these built-in sink types:

### GrpcSink

Streams data to a Rerun Viewer over gRPC. This is the most common sink for live visualization.

snippet: concepts/grpc_sink

### FileSink

Writes data to `.rrd` files on disk.

snippet: concepts/file_sink

## Multiple sinks (Tee pattern)

Pass several sinks to `set_sinks` to send data to all of them at once.
For example, a `GrpcSink` and a `FileSink` let you view data live and save it to disk in the same run:

snippet: howto/set_sinks

You can also stream to several Viewers or save to several files.

Calling `rr.spawn()`, `rr.connect_grpc()`, or `rr.save()` replaces all existing sinks.
To start a Viewer without touching your sinks, call `rr.spawn(connect=False)`: it only starts the Viewer process, and your `GrpcSink` can then connect to it.

```python
rr.init("rerun_example_spawn_and_save")
rr.spawn(connect=False)  # Start a Viewer, but leave the sinks alone.
rr.set_sinks(rr.GrpcSink(), rr.FileSink("data.rrd"))
```

## Connection lifecycle

These rules apply to the gRPC sink (`connect_grpc`, `spawn`, and `GrpcSink`):

- **Connecting:** the SDK connects in the background and retries forever, with a backoff of up to one second between attempts.
  You can start logging before the Viewer is running: data is queued until the connection is up.
  If the queue fills up before a connection is made, log calls block.
- **Flushing:** `flush` waits for queued data to be sent.
  If the SDK has not connected yet, `flush` gives up after about 5 seconds.
  In Python, use `rec.flush(timeout_sec=…)` on a `RecordingStream` (for example the one returned by `rr.get_global_data_recording()`) to limit how long it waits; it raises an error on timeout.
- **Shutdown:** at shutdown, the SDK waits at most about 5 seconds for a final connection attempt.
  If it still cannot connect, the queued data is dropped and an error is logged.
- **Disconnects:** if an established connection fails (for example because the Viewer was closed), the SDK does _not_ reconnect.
  Data logged after that is lost for this sink.
  If you need the data, also log to a `FileSink`.
- **Status:** only the Rust SDK exposes the connection state, through the experimental `GrpcSink::status`.

If the Viewer cannot start because its port is taken, run `rerun --port auto` (or `rerun --new`) to start a new Viewer on a free port, and connect to the port it prints.

## See also

- [Recordings](../data-model/recordings.md): Understand how recordings relate to sinks
- [Blueprints](../../viewer/inspect-recordings/recording-layout.md): Learn how to configure the viewer's layout
- API References:
  - [🐍 Python sinks API](https://ref.rerun.io/docs/python/stable/rerun/)
  - [🦀 Rust RecordingStream](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html)
  - [🌊 C++ RecordingStream](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html)
