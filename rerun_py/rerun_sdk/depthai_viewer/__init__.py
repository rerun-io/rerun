"""The Rerun Python SDK, which is a wrapper around the re_sdk crate."""

import atexit
from typing import Optional

import rerun_bindings as bindings  # type: ignore[attr-defined]

from depthai_viewer.log import log_cleared
from depthai_viewer.log.annotation import AnnotationInfo, ClassDescription, log_annotation_context
from depthai_viewer.log.arrow import log_arrow
from depthai_viewer.log.bounding_box import log_obb
from depthai_viewer.log.camera import log_pinhole
from depthai_viewer.log.extension_components import log_extension_components
from depthai_viewer.log.file import ImageFormat, MeshFormat, log_image_file, log_mesh_file
from depthai_viewer.log.image import log_depth_image, log_image, log_segmentation_image
from depthai_viewer.log.imu import log_imu
from depthai_viewer.log.lines import log_line_segments, log_line_strip, log_path
from depthai_viewer.log.mesh import log_mesh, log_meshes
from depthai_viewer.log.pipeline_graph import log_pipeline_graph
from depthai_viewer.log.points import log_point, log_points
from depthai_viewer.log.rects import RectFormat, log_rect, log_rects
from depthai_viewer.log.scalar import log_scalar
from depthai_viewer.log.tensor import log_tensor
from depthai_viewer.log.text import LoggingHandler, LogLevel, log_text_entry
from depthai_viewer.log.transform import log_rigid3, log_unknown_transform, log_view_coordinates
from depthai_viewer.recording import MemoryRecording
from depthai_viewer.script_helpers import script_add_args, script_setup, script_teardown
from depthai_viewer.log.xlink_stats import log_xlink_stats
from depthai_viewer import _backend

__all__ = [
    "AnnotationInfo",
    "ClassDescription",
    "LoggingHandler",
    "bindings",
    "components",
    "inline_show",
    "ImageFormat",
    "log_annotation_context",
    "log_arrow",
    "log_cleared",
    "log_depth_image",
    "log_extension_components",
    "log_image_file",
    "log_image",
    "log_pipeline_graph",
    "log_line_segments",
    "log_line_strip",
    "log_mesh_file",
    "log_mesh",
    "log_meshes",
    "log_obb",
    "log_path",
    "log_pinhole",
    "log_point",
    "log_points",
    "log_rect",
    "log_rects",
    "log_rigid3",
    "log_scalar",
    "log_segmentation_image",
    "log_tensor",
    "log_text_entry",
    "log_unknown_transform",
    "log_view_coordinates",
    "notebook",
    "LogLevel",
    "MeshFormat",
    "RectFormat",
    "script_add_args",
    "script_setup",
    "script_teardown",
    "log_imu",
    "log_xlink_stats",
    "_backend",
]


# If `True`, we raise exceptions on use error (wrong parameter types etc).
# If `False` we catch all errors and log a warning instead.
_strict_mode = False


def rerun_shutdown() -> None:
    bindings.shutdown()


atexit.register(rerun_shutdown)


def unregister_shutdown() -> None:
    atexit.unregister(rerun_shutdown)


# -----------------------------------------------------------------------------


def get_recording_id() -> str:
    """
    Get the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python
    processes to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.

    Returns
    -------
    str
        The recording ID that this process is logging to.

    """
    return str(bindings.get_recording_id())


def set_recording_id(value: str) -> None:
    """
    Set the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python
    processes to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.

    Parameters
    ----------
    value : str
        The recording ID to use for this process.

    """
    bindings.set_recording_id(value)


