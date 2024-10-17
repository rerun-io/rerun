---
title: Operating Modes
order: 800
---

There are many different ways of sending data to the Rerun Viewer depending on what you're trying to achieve and whether the Viewer is running in the same process as your code, in another process, or even as a separate web application.

In the [official examples](/examples), these different modes of operation are exposed via a standardized set of flags that we'll cover below.
We will also demonstrate how you can achieve the same behavior in your own code.

## Operating modes

The Rerun SDK provides 4 modes of operation: `spawn`, `connect`, `serve` & `save`.

All four of them are optional: when none of these modes are active, the client will simply buffer the logged data in memory, waiting for one of these modes to be enabled so that it can flush it.

### Spawn

This is the default behavior you get when running all of our C++/Python/Rust examples, and is generally the most convenient when you're experimenting.

#### C++
`RecordingStream::spawn` spawns a new Rerun Viewer process using an executable available in your PATH, then streams all the data to it via TCP. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.

#### Python
Call [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn) once at the start of your program to start a Rerun Viewer in an external process and stream all the data to it via TCP. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.

#### Rust
[`RecordingStream::spawn`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.spawn) spawns a new Rerun Viewer process using an executable available in your PATH, then streams all the data to it via TCP. If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.


### Connect

Connects to a remote Rerun Viewer and streams all the data via TCP.

You will need to start a stand-alone Viewer first by typing `rerun` in your terminal.

#### C++
`RecordingStream::connect`

#### Python
[`rr.connect`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.connect)

#### Rust
[`RecordingStream::connect`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.connect)


### Serve

This starts the web version of the Rerun Viewer in your browser, and streams data to it in real-time using WebSockets.

#### C++
Not available yet.

#### Python
Use [`rr.serve`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.serve).

#### Rust
[`RecordingStream::serve`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.serve)


### Save

Streams all logging data into an `.rrd` file on disk, which can then be loaded into a stand-alone viewer.

To view the saved file, use `rerun path/to/file.rrd`.

⚠️ [RRD files are not yet stable across different versions!](https://github.com/rerun-io/rerun/issues/6410) ⚠️

#### C++
Use `RecordingStream::save`.

#### Python
Use [`rr.save`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.save).

#### Rust
Use [`RecordingStream::save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.save).


### Standard Input/Output

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


## Adding the standard flags to your programs

We provide helpers for both Python & Rust to effortlessly add and properly handle all of these flags in your programs.

- For Python, checkout the [`script_helpers`](https://ref.rerun.io/docs/python/stable/common/script_helpers/) module.
- For Rust, checkout our [`clap`]() [integration](https://docs.rs/rerun/latest/rerun/clap/index.html).

Have a look at the [official examples](/examples) to see these helpers in action.
