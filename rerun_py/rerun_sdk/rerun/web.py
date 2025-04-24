from __future__ import annotations

import rerun_bindings as bindings


def serve_web_viewer(*, web_port: int | None = None, open_browser: bool = True, connect_to: str | None = None) -> None:
    """
    Host a web viewer over HTTP.

    You can pass this function the URL returned from [`rerun.serve_grpc`][] and  [`rerun.RecordingStream.serve_grpc`][]
    so that the spawned web viewer connects to that server.

    Note that this is NOT a log sink, and this does NOT host a gRPC server.

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
    )