def init(application_id: str, spawn: bool = False, default_enabled: bool = True, strict: bool = False) -> None:
    """
    Initialize the Rerun SDK with a user-chosen application id (name).

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For example, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    spawn : bool
        Spawn a Depthai Viewer and stream logging data to it.
        Short for calling `spawn` separately.
        If you don't call this, log events will be buffered indefinitely until
        you call either `connect`, `show`, or `save`
    default_enabled
        Should Rerun logging be on by default?
        Can overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.
    strict
        If `True`, an exceptions is raised on use error (wrong parameter types etc).
        If `False`, errors are logged as warnings instead.

    """

    _strict_mode = strict
    application_path = None

    # NOTE: It'd be even nicer to do such thing on the Rust-side so that this little trick would
    # only need to be written once and just work for all languages out of the box... unfortunately
    # we lose most of the details of the python part of the backtrace once we go over the bridge.
    #
    # Still, better than nothing!
    try:
        import inspect
        import pathlib

        # We're trying to grab the filesystem path of the example script that called `init()`.
        # The tricky part is that we don't know how many layers are between this script and the
        # original caller, so we have to walk the stack and look for anything that might look like
        # an official Rerun example.

        MAX_FRAMES = 10  # try the first 10 frames, should be more than enough
        FRAME_FILENAME_INDEX = 1  # `FrameInfo` tuple has `filename` at index 1

        stack = inspect.stack()
        for frame in stack[:MAX_FRAMES]:
            filename = frame[FRAME_FILENAME_INDEX]
            # normalize before comparison!
            path = pathlib.Path(str(filename)).resolve()
            if "rerun/examples" in str(path):
                application_path = path
    except Exception:
        pass

    bindings.init(
        application_id=application_id,
        application_path=application_path,
        default_enabled=default_enabled,
    )

    if spawn:
        _spawn()


def is_enabled() -> bool:
    """
    Is the Rerun SDK enabled.

    If false, all calls to the rerun library are ignored.

    The default can be set in [`rerun.init`][], but is otherwise `True`.

    This can be controlled with the environment variable `RERUN`,
    (e.g. `RERUN=on` or `RERUN=off`) and with [`set_enabled`].

    """
    return bindings.is_enabled()  # type: ignore[no-any-return]


def set_enabled(enabled: bool) -> None:
    """
    Enable or disable logging.

    If false, all calls to the rerun library are ignored. The default is `True`.

    This is a global setting that affects all threads.

    By default logging is enabled, but can be controlled with the environment variable `RERUN`,
    (e.g. `RERUN=on` or `RERUN=off`).

    The default can be set in [`rerun.init`][].

    Parameters
    ----------
    enabled : bool
        Whether to enable or disable logging.

    """
    bindings.set_enabled(enabled)


def strict_mode() -> bool:
    """
    Strict mode enabled.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is on, such problems are instead logged as warnings.

    The default is OFF.
    """

    return _strict_mode


def set_strict_mode(strict_mode: bool) -> None:
    """
    Turn strict mode on/off.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is off, such problems are instead logged as warnings.

    The default is OFF.
    """

    _strict_mode = strict_mode


def connect(addr: Optional[str] = None) -> None:
    """
    Connect to a remote Depthai Viewer on the given ip:port.

    Requires that you first start a Depthai Viewer, e.g. with 'python -m rerun'

    This function returns immediately.

    Parameters
    ----------
    addr
        The ip:port to connect to

    """

    if not bindings.is_enabled():
        print("Rerun is disabled - connect() call ignored")
        return

    bindings.connect(addr)


_connect = connect  # we need this because Python scoping is horrible


def spawn(port: int = 9876, connect: bool = True) -> None:
    """
    Spawn a Depthai Viewer, listening on the given port.

    This is often the easiest and best way to use Rerun.
    Just call this once at the start of your program.

    You can also call [rerun.init][] with a `spawn=True` argument.

    Parameters
    ----------
    port : int
        The port to listen on.
    connect
        also connect to the viewer and stream logging data to it.

    """

    if not bindings.is_enabled():
        print("Rerun is disabled - spawn() call ignored")
        return

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
    subprocess.Popen(
        [python_executable, "-m", "depthai_viewer", "--port", str(port)], env=new_env, start_new_session=True
    )

    # TODO(emilk): figure out a way to postpone connecting until the rerun viewer is listening.
    # For example, wait until it prints "Hosting a SDK server over TCP at …"
    sleep(0.5)  # almost as good as waiting the correct amount of time

    if connect:
        _connect(f"127.0.0.1:{port}")


_spawn = spawn  # we need this because Python scoping is horrible


