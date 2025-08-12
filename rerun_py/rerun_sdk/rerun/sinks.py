from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Union

import rerun_bindings as bindings
from rerun_bindings import (
    FileSink as FileSink,
    GrpcSink as GrpcSink,
)
from typing_extensions import deprecated

from rerun.blueprint.api import BlueprintLike, create_in_memory_blueprint
from rerun.recording_stream import RecordingStream, get_application_id

from ._spawn import _spawn_viewer

if TYPE_CHECKING:
    import pathlib

    from rerun.dataframe import Recording
    from rerun.recording_stream import RecordingStream


# --- Sinks ---


def is_recording_enabled(recording: RecordingStream | None) -> bool:
    if recording is not None:
        return bindings.is_enabled(recording.inner)  # type: ignore[no-any-return]
    return bindings.is_enabled()  # type: ignore[no-any-return]


LogSinkLike = Union[GrpcSink, FileSink]


def set_sinks(
    *sinks: LogSinkLike,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Stream data to multiple different sinks.

    Duplicate sinks are not allowed. For example, two [`rerun.GrpcSink`][]s that
    use the same `url` will cause this function to throw a `ValueError`.

    This _replaces_ existing sinks. Calling `rr.init(spawn=True)`, `rr.spawn()`,
    `rr.connect_grpc()` or similar followed by `set_sinks` will result in only
    the sinks passed to `set_sinks` remaining active.

    Only data logged _after_ the `set_sinks` call will be logged to the newly attached sinks.

    Parameters
    ----------
    sinks:
        A list of sinks to wrap.

        See [`rerun.GrpcSink`][], [`rerun.FileSink`][].
    default_blueprint:
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Example
    -------
    ```py
    rr.init("rerun_example_tee")
    rr.set_sinks(
        rr.GrpcSink(),
        rr.FileSink("data.rrd")
    )
    rr.log("my/point", rr.Points3D(position=[1.0, 2.0, 3.0]))
    ```

    """

    # Check for duplicates
    seen = set()
    duplicates = set()
    for sink in sinks:
        if sink in seen:
            duplicates.add(sink)
        else:
            seen.add(sink)
    if duplicates:
        raise ValueError(f"Duplicate sinks detected: {', '.join(str(d) for d in duplicates)}")

    if not is_recording_enabled(recording):
        logging.warning("Rerun is disabled - set_sinks() call ignored")
        return

    application_id = get_application_id(recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    bindings.set_sinks(
        [*sinks],
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
    )


def connect_grpc(
    url: str | None = None,
    *,
    flush_timeout_sec: float | None = 2.0,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Connect to a remote Rerun Viewer on the given URL.

    This function returns immediately.

    Parameters
    ----------
    url:
        The URL to connect to.

        The scheme must be one of `rerun://`, `rerun+http://`, or `rerun+https://`,
        and the pathname must be `/proxy`.

        The default is `rerun+http://127.0.0.1:9876/proxy`.
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
        logging.warning("Rerun is disabled - connect_grpc() call ignored")
        return

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    bindings.connect_grpc(
        url=url,
        flush_timeout_sec=flush_timeout_sec,
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
    )


def save(
    path: str | pathlib.Path,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
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

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording=recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    bindings.save(
        path=str(path),
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
    )


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

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording=recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    bindings.stdout(
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
    )


def disconnect(recording: RecordingStream | None = None) -> None:
    """
    Closes all gRPC connections, servers, and files.

    Closes all gRPC connections, servers, and files that have been opened with
    [`rerun.connect_grpc`], [`rerun.serve`], [`rerun.save`] or [`rerun.spawn`].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.disconnect(
        recording=recording.to_native() if recording is not None else None,
    )


def serve_grpc(
    *,
    grpc_port: int | None = None,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
    server_memory_limit: str = "25%",
) -> str:
    """
    Serve log-data over gRPC.

    You can connect to this server with the native viewer using `rerun rerun+http://localhost:{grpc_port}/proxy`.

    The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
    You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
    Once reached, the earliest logged data will be dropped. Static data is never dropped.

    It is highly recommended that you set the memory limit to `0B` if both the server and client are running
    on the same machine, otherwise you're potentially doubling your memory usage!

    Returns the URI of the server so you can connect the viewer to it.

    This function returns immediately. In order to keep the server running, you must keep the Python process running
    as well.

    Parameters
    ----------
    grpc_port:
        The port to serve the gRPC server on (defaults to 9876)
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
        logging.warning("Rerun is disabled - serve_grpc() call ignored")
        return "[rerun is disabled]"

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording=recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    return bindings.serve_grpc(
        grpc_port,
        server_memory_limit=server_memory_limit,
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
    )


@deprecated(
    """Use a combination of `rr.serve_grpc` and `rr.serve_web_viewer` instead.
    See: https://www.rerun.io/docs/reference/migration/migration-0-24 for more details.""",
)
def serve_web(
    *,
    open_browser: bool = True,
    web_port: int | None = None,
    grpc_port: int | None = None,
    default_blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
    server_memory_limit: str = "25%",
) -> None:
    """
    Serve log-data over gRPC and serve a Rerun web viewer over HTTP.

    You can also connect to this server with the native viewer using `rerun rerun+http://localhost:{grpc_port}/proxy`.

    The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
    You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
    Once reached, the earliest logged data will be dropped. Static data is never dropped.

    This function returns immediately.

    Calling `serve_web` is equivalent to calling [`rerun.serve_grpc`][] followed by [`rerun.serve_web_viewer`][].
    ```
    server_uri = rr.serve_grpc(grpc_port=grpc_port, default_blueprint=default_blueprint, server_memory_limit=server_memory_limit)
    rr.serve_web_viewer(web_port=web_port, open_browser=open_browser, connect_to=server_uri)
    ```

    Parameters
    ----------
    open_browser:
        Open the default browser to the viewer.
    web_port:
        The port to serve the web viewer on (defaults to 9090).
    grpc_port:
        The port to serve the gRPC server on (defaults to 9876)
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

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording=recording)  # NOLINT
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before connecting to a viewer, or provide a recording.",
        )

    # If a blueprint is provided, we need to create a blueprint storage object
    blueprint_storage = None
    if default_blueprint is not None:
        blueprint_storage = create_in_memory_blueprint(
            application_id=application_id,
            blueprint=default_blueprint,
        ).storage

    # TODO(#5531): keep static data around.
    bindings.serve_web(
        open_browser,
        web_port,
        grpc_port,
        server_memory_limit=server_memory_limit,
        default_blueprint=blueprint_storage,
        recording=recording.to_native() if recording is not None else None,
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

    from rerun.recording_stream import get_application_id

    application_id = get_application_id(recording=recording)  # NOLINT

    if application_id is None:
        raise ValueError("No application id found. You must call rerun.init before sending a blueprint.")

    blueprint_storage = create_in_memory_blueprint(application_id=application_id, blueprint=blueprint).storage

    bindings.send_blueprint(
        blueprint_storage,
        make_active,
        make_default,
        recording=recording.to_native() if recording is not None else None,
    )


def send_recording(rrd: Recording, recording: RecordingStream | None = None) -> None:
    """
    Send a `Recording` loaded from a `.rrd` to the `RecordingStream`.

    .. warning::
        ⚠️ This API is experimental and may change or be removed in future versions! ⚠️

    Parameters
    ----------
    rrd:
        A recording loaded from a `.rrd` file.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    application_id = get_application_id(recording=recording)  # NOLINT

    if application_id is None:
        raise ValueError("No application id found. You must call rerun.init before sending a recording.")

    bindings.send_recording(
        rrd,
        recording=recording.to_native() if recording is not None else None,
    )


def spawn(
    *,
    port: int = 9876,
    connect: bool = True,
    memory_limit: str = "75%",
    server_memory_limit: str = "0B",
    hide_welcome_screen: bool = False,
    detach_process: bool = True,
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
    server_memory_limit:
        An upper limit on how much memory the gRPC server running
        in the same process as the Rerun Viewer should use.
        When this limit is reached, Rerun will drop the oldest data.
        Example: `16GB` or `50%` (of system total).

        Defaults to `0B`.
    hide_welcome_screen:
        Hide the normal Rerun welcome screen.
    detach_process:
        Detach Rerun Viewer process from the application process.
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

    _spawn_viewer(
        port=port,
        memory_limit=memory_limit,
        server_memory_limit=server_memory_limit,
        hide_welcome_screen=hide_welcome_screen,
        detach_process=detach_process,
    )

    if connect:
        connect_grpc(
            f"rerun+http://127.0.0.1:{port}/proxy",
            recording=recording,  # NOLINT
            default_blueprint=default_blueprint,
        )
