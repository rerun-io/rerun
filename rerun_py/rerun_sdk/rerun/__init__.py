"""The Rerun Python SDK, which is a wrapper around the rerun_sdk crate."""

import atexit
from typing import Optional

import rerun_bindings as bindings  # type: ignore[attr-defined]
from rerun.log import log_cleared
from rerun.log.annotation import log_annotation_context
from rerun.log.arrow import log_arrow
from rerun.log.bounding_box import log_obb
from rerun.log.camera import log_pinhole
from rerun.log.extension_components import log_extension_components
from rerun.log.file import log_image_file, log_mesh_file
from rerun.log.image import log_depth_image, log_image, log_segmentation_image
from rerun.log.lines import log_line_segments, log_path
from rerun.log.mesh import log_mesh, log_meshes
from rerun.log.points import log_point, log_points
from rerun.log.rects import log_rect, log_rects
from rerun.log.scalar import log_scalar
from rerun.log.tensor import log_tensor
from rerun.log.text import log_text_entry
from rerun.log.transform import log_rigid3, log_unknown_transform, log_view_coordinates
from rerun.script_helpers import script_add_args, script_setup, script_teardown

__all__ = [
    "bindings",
    "components",
    "log_annotation_context",
    "log_arrow",
    "log_cleared",
    "log_cleared",
    "log_depth_image",
    "log_image",
    "log_image_file",
    "log_line_segments",
    "log_mesh",
    "log_meshes",
    "log_mesh_file",
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
    "log_extension_components",
    "log_view_coordinates",
    "script_add_args",
    "script_setup",
    "script_teardown",
    "LoggingHandler",
]


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


def init(application_id: str, spawn_and_connect: bool = False) -> None:
    """
    Initialize the Rerun SDK with a user-chosen application id (name).

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For instance, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    spawn_and_connect : bool
        Spawn a Rerun Viewer and stream logging data to it.
        Short for calling `spawn_and_connect` separately.
        If you don't call this, log events will be buffered indefinitely until
        you call either `connect`, `show`, or `save`

    """
    app_path = None

    # NOTE: It'd be even nicer to do such thing on the Rust-side so that this little trick would
    # only need to be written once and just work for all languages out of the box... unfortunately
    # we lose most of the details of the python part of the backtrace once we go over the bridge.
    #
    # Still, better than nothing!
    try:
        import inspect
        import pathlib

        app_path = pathlib.Path(inspect.stack()[1][1]).resolve()
    except Exception:
        pass

    bindings.init(application_id, app_path)

    if spawn_and_connect:
        _spawn_and_connect()


def spawn_and_connect(port: int = 9876) -> None:
    """
    Spawn a Rerun Viewer and stream logging data to it.

    This is often the easiest and best way to use Rerun.
    Just call this once at the start of your program.

    Parameters
    ----------
    port : int
        The port to connect to

    See Also
    --------
     * [rerun.connect][]

    """
    spawn_viewer(port)
    connect(f"127.0.0.1:{port}")


_spawn_and_connect = spawn_and_connect  # we need this because Python scoping is horrible


def connect(addr: Optional[str] = None) -> None:
    """
    Connect to a remote Rerun Viewer on the given ip:port.

    Requires that you first start a Rerun Viewer, e.g. with 'python -m rerun'

    Parameters
    ----------
    addr : str
        The ip:port to connect to

    """
    bindings.connect(addr)


def spawn_viewer(port: int = 9876) -> None:
    """
    Spawn a Rerun Viewer, listening on the given port.

    Parameters
    ----------
    port : int
        The port to listen on.

    """
    import subprocess
    import sys
    from time import sleep

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    # start_new_session=True ensures the spawned process does NOT die when
    # we hit ctrl-c in the terminal running the parent Python process.
    rerun_process = subprocess.Popen([python_executable, "-m", "rerun", "--port", str(port)], start_new_session=True)
    print(f"Spawned Rerun Viewer with pid {rerun_process.pid}")

    # TODO(emilk): figure out a way to postpone connecting until the rerun viewer is listening.
    # For instance, wait until it prints "Hosting a SDK server over TCP at …"
    sleep(0.2)  # almost as good as waiting the correct amount of time


def serve() -> None:
    """
    Serve a Rerun Web Viewer.

    WARNING: This is an experimental feature.
    """
    bindings.serve()


def disconnect() -> None:
    """Disconnect from the remote rerun server (if any)."""
    bindings.disconnect()


def show() -> None:
    """
    Show previously logged data.

    This only works if you have not called `connect`.

    This will clear the logged data after showing it.

    NOTE: There is a bug which causes this function to only work once on some platforms.

    """
    bindings.show()


def save(path: str) -> None:
    """
    Save previously logged data to a file.

    This only works if you have not called `connect`.

    This will clear the logged data after saving.

    Parameters
    ----------
    path : str
        The path to save the data to.

    """
    bindings.save(path)


def set_time_sequence(timeline: str, sequence: Optional[int]) -> None:
    """
    Set the current time for this thread as an integer sequence.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `set_time_sequence("frame_nr", None)`.

    There is no requirement of monoticity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    sequence : int
        The current time on the timeline in integer units.

    """
    bindings.set_time_sequence(timeline, sequence)


def set_time_seconds(timeline: str, seconds: Optional[float]) -> None:
    """
    Set the current time for this thread in seconds.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_seconds`.

    For instance: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a timeline again using `set_time_seconds("capture_time", None)`.

    The argument should be in seconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The bindings has a built-in time which is `log_time`, and is logged as seconds
    since unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    seconds : float
        The current time on the timeline in seconds.

    """
    bindings.set_time_seconds(timeline, seconds)


def set_time_nanos(timeline: str, nanos: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_nanos`.

    For instance: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a timeline again using `set_time_nanos("capture_time", None)`.

    The argument should be in nanoseconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The bindings has a built-in time which is `log_time`, and is logged as nanos since
    unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.

    Parameters
    ----------
    timeline : str
        The name of the timeline to set the time for.
    nanos : int
        The current time on the timeline in nanoseconds.

    """
    bindings.set_time_nanos(timeline, nanos)