def serve(open_browser: bool = True, web_port: Optional[int] = None, ws_port: Optional[int] = None) -> None:
    """
    Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.

    You can connect to this server using `python -m rerun`.

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
    """

    if not bindings.is_enabled():
        print("Rerun is disabled - serve() call ignored")
        return

    bindings.serve(open_browser, web_port, ws_port)


def start_web_viewer_server(port: int = 0) -> None:
    """
    Start an HTTP server that hosts the rerun web viewer.

    This only provides the web-server that makes the viewer available and
    does not otherwise provide a rerun websocket server or facilitate any routing of
    data.

    This is generally only necessary for application such as running a jupyter notebook
    in a context where app.rerun.io is unavailable, or does not having the matching
    resources for your build (such as when running from source.)

    Parameters
    ----------
    port
        Port to serve assets on. Defaults to 0 (random port).
    """

    if not bindings.is_enabled():
        print("Rerun is disabled - self_host_assets() call ignored")
        return

    bindings.start_web_viewer_server(port)


def disconnect() -> None:
    """
    Closes all TCP connections, servers, and files.

    Closes all TCP connections, servers, and files that have been opened with
    [`rerun.connect`], [`rerun.serve`], [`rerun.save`] or [`rerun.spawn`].
    """

    bindings.disconnect()


def save(path: str) -> None:
    """
    Stream all log-data to a file.

    Parameters
    ----------
    path : str
        The path to save the data to.

    """

    if not bindings.is_enabled():
        print("Rerun is disabled - save() call ignored")
        return

    bindings.save(path)


def memory_recording() -> MemoryRecording:
    """
    Streams all log-data to a memory buffer.

    This can be used to display the RRD to alternative formats such as html.
    See: [rerun.MemoryRecording.as_html][].

    Returns
    -------
    MemoryRecording
        A memory recording object that can be used to read the data.
    """

    return MemoryRecording(bindings.memory_recording())


def set_time_sequence(timeline: str, sequence: Optional[int]) -> None:
    """
    Set the current time for this thread as an integer sequence.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For example: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `set_time_sequence("frame_nr", None)`.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    sequence : int
        The current time on the timeline in integer units.

    """

    if not bindings.is_enabled():
        return

    bindings.set_time_sequence(timeline, sequence)


def set_time_seconds(timeline: str, seconds: Optional[float]) -> None:
    """
    Set the current time for this thread in seconds.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time_seconds`][] or [`rerun.set_time_nanos`][].

    For example: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a timeline again using `set_time_seconds("capture_time", None)`.

    Very large values will automatically be interpreted as seconds since unix epoch (1970-01-01).
    Small values (less than a few years) will be interpreted as relative
    some unknown point in time, and will be shown as e.g. `+3.132s`.

    The bindings has a built-in time which is `log_time`, and is logged as seconds
    since unix epoch.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    seconds : float
        The current time on the timeline in seconds.

    """

    if not bindings.is_enabled():
        return

    bindings.set_time_seconds(timeline, seconds)


def set_time_nanos(timeline: str, nanos: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time_nanos`][] or [`rerun.set_time_seconds`][].

    For example: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a timeline again using `set_time_nanos("capture_time", None)`.

    Very large values will automatically be interpreted as nanoseconds since unix epoch (1970-01-01).
    Small values (less than a few years) will be interpreted as relative
    some unknown point in time, and will be shown as e.g. `+3.132s`.

    The bindings has a built-in time which is `log_time`, and is logged as nanos since
    unix epoch.

    There is no requirement of monotonicity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    nanos : int
        The current time on the timeline in nanoseconds.

    """

    if not bindings.is_enabled():
        return

    bindings.set_time_nanos(timeline, nanos)


def reset_time() -> None:
    """
    Clear all timeline information on this thread.

    This is the same as calling `set_time_*` with `None` for all of the active timelines.

    Used for all subsequent logging on the same thread,
    until the next call to [`rerun.set_time_nanos`][] or [`rerun.set_time_seconds`][].
    """

    if not bindings.is_enabled():
        return

    bindings.reset_time()
