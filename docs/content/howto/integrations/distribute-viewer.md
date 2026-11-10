---
title: Share recordings with people who don't code
order: 150
description: Ship the Rerun Viewer to non-programmers, share recordings as a link, or bundle the Viewer with your Python app
---

You don't need a Python or Rust setup to look at Rerun data.
There are three ways to give someone the Viewer.

Whichever you pick, use a Viewer with the same version as the SDK that wrote the data.
An older Viewer may not be able to read data from a newer SDK.

## Share a link to the web Viewer

If you can host the `.rrd` file on a web server, people can open it in their browser without installing anything.
Pass the URL of the file as the `url` parameter to the web Viewer of the matching version:

```
https://app.rerun.io/version/<VERSION>/?url=https://example.com/my_recording.rrd
```

For example, `https://app.rerun.io/version/0.41.0/?url=…` for data written with Rerun 0.41.0.
`https://rerun.io/viewer?url=…` always uses the latest version.

The web server must allow cross-origin requests (CORS) for the file, because the browser loads it from another domain.
See [Viewer URLs](../../reference/viewer/urls.md#in-the-web-viewer) and [Embed Rerun in web pages](embed-web.md).

## Send them the Viewer binary

The Viewer is a single, self-contained executable called `rerun`.
Download it for their platform from the [GitHub release assets](https://github.com/rerun-io/rerun/releases/latest/):

| Platform              | Asset                                                                                                     |
| --------------------- | --------------------------------------------------------------------------------------------------------- |
| Linux x86_64          | `rerun-cli-<VERSION>-x86_64-unknown-linux-gnu`                                                            |
| Linux arm64           | `rerun-cli-<VERSION>-aarch64-unknown-linux-gnu`                                                           |
| macOS (Apple silicon) | `Rerun-<VERSION>-aarch64-apple-darwin.app.tar.gz`, or the bare `rerun-cli-<VERSION>-aarch64-apple-darwin` |
| Windows x86_64        | `rerun-cli-<VERSION>-x86_64-pc-windows-msvc.exe`                                                          |

They can then open a recording from the command line:

```sh
rerun my_recording.rrd
```

or start the Viewer and drag-and-drop the `.rrd` file onto it, or use **Open file…** in the menu.

## Bundle the Viewer with a Python app

The `rerun-sdk` wheel contains the Viewer binary in its `rerun_cli` package.
`rr.spawn()` looks for `rerun` on your `PATH`, which usually doesn't exist when you package your app with a tool like PyInstaller.
Copy the binary into your bundle, and pass its path to `rr.spawn`:

```python
rr.spawn(executable_path="/path/to/bundled/rerun")
```

To find the binary in your Python environment:

```sh
python -c "import os, rerun_cli; print(os.path.dirname(rerun_cli.__file__))"
```

On macOS this folder contains `Rerun.app`, with the binary at `Rerun.app/Contents/MacOS/Rerun`, or a bare `rerun` binary.
On Linux and Windows it contains `rerun` or `rerun.exe`.
