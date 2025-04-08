---
title: Application model
order: 0
---

The Rerun distribution comes with numerous moving pieces:
* The **SDKs** (Python, Rust & C++), for logging data and querying it back. These are libraries running directly in the end user's process.
* The **Native Viewer**: the Rerun GUI application for native platforms (Linux, macOS, Windows).
* The **Web Viewer**, which packs the **Native Viewer** into a Wasm application that can run on the Web and its derivatives (notebooks, etc).
* The **gRPC server**, which receives data from the **SDKs** and forwards it to the **Native Viewer** and/or **Web Viewer**. The communication is unidirectional: clients push data into the connection, never the other way around.
* The **Web/HTTP Server**, for serving the web page that hosts the **Web Viewer**.
* The **CLI**, which allows you to control all the pieces above as well as manipulate RRD files.

The **Native Viewer** always includes:
  * A **Chunk Store**: an in-memory database that stores the logged data.
  * A **Renderer**: a 3D engine that renders the contents of the **Chunk Store**.


## What runs where?

This is a lot to take in at first, but as we'll see these different pieces are generally deployed in just a few unique configurations for most common cases.

The first thing to understand is what process do each of these things run in.

The **CLI**, **Native Viewer**, **gRPC server**, and **Web/HTTP Server** are all part of the same binary: `rerun`.
Some of them can be enabled or disabled on demand using the appropriate flags but, no matter what, all these pieces are part of the same binary and execute in the same process.
Keep in mind that even the **Native Viewer** can be disabled (headless mode).

The **SDKs** are vanilla software libraries and therefore always executes in the same context as the end-user's code.

Finally, the **Web Viewer** is a Wasm application and therefore has its own dedicated `.wasm` artifact, and always runs in isolation in the end-user's web browser.

The best way to make sense of it all it to look at some of the most common scenarios when:
* Logging and visualizing data on native.
* Logging data on native and visualizing it on the web.


## Logging and visualizing data on native

There are two common sub-scenarios when working natively:
* Data is being logged and visualized at the same time (synchronous workflow).
* Data is being logged first to some persistent storage, and visualized at a later time (asynchronous workflow).


### Synchronous workflow

This is the most common kind of Rerun deployment, and also the simplest: one or more **SDKs**, embedded into the user's process, are logging data directly to a **gRPC server**, which in turns feeds the **Native Viewer**.
Both the **Native Viewer** and the **gRPC server** are running in the same `rerun` process.

Logging script:

snippet: concepts/app-model/native-sync

Deployment:
<!-- TODO(#7768): talk about rr.spawn(serve=True) once that's thing -->
```sh
# Start the Rerun Native Viewer in the background.
#
# This will also start the gRPC server on its default port (9876, use `--port`
# to pick another one).
#
# We could also have just used `spawn()` instead of `connect()` in the logging
# script above, and # we wouldn't have had to start the Native Viewer manually.
# `spawn()` does exactly this: it fork-execs a Native Viewer in the background
# using the first `rerun` # binary available # on your $PATH.
$ rerun &

# Start logging data. It will be pushed to the Native Viewer through the gRPC link.
$ ./logging_script
```


Dataflow:

<picture>
  <img src="https://static.rerun.io/rerun_native_sync/df05102a1dd04839ffec8442e5e9ffe65e9649db/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun_native_sync/df05102a1dd04839ffec8442e5e9ffe65e9649db/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun_native_sync/df05102a1dd04839ffec8442e5e9ffe65e9649db/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun_native_sync/df05102a1dd04839ffec8442e5e9ffe65e9649db/1024w.png">
</picture>


Reference:
* [SDK operating modes: `connect`](../reference/sdk/operating-modes.md#connect)
* [🐍 Python `connect`](https://ref.rerun.io/docs/python/0.19.0/common/initialization_functions/#rerun.connect)
* [🦀 Rust `connect`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.connect)
* [🌊 C++ `connect`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#aef3377ffaa2441b906d2bac94dd8fc64)

### Asynchronous workflow

The asynchronous native workflow is similarly simple: one or more **SDKs**, embedded into the user's process, are logging data directly to one or more files.
The user will then manually start the **Native Viewer** at some later point, in order to visualize these files.

Note: the `rerun` process still embeds both a **Native Viewer** and a **gRPC server**. For each **Native Viewer**, there is **always** an accompanying **gRPC server**, no exception.

Logging script:

snippet: concepts/app-model/native-async

Deployment:
```sh
# Log the data into one or more files.
$ ./logging_script

# Start the Rerun Native Viewer and feed it the RRD file directly.
#
# This will also start the gRPC server on its default port (9876, use `--port`
# to pick another one). Although it is not used yet, some client might want
# to connect in the future.
$ rerun /tmp/my_recording.rrd
```

Dataflow:

<picture>
  <img src="https://static.rerun.io/rerun_native_async/272c9ba7e7afe0ee5491ff1aabc76965588c513f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun_native_async/272c9ba7e7afe0ee5491ff1aabc76965588c513f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun_native_async/272c9ba7e7afe0ee5491ff1aabc76965588c513f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun_native_async/272c9ba7e7afe0ee5491ff1aabc76965588c513f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun_native_async/272c9ba7e7afe0ee5491ff1aabc76965588c513f/1200w.png">
</picture>


Reference:
* [SDK operating modes: `save`](../reference/sdk/operating-modes.md#save)
* [🐍 Python `save`](https://ref.rerun.io/docs/python/0.19.0/common/initialization_functions/#rerun.save)
* [🦀 Rust `save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.save)
* [🌊 C++ `save`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a555a7940a076c93d951de5b139d14918)

<!--
Logging data on native and visualizing it on the web.

TODO(#8046): incoming.
-->


## FAQ

### How can I use multiple **Native Viewers** at the same (i.e. multiple windows)?

Every **Native Viewer** comes with a corresponding **gRPC server** -- always. You cannot start a **Native Viewer** without starting a **gRPC server**.

The only way to have more than one Rerun window is to have more than one **gRPC server**, by means of the `--port` flag.

E.g.:
```sh
# starts a new viewer, listening for gRPC connections on :9876
rerun &

# does nothing, there's already a viewer session running at that address
rerun &

# does nothing, there's already a viewer session running at that address
rerun --port 9876 &

# logs the image file to the existing viewer running on :9876
rerun image.jpg

# logs the image file to the existing viewer running on :9876
rerun --port 9876 image.jpg

# starts a new viewer, listening for gRPC connections on :6789, and logs the image data to it
rerun --port 6789 image.jpg

# does nothing, there's already a viewer session running at that address
rerun --port 6789 &

# logs the image file to the existing viewer running on :6789
rerun --port 6789 image.jpg &
```

<!--

(these are headings, not marked as such since it confuses svelte's link checking)

What happens when I use `rr.spawn()` from my SDK of choice?

TODO(#8046): incoming.


What happens when I use `rr.serve()` from my SDK of choice?

TODO(#8046): incoming.


What happens when I use `rerun --serve`?

TODO(#8046): incoming.

-->
