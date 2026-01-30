---
title: Operating Modes
order: 800
---

There are many different ways of sending data to the Rerun Viewer depending on what you're trying to achieve and whether the Viewer is running in the same process as your code, in another process, or even as a separate web application.

In the [official examples](/examples), these different modes of operation are exposed via a standardized set of flags that we'll cover below.
We will also demonstrate how you can achieve the same behavior in your own code.

Before reading this document, you might want to familiarize yourself with the [Rerun application model](../../concepts/how-does-rerun-work.md).

## Operating modes

The Rerun SDK provides multiple modes of operation: `spawn`, `connect_grpc`, `serve_grpc`, `save`, and `stdout`.

All of them are optional: when none of these modes are active, the client will simply buffer the logged data in memory, waiting for one of these modes to be enabled so that it can flush it.

> [!WARNING]
> These modes will override each other and destroy any existing sinks; if you want to run multiple sinks concurrently, you'll need to use `set_sinks()`.

### `spawn`

This is the default behavior you get when running all of our C++/Python/Rust examples, and is generally the most convenient when you're experimenting.

#### C++
`RecordingStream::spawn` spawns a new Rerun Viewer process using an executable available in your PATH, then streams all the data to it via gRPC. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.

#### Python
Call [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn) once at the start of your program to start a Rerun Viewer in an external process and stream all the data to it via gRPC. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.

#### Rust
[`RecordingStream::spawn`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.spawn) spawns a new Rerun Viewer process using an executable available in your PATH, then streams all the data to it via gRPC. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.


### `connect_grpc`

Connects to a remote Rerun Viewer and streams all the data via gRPC.

You will need to start a stand-alone Viewer first by typing `rerun` in your terminal.

#### C++
`RecordingStream::connect_grpc`

#### Python
[`rr.connect_grpc`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.connect_grpc)

#### Rust
[`RecordingStream::connect_grpc`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.connect_grpc)


### `serve_grpc`
Calling `serve_grpc` will start a Rerun gRPC server in your process, and stream logged data to it.
This gRPC server can then be connected to from the Rerun Viewer, e.g. by running `rerun --connect`.
The gRPC server acts as a proxy, buffering and forwarding log data to the Rerun Viewer.

You can also connect to the gRPC server from a Rerun Web Viewer.
To host a Rerun Web Viewer, you can use the `serve_web_viewer` function.

snippet: howto/serve_web_viewer

#### C++
* [`RecordingStream::serve_grpc`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html).
* TODO(#4638): `serve_web_viewer` is not available.

#### Python
* [`rr.serve_grpc`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.serve_grpc)
* [`rr.serve_web_viewer`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.serve_web_viewer)

#### Rust
* [`RecordingStream::serve_grpc`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.serve_grpc)
* [`RecordingStream::serve_web_viewer`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.serve_web_viewer)


### `save`

Streams all logging data into an `.rrd` file on disk, which can then be loaded into a stand-alone viewer.

To view the saved file, use `rerun path/to/file.rrd`.

**Note:** RRD files saved with Rerun 0.23 or later can be opened with a newer Rerun version.
For more details and potential limitations, please refer to [our blog post](https://rerun.io/blog/release-0.23).

⚠️ At the moment, we only guarantee compatibility across adjacent minor versions (e.g. Rerun 0.24 can open RRDs from 0.23).

#### C++
Use `RecordingStream::save`.

#### Python
Use [`rr.save`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.save).

#### Rust
Use [`RecordingStream::save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.save).


### `stdout`

Streams all logging data to standard output, which can then be loaded by the Rerun Viewer by streaming it from standard input.

#### C++

Use [`RecordingStream::stdout`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html).

Check out our [dedicated example](https://github.com/rerun-io/rerun/tree/latest/examples/cpp/stdio/main.cpp).

#### Python

Use [`rr.stdout`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.stdout).

Check out our [dedicated example](https://github.com/rerun-io/rerun/tree/latest/examples/python/stdio/stdio.py).

#### Rust

Use [`RecordingStream::stdout`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.stdout).

Check out our [dedicated example](https://github.com/rerun-io/rerun/tree/latest/examples/rust/stdio/src/main.rs).

### `set_sinks`

You can use this to log to multiple sinks concurrently, such as saving to disk while streaming to the viewer.

snippet: howto/set_sinks

#### Python

Use [`rr.set_sinks`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.set_sinks)

#### Rust

Use [`RecordingStream::set_sinks`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_sinks)

#### C++

Use [`RecordingStream::set_sinks`](https://ref.rerun.io/docs/cpp/classrerun_1_1RecordingStream.html#a92c9d3ecd3007d87b9c801fa33b140dc)


## Adding the standard flags to your programs

We provide helpers for both Python & Rust to effortlessly add and properly handle all of these flags in your programs.

- For Python, checkout the [`script_helpers`](https://ref.rerun.io/docs/python/stable/common/script_helpers/) module.
- For Rust, checkout our [`clap`]() [integration](https://docs.rs/rerun/latest/rerun/clap/index.html).

Have a look at the [official examples](/examples) to see these helpers in action.
