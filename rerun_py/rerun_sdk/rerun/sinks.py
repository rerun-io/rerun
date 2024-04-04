from __future__ import annotations

import logging
import pathlib
import socket

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun.blueprint.api import BlueprintLike, create_in_memory_blueprint
from rerun.recording_stream import RecordingStream, get_application_id

# --- Sinks ---


def connect(
    addr: str | None = None,
    *,
    flush_timeout_sec: float | None = 2.0,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Connect to a remote Rerun Viewer on the given ip:port.

    Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.

    This function returns immediately.

    Parameters
    ----------
    addr:
        The ip:port to connect to
    flush_timeout_sec:
        The minimum time the SDK will wait during a flush before potentially
        dropping data if progress is not being made. Passing `None` indicates no timeout,
        and can cause a call to `flush` to block indefinitely.
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - connect() call ignored")
        return

    application_id = get_application_id(recording=recording)
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording."
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id, blueprint=default_blueprint
        ).storage

    recording = RecordingStream.to_native(recording)

    bindings.connect(
        addr=addr, flush_timeout_sec=flush_timeout_sec, default_blueprint=blueprint_storage, recording=recording
    )


_connect = connect  # we need this because Python scoping is horrible


def save(
    path: str | pathlib.Path, default_blueprint: BlueprintLike | None = None, recording: RecordingStream | None = None
) -> None:
    """
    Stream all log-data to a file.

    Call this _before_ you log any data!

    Parameters
    ----------
    path:
        The path to save the data to.
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - save() call ignored. You must call rerun.init before saving a recording.")
        return

    application_id = get_application_id(recording=recording)
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording."
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id, blueprint=default_blueprint
        ).storage

    recording = RecordingStream.to_native(recording)

    bindings.save(path=str(path), default_blueprint=blueprint_storage, recording=recording)


def stdout(default_blueprint: BlueprintLike | None = None, recording: RecordingStream | None = None) -> None:
    """
    Stream all log-data to stdout.

    Pipe it into a Rerun Viewer to visualize it.

    Call this _before_ you log any data!

    If there isn't any listener at the other end of the pipe, the `RecordingStream` will
    default back to `buffered` mode, in order not to break the user's terminal.

    Parameters
    ----------
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - save() call ignored. You must call rerun.init before saving a recording.")
        return

    application_id = get_application_id(recording=recording)
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording."
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id, blueprint=default_blueprint
        ).storage

    recording = RecordingStream.to_native(recording)
    bindings.stdout(default_blueprint=blueprint_storage, recording=recording)


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


def serve(
    *,
    open_browser: bool = True,
    web_port: int | None = None,
    ws_port: int | None = None,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
    server_memory_limit: str = "25%",
) -> None:
    """
    Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.

    You can also connect to this server with the native viewer using `rerun localhost:9090`.

    The WebSocket server will buffer all log data in memory so that late connecting viewers will get all the data.
    You can limit the amount of data buffered by the WebSocket server with the `server_memory_limit` argument.
    Once reached, the earliest logged data will be dropped.
    Note that this means that timeless data may be dropped if logged early.

    This function returns immediately.

    Parameters
    ----------
    open_browser:
        Open the default browser to the viewer.
    web_port:
        The port to serve the web viewer on (defaults to 9090).
    ws_port:
        The port to serve the WebSocket server on (defaults to 9877)
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    server_memory_limit:
        Maximum amount of memory to use for buffering log data for clients that connect late.
        This can be a percentage of the total ram (e.g. "50%") or an absolute value (e.g. "4GB").

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - serve() call ignored")
        return

    application_id = get_application_id(recording=recording)
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording."
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id, blueprint=default_blueprint
        ).storage

    recording = RecordingStream.to_native(recording)
    bindings.serve(
        open_browser,
        web_port,
        ws_port,
        server_memory_limit=server_memory_limit,
        default_blueprint=blueprint_storage,
        recording=recording,
    )


def send_blueprint(
    blueprint: BlueprintLike,
    *,
    make_active: bool = True,
    make_default: bool = True,
    recording: RecordingStream | None = None,
) -> None:
    """
    Create a blueprint from a `BlueprintLike` and send it to the `RecordingStream`.

    Parameters
    ----------
    blueprint:
        A blueprint object to send to the viewer.
    make_active:
        Immediately make this the active blueprint for the associated `app_id`.
        Note that setting this to `false` does not mean the blueprint may not still end
        up becoming active. In particular, if `make_default` is true and there is no other
        currently active blueprint.
    make_default:
        Make this the default blueprint for the `app_id`.
        The default blueprint will be used as the template when the user resets the
        blueprint for the app. It will also become the active blueprint if no other
        blueprint is currently active.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    application_id = get_application_id(recording=recording)

    if application_id is None:
        raise ValueError("No application id found. You must call rerun.init before sending a blueprint.")

    recording = RecordingStream.to_native(recording)

    blueprint_storage = create_in_memory_blueprint(application_id=application_id, blueprint=blueprint).storage

    bindings.send_blueprint(blueprint_storage, make_active, make_default, recording=recording)


