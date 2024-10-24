---
title: The Rerun application model
order: 750
---

The Rerun distribution comes with numerous moving pieces:
* The **SDKs** (Python, Rust & C++), for logging data and querying it back.
* The **Chunk Store**: the in-memory database that stores the logged data.
* The **TCP server**, which accepts connections from the SDKs and inserts the incoming data into the Chunk Store.
* The **Native Viewer**, which visualizes the contents of the Chunk Store on native platforms (Linux, macOS, Windows).
* The **Web Viewer**, a WASM application for visualizing the contents of the Chunk Store on the Web and its derivatives (notebooks, etc).
* The **Web/HTTP Server**, for serving the web page that hosts the Web Viewer.
* The **WebSocket server**, for serving data to the Web Viewer.
* The **CLI**, which allows you to control all the pieces above and manipulate RRD files.

This is a lot to take in at first, but as we'll see these different pieces are generally deployed in just a few unique configurations in most common cases.

The first clear distinction we can make is client-side vs. server-side: the SDKs are the Rerun clients, everything else is part of the Rerun server in one way or another.
In fact, with the exception of the SDKs (which are vanilla software libraries) and the Web Viewer (which is a WASM artifact), every single one of these pieces live in the same binary: `rerun`.
The `rerun` binary is the CLI, it's the Native Viewer, it's the TCP server, the Web/HTTP server, even the WebSocket server.

The best way to make sense of it all it to look at some of the most common scenarios when:
* Logging and visualizing data on native.
* Logging data on native and visualizing it on the web.


## Logging and visualizing data on native

TODO: well, all pieces always spawn, really

There are two common scenarios when working natively:
* Data is being logged and visualized at the same time (synchronous workflow).
* Data is being logged first to some persistent storage, and visualized at a later time (asynchronous workflow).

We'll take a closer look at both.


### Synchronous workflow

Situation: rr.connect() + `rerun --port 9876`

Logging script:
TODO: snippets
```python
# Connect to the Rerun TCP server using the default address and port: localhost:9876
rr.connect()

while True:
  # Log data as usual, thereby pushing it into the TCP socket.
  log_things()
```

```sh
# Start the Rerun Native Viewer in the background.
#
# This will also start the TCP server on its default port (9876, use `--port` to pick another one).
#
# TODO: nuh-uh
# This will spawn all the Rerun servers on their default ports:
# * TCP: 9876
# * Web/HTTP: 9090
# * WebSocket: 9877
#
# We could also have just used `spawn()` instead of `connect()` in the logging script.
# `spawn()` does exactly this: it fork-execs a Native Viewer using the first `rerun` binary available on your $PATH.
$ rerun &

$ ./logging_script
```


TODO: talk about https://github.com/rerun-io/rerun/issues/7768


### Asynchronous workflow

Situation: rr.save("/tmp/file.rrd") + `rerun /tmp/file.rrd`

TODO: rr.save()


## Logging data on native and visualizing it on the web.

TODO: either you serve

TODO: we gotta explain how the URL works...

TODO: is the WebSocket push or pull?

When you run `rerun --serve`:
```
http://localhost:9090/?url=ws://localhost:9877
^^^^^^^^^^^^^^^^^^^^^      ^^^^^^^^^^^^^^^^^^^
      |                         |
      |                         +> The Rerun WebSocket server that serves the actual data
      |
      +> The Rerun Web/HTTP server that serves the web page for the Rerun Web Viewer (i.e. some html, css, js and wasm)
```


There are two common situations when working on the Web:
* Data is being logged and visualized at the same time (synchronous workflow).
* Data is being logged first to some persistent storage, and visualized at a later time (asynchronous workflow).


### Synchronous workflow

TODO: rr.connect(), rr.spawn()


### Asynchronous workflow

TODO: rr.save()


## FAQ

### What is the relationship between the Native Viewer and the different servers?



* Every Rerun viewer session now corresponds to a Rerun TCP server.
* You cannot start the Rerun viewer without starting a TCP server.
* The only way to have more than one Rerun window is to have more than one TCP server, which means using the `--port` flag.

```
# starts a new viewer, listening for TCP connections on :9876
rerun &

# does nothing, there's already a viewer session running at that address
rerun &

# does nothing, there's already a viewer session running at that address
rerun --port 9876 &

# logs the image file to the existing viewer running on :9876
rerun image.jpg

# logs the image file to the existing viewer running on :9876
rerun --port 9876 image.jpg

# starts a new viewer, listening for TCP connections on :6789, and logs the image data to it
rerun --port 6789 image.jpg

# does nothing, there's already a viewer session running at that address
rerun --port 6789 &

# logs the image file to the existing viewer running on :6789
rerun --port 6789 image.jpg &
```

TODO: every native window == one TCP server


### What happens when I use `rr.spawn()` from my SDK of choice?

TODO: link to rr.spawn for all languages


### What happens when I use `rr.serve()` from my SDK of choice?

TODO: link to rr.serve for all languages

### What happens when I use `rerun --serve`?

