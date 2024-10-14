from __future__ import annotations

import logging
import pathlib

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun.blueprint.api import BlueprintLike, create_in_memory_blueprint
from rerun.recording_stream import RecordingStream, get_application_id

from ._spawn import _spawn_viewer

# --- Sinks ---


def is_recording_enabled(recording: RecordingStream | None) -> bool:
    if recording is not None:
        return bindings.is_enabled(recording.inner)  # type: ignore[no-any-return]
    return bindings.is_enabled()  # type: ignore[no-any-return]


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

    if not is_recording_enabled(recording):
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

    The Rerun Viewer is able to read continuously from the resulting rrd file while it is being written.
    However, depending on your OS and configuration, changes may not be immediately visible due to file caching.
    This is a common issue on Windows and (to a lesser extent) on MacOS.

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

    if not is_recording_enabled(recording):
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

    if not is_recording_enabled(recording):
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
    Note that this means that static data may be dropped if logged early (see <https://github.com/rerun-io/rerun/issues/5531>).

    This function returns immediately.

    Parameters
    ----------
    open_browser:
        Open the default browser to the viewer.
    web_port:
        The port to serve the web viewer on (defaults to 9090).
    ws_port:
        The port to serve the WebSocket server on (defaults to 9877)
    default_blueprint:
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

    if not is_recording_enabled(recording):
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
    # TODO(#5531): keep static data around.
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


def spawn(
    *,
    port: int = 9876,
    connect: bool = True,
    memory_limit: str = "75%",
    hide_welcome_screen: bool = False,
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
    hide_welcome_screen:
        Hide the normal Rerun welcome screen.
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

    if not is_recording_enabled(recording):
        logging.warning("Rerun is disabled - spawn() call ignored.")
        return

    _spawn_viewer(port=port, memory_limit=memory_limit, hide_welcome_screen=hide_welcome_screen)

    if connect:
        _connect(f"127.0.0.1:{port}", recording=recording, default_blueprint=default_blueprint)