# TODO(#4019): application-level handshake
def _check_for_existing_viewer(port: int) -> bool:
    try:
        # Try opening a connection to the port to see if something is there
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(1)
        s.connect(("127.0.0.1", port))
        return True
    except Exception:
        # If the connection times out or is refused, the port is not open
        return False
    finally:
        # Always close the socket to release resources
        s.close()


def spawn(
    *,
    port: int = 9876,
    connect: bool = True,
    memory_limit: str = "75%",
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Spawn a Rerun Viewer, listening on the given port.

    This is often the easiest and best way to use Rerun.
    Just call this once at the start of your program.

    You can also call [rerun.init][] with a `spawn=True` argument.

    Parameters
    ----------
    port:
        The port to listen on.
    connect:
        also connect to the viewer and stream logging data to it.
    memory_limit:
        An upper limit on how much memory the Rerun Viewer should use.
        When this limit is reached, Rerun will drop the oldest data.
        Example: `16GB` or `50%` (of system total).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use if `connect = True`.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.

    """

    if not bindings.is_enabled():
        logging.warning("Rerun is disabled - spawn() call ignored.")
        return

    import os
    import subprocess
    import sys
    from time import sleep

    # Let the spawned rerun process know it's just an app
    new_env = os.environ.copy()
    # NOTE: If `_RERUN_TEST_FORCE_SAVE` is set, all recording streams will write to disk no matter
    # what, thus spawning a viewer is pointless (and probably not intended).
    if os.environ.get("_RERUN_TEST_FORCE_SAVE") is not None:
        return
    new_env["RERUN_APP_ONLY"] = "true"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    # TODO(jleibs): More options to opt out of this behavior.
    if _check_for_existing_viewer(port):
        # Using print here for now rather than `logging.info` because logging.info isn't
        # visible by default.
        #
        # If we spawn a process it's going to send a bunch of stuff to stdout anyways.
        print(f"Found existing process on port {port}. Trying to connect.")
    else:
        # start_new_session=True ensures the spawned process does NOT die when
        # we hit ctrl-c in the terminal running the parent Python process.
        subprocess.Popen(
            [
                python_executable,
                "-c",
                "import rerun_bindings; rerun_bindings.main()",
                f"--port={port}",
                f"--memory-limit={memory_limit}",
                "--skip-welcome-screen",
            ],
            env=new_env,
            start_new_session=True,
        )

        # Give the newly spawned Rerun Viewer some time to bind.
        #
        # NOTE: The timeout only covers the TCP handshake: if no process is bound to that address
        # at all, the connection will fail immediately, irrelevant of the timeout configuration.
        # For that reason we use an extra loop.
        for _ in range(0, 5):
            _check_for_existing_viewer(port)
            sleep(0.1)

    if connect:
        _connect(f"127.0.0.1:{port}", recording=recording, default_blueprint=default_blueprint)
