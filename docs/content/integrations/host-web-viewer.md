---
title: Share recordings with many users through the web Viewer
order: 125
description: Give many users their own recordings in the web Viewer, with links from dashboards and experiment trackers
---

To let many users look at different recordings in a browser, give each of them a web Viewer link to their recording.
The link names the data in its `url` parameter, so you only need to host the data, not one Viewer per user.

## A gRPC proxy shows the same data to everyone

`rerun --serve-web`, `rerun --serve-grpc`, and `rr.serve_grpc()` start a gRPC proxy.
The proxy sends every message it receives to every connected Viewer.
It has no per-user sessions: all users see the same stream.

The proxy also keeps a buffer of past messages, so that a Viewer that connects late gets the history.
The buffer is limited by `--server-memory-limit` (or `server_memory_limit` in `serve_grpc`), 1 GiB by default, and the oldest data is dropped when it is full.

This is a good fit for a live demo or a shared screen, but not for giving each user their own data.

## Give each user a link to a file

Save each recording as an `.rrd` file (for example with `rr.save()`), and host the files on any HTTP server or object store.
Then give each user a link with the file's URL in the `url` parameter:

```
https://rerun.io/viewer?url=https%3A%2F%2Fdata.example.com%2Fruns%2F42.rrd
```

See [Open a file by URL](../viewer/extend/urls.md#open-a-file-by-url) for the details: URL-encoding, presigned URLs, and the CORS headers that the file host must send.

Experiment trackers such as MLflow or Weights & Biases need nothing special: store the Viewer link with the run, for example as a link or an HTML artifact.

## Give each user a link to a catalog server

If you have many recordings, or want users to browse and query them, put them on a catalog server and link to each recording (segment) there.
The open-source [`rerun server`](../query-and-transform/catalog/oss-server.md) serves a directory of `.rrd` files; [Rerun Hub](../hub.md) is the managed version.

[Generate segment URLs in dataframes](../query-and-transform/dataframes/segment_url.md) shows how to create a link per segment.
Put the segment URL, URL-encoded, in the `url` parameter of the web Viewer page.

The server only accepts browser requests from pages on `localhost`, `127.0.0.1`, and `rerun.io` (and its subdomains) by default.
If you serve the Viewer from your own domain, allow it with `rerun server --cors-allow-origin "https://viewer.example.com"`.

## Where to host the Viewer

- Use the hosted Viewer at `https://rerun.io/viewer`.
- Embed it in your own page with an `<iframe>` or the JavaScript package.
  See [Embed Rerun in Web pages](embed-web.md).
- Serve it yourself with `rerun --serve-web`, which hosts the web Viewer on port `9090` (`--web-viewer-port`).

Use a Viewer version that can read your files.
See [RRD file backwards compatibility](../logging-and-ingestion/tutorial.md#rrd-file-backwards-compatibility).
