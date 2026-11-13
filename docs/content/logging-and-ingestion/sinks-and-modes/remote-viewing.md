---
title: View data from a remote or headless machine
order: 250
description: Stream data from a remote server, container, or SSH session to your Viewer
---

When your code runs on a machine without a display (a cloud VM, a robot, a Docker container, or a server you reach over SSH), run the Viewer on your own machine and send the data to it over gRPC.
All SDK-to-Viewer traffic uses one TCP port, `9876` by default.

There are three common setups:

| Setup                                                                       | Remote machine runs | Your machine runs   |
| --------------------------------------------------------------------------- | ------------------- | ------------------- |
| [Serve from the remote machine](#serve-from-the-remote-machine)             | `rr.serve_grpc()`   | `rerun --connect …` |
| [Connect to a Viewer on your machine](#connect-to-a-viewer-on-your-machine) | `rr.connect_grpc()` | `rerun`             |
| [Use a web Viewer](#use-a-web-viewer)                                       | `rerun --serve-web` | A web browser       |

See [SDK operating modes](operating-modes.md) for the full list of ways to send data, and [Log and Ingest](../tutorial.md) for the basics.

## Serve from the remote machine

Call `serve_grpc` in your logging code.
It starts a gRPC server inside your process that listens on `0.0.0.0:9876` and buffers the logged data, so a Viewer can connect at any time and still get the data that was logged before it connected.

```python
import rerun as rr

rr.init("rerun_example_remote")
rr.serve_grpc()  # Listens on 0.0.0.0:9876

# … log data as usual …
```

The server stops when your process exits, so keep the process running while you view the data.

On your machine, forward the port over SSH and connect the Viewer to the forwarded port:

```sh
ssh -L 9876:localhost:9876 user@remote-host
```

```sh
rerun --connect rerun+http://localhost:9876/proxy
```

Use `--connect` here, and not a plain `rerun rerun+http://…` argument.
Without `--connect`, the Viewer tries to start its own server on port `9876`, finds the SSH tunnel listening there, and assumes that another Viewer is already running.

If the remote machine is reachable on your network, you can skip the tunnel and connect directly: `rerun --connect rerun+http://remote-host:9876/proxy`.

Instead of `serve_grpc`, you can also run `rerun --serve-grpc` on the remote machine as a standalone server, and let your logging code send to it with `rr.connect_grpc()`.
The data then stays available after your logging process exits.

## Connect to a Viewer on your machine

Start the Viewer on your own machine:

```sh
rerun
```

It listens on `0.0.0.0:9876` by default.
Then open a reverse tunnel, so that port `9876` on the remote machine leads to your Viewer:

```sh
ssh -R 9876:localhost:9876 user@remote-host
```

On the remote machine, connect to it.
The default URL is `rerun+http://127.0.0.1:9876/proxy`, which goes through the tunnel:

```python
import rerun as rr

rr.init("rerun_example_remote")
rr.connect_grpc()  # Same as rr.connect_grpc("rerun+http://127.0.0.1:9876/proxy")
```

With this setup, the Viewer must be running before your code starts logging.

## Use a web Viewer

If you cannot install the Viewer on your machine, the remote machine can host a web Viewer:

```sh
rerun --serve-web
```

This starts a gRPC server on port `9876` (`--port`) and serves the web Viewer over HTTP on port `9090` (`--web-viewer-port`).
Your logging code sends data to it with `rr.connect_grpc()`.
The command prints the URL to open, which looks like `http://127.0.0.1:9090?url=rerun%2Bhttp%3A%2F%2Flocalhost%3A9876%2Fproxy`.

The `url` parameter in that page URL tells the web Viewer, which runs in your browser, where to find the gRPC server.
The simplest way to make both ports reachable is to forward both:

```sh
ssh -L 9090:localhost:9090 -L 9876:localhost:9876 user@remote-host
```

Then open the printed URL in your browser.

If you open the web Viewer by the remote machine's own address instead (for example `http://remote-host:9090`), two things change:
- The `url` parameter must point at the remote machine: `http://remote-host:9090?url=rerun%2Bhttp%3A%2F%2Fremote-host%3A9876%2Fproxy`.
- By default, the gRPC server only accepts browser requests from pages on `localhost`, `127.0.0.1`, and `rerun.io` (and its subdomains).
  Allow the page's origin with `--cors-allow-origin`, for example `rerun --serve-web --cors-allow-origin "http://remote-host:9090"`.

The web Viewer runs as WebAssembly in your browser, so it has less memory available, and is slower, than the native Viewer.

## Docker

Inside a container, both `serve_grpc` and the `rerun` CLI listen on all interfaces (`0.0.0.0`) by default, so you only need to publish the ports:

```sh
docker run -p 9876:9876 my-image            # for serve_grpc or rerun --serve-grpc
docker run -p 9876:9876 -p 9090:9090 my-image  # for rerun --serve-web
```

Then connect from the host as described above, without the SSH tunnel.

To send data from a container to a Viewer that runs on the host, connect to the host's address instead of `127.0.0.1`.
With Docker Desktop, that is `rr.connect_grpc("rerun+http://host.docker.internal:9876/proxy")`.
On Linux, add `--add-host=host.docker.internal:host-gateway` to `docker run` for the same name to work.

The `rerun` CLI uses `--bind` to choose the listening address (default `0.0.0.0`).
`rr.server.Server`, the catalog server, uses its `host` argument (default `0.0.0.0`).

## Behind TLS or a reverse proxy

Use the `rerun+https://` scheme when the server is behind TLS, for example `rr.connect_grpc("rerun+https://rerun.example.com/proxy")` or `rerun --connect rerun+https://rerun.example.com/proxy`.

The proxy must forward gRPC traffic.
Native Viewers and SDKs use gRPC over HTTP/2.
Web Viewers use gRPC-Web over HTTP, which the server also accepts.

If a web Viewer is hosted on an origin other than `localhost`, `127.0.0.1`, or `rerun.io` and its subdomains, allow that origin on the gRPC server:

```sh
rerun --serve-grpc --cors-allow-origin "https://viewer.example.com"
```

```python
rr.serve_grpc(cors_allow_origin=["https://viewer.example.com"])
```

Patterns can use `*` as a wildcard, for example `"https://*.example.com"`.

## Slow or unreliable links

The amount of data you log decides how fast it gets to the Viewer.
To send less:
- Compress images before logging, with [`EncodedImage`](../../reference/types/archetypes/encoded_image.md) or `rr.Image(…).compress(jpeg_quality=…)` in Python.
- Log camera streams as video, with [`VideoStream`](../../reference/types/archetypes/video_stream.md) or [`AssetVideo`](../../reference/types/archetypes/asset_video.md).
  See [Video](../media/video.md).
- Log only the entities you need to look at remotely.
  Log the rest to a second `RecordingStream` that saves to a file.
  Calling `rr.save()` on the streaming recording replaces its network sink instead of adding a file.

A gRPC server buffers logged data in memory, so that late-connecting Viewers get the full history.
When the buffer is full, it drops the oldest temporal data first.
If that is not enough, it also drops the oldest static data.
The default limit is 1 GiB.
Set it with `rr.serve_grpc(server_memory_limit="4GB")` or `rerun --serve-grpc --server-memory-limit 4GB`.
To let a Viewer that connects late see the newest data first, use `newest_first=True` or `--newest-first`.

The Viewer has its own memory limit; see [Limit the viewer's memory usage](../../viewer/performance/limit-ram.md).
