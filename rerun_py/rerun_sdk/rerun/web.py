from __future__ import annotations

from pathlib import Path

import rerun_bindings as bindings


def _packaged_assets_archive_path() -> str | None:
    """
    Path to the web viewer assets archive shipped in the wheel, or `None` if absent.

    Some wheels are built without web viewer assets embedded in the extension module.
    They instead ship the assets as `rerun_sdk/web_viewer.zip`,
    which is expected to be served from disk.
    """
    path = Path(__file__).parent.parent / "web_viewer.zip"
    return str(path) if path.is_file() else None


def serve_web_viewer(*, web_port: int | None = None, open_browser: bool = True, connect_to: str | None = None) -> None:
    """
    Host a web viewer over HTTP.

    You can pass this function the URL returned from [`rerun.serve_grpc`][] and  [`rerun.RecordingStream.serve_grpc`][]
    so that the spawned web viewer connects to that server.

    Note that this is NOT a log sink, and this does NOT host a gRPC server.
    If you want to log data to a gRPC server and connect the web viewer to it, you can do so like this:
    ```
    server_uri = rr.serve_grpc()
    rr.serve_web_viewer(connect_to=server_uri)
    ```

    This function returns immediately.
    In order to keep the web server running you must keep the Python process running too.

    Parameters
    ----------
    web_port:
        The port to serve the web viewer on (defaults to 9090).
    open_browser:
        Open the default browser to the viewer.
    connect_to:
        If `open_browser` is true, then this is the URL the web viewer will connect to.

    """

    bindings.serve_web_viewer(
        web_port=web_port,
        open_browser=open_browser,
        connect_to=connect_to,
        assets_archive_path=_packaged_assets_archive_path(),
    )
