from __future__ import annotations

import logging

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun.recording import MemoryRecording
from rerun.recording_stream import RecordingStream

# --- Sinks ---


def connect(
    addr: str | None = None, flush_timeout_sec: float | None = 2.0, recording: RecordingStream | None = None
) -> None:
    """
    Connect to a remote Rerun Viewer on the given ip:port.

    Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.

    This function returns immediately.

    Parameters
    ----------
    addr
        The ip:port to connect to
    flush_timeout_sec: float
        The minimum time the SDK will wait during a flush before potentially
        dropping data if progress is not being made. Passing `None` indicates no timeout,
        and can cause a call to `flush` to block indefinitely.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)
    bindings.connect(addr=addr, flush_timeout_sec=flush_timeout_sec, recording=recording)


_connect = connect  # we need this because Python scoping is horrible


def save(path: str, recording: RecordingStream | None = None) -> None:
    """
    Stream all log-data to a file.

    Parameters
    ----------
    path : str
        The path to save the data to.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - save() call ignored. You must call rerun.init before saving a recording.")
        return

    recording = RecordingStream.to_native(recording)
    bindings.save(path=path, recording=recording)


def disconnect(recording: RecordingStream | None = None) -> None:
    """
    Closes all TCP connections, servers, and files.

    Closes all TCP connections, servers, and files that have been opened with
    [`rerun.connect`], [`rerun.serve`], [`rerun.save`] or [`rerun.spawn`].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)
    bindings.disconnect(recording=recording)


def memory_recording(recording: RecordingStream | None = None) -> MemoryRecording:
    """
    Streams all log-data to a memory buffer.

    This can be used to display the RRD to alternative formats such as html.
    See: [rerun.MemoryRecording.as_html][].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    MemoryRecording
        A memory recording object that can be used to read the data.
    """

    recording = RecordingStream.to_native(recording)
    return MemoryRecording(bindings.memory_recording(recording=recording))


def serve(
    open_browser: bool = True,
    web_port: int | None = None,
    ws_port: int | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.

    You can also connect to this server with the native viewer using `rerun localhost:9090`.

    WARNING: This is an experimental feature.

    This function returns immediately.

    Parameters
    ----------
    open_browser
        Open the default browser to the viewer.
    web_port:
        The port to serve the web viewer on (defaults to 9090).
    ws_port:
        The port to serve the WebSocket server on (defaults to 9877)
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    """

    recording = RecordingStream.to_native(recording)
    bindings.serve(open_browser, web_port, ws_port, recording=recording)


def spawn(port: int = 9876, connect: bool = True, recording: RecordingStream | None = None) -> None:
    """
    Spawn a Rerun Viewer, listening on the given port.

    This is often the easiest and best way to use Rerun.
    Just call this once at the start of your program.

    You can also call [rerun.init][] with a `spawn=True` argument.

    Parameters
    ----------
    port : int
        The port to listen on.
    connect
        also connect to the viewer and stream logging data to it.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use if `connect = True`.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    import os
    import subprocess
    import sys
    from time import sleep

    # Let the spawned rerun process know it's just an app
    new_env = os.environ.copy()
    new_env["RERUN_APP_ONLY"] = "true"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    # start_new_session=True ensures the spawned process does NOT die when
    # we hit ctrl-c in the terminal running the parent Python process.
    subprocess.Popen([python_executable, "-m", "rerun", "--port", str(port)], env=new_env, start_new_session=True)

    # TODO(emilk): figure out a way to postpone connecting until the rerun viewer is listening.
    # For example, wait until it prints "Hosting a SDK server over TCP at …"
    sleep(0.5)  # almost as good as waiting the correct amount of time

    if connect:
        _connect(f"127.0.0.1:{port}", recording=recording)
