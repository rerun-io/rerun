"""The Rerun Python SDK, which is a wrapper around the re_sdk crate."""

import atexit
import logging
import sys
from inspect import getmembers, isfunction
from typing import Optional

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun import experimental
from rerun.log.annotation import AnnotationInfo, ClassDescription, log_annotation_context
from rerun.log.arrow import log_arrow
from rerun.log.bounding_box import log_obb
from rerun.log.camera import log_pinhole
from rerun.log.clear import log_cleared
from rerun.log.extension_components import log_extension_components
from rerun.log.file import ImageFormat, MeshFormat, log_image_file, log_mesh_file
from rerun.log.image import log_depth_image, log_image, log_segmentation_image
from rerun.log.lines import log_line_segments, log_line_strip, log_path
from rerun.log.mesh import log_mesh, log_meshes
from rerun.log.points import log_point, log_points
from rerun.log.rects import RectFormat, log_rect, log_rects
from rerun.log.scalar import log_scalar
from rerun.log.tensor import log_tensor
from rerun.log.text import LoggingHandler, LogLevel, log_text_entry
from rerun.log.transform import log_rigid3, log_unknown_transform, log_view_coordinates
from rerun.recording_stream import (
    RecordingStream,
    get_application_id,
    get_data_recording,
    get_global_data_recording,
    get_recording_id,
    get_thread_local_data_recording,
    is_enabled,
    set_global_data_recording,
    set_thread_local_data_recording,
)

# --- Init RecordingStream class ---
from rerun.recording_stream import _patch as recording_stream_patch
from rerun.script_helpers import script_add_args, script_setup, script_teardown
from rerun.sinks import connect, disconnect, memory_recording, save, serve, spawn
from rerun.time import reset_time, set_time_nanos, set_time_seconds, set_time_sequence

# Inject all relevant methods into the `RecordingStream` class.
# We need to do this from here to avoid circular import issues.
recording_stream_patch(
    [connect, save, disconnect, memory_recording, serve, spawn]
    + [set_time_sequence, set_time_seconds, set_time_nanos, reset_time]
    + [fn for name, fn in getmembers(sys.modules[__name__], isfunction) if name.startswith("log_")]
)  # type: ignore[no-untyped-call]

# ---

__all__ = [
    # init
    "init",
    "new_recording",
    "rerun_shutdown",
    # recordings
    "RecordingStream",
    "is_enabled",
    "get_application_id",
    "get_recording_id",
    "get_data_recording",
    "get_global_data_recording",
    "set_global_data_recording",
    "get_thread_local_data_recording",
    "set_thread_local_data_recording",
    # time
    "reset_time",
    "set_time_nanos",
    "set_time_seconds",
    "set_time_sequence",
    # sinks
    "connect",
    "disconnect",
    "memory_recording",
    "save",
    "serve",
    "spawn",
    # log functions
    "log_annotation_context",
    "log_arrow",
    "log_cleared",
    "log_depth_image",
    "log_extension_components",
    "log_image",
    "log_image_file",
    "log_line_segments",
    "log_line_strip",
    "log_mesh",
    "log_mesh_file",
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
    # classes
    "AnnotationInfo",
    "ClassDescription",
    "ImageFormat",
    "LogLevel",
    "LoggingHandler",
    "MeshFormat",
    "RectFormat",
    # special
    "bindings",
    "experimental",
    # script helpers
    "script_add_args",
    "script_setup",
    "script_teardown",
]


# If `True`, we raise exceptions on use error (wrong parameter types etc).
# If `False` we catch all errors and log a warning instead.
_strict_mode = False


# --- Init ---


def init(
    application_id: str,
    recording_id: Optional[str] = None,
    spawn: bool = False,
    default_enabled: bool = True,
    strict: bool = False,
) -> None:
    """
    Initialize the Rerun SDK with a user-chosen application id (name).

    You must call this function first in order to initialize a global recording.
    Without an active recording, all methods of the SDK will turn into no-ops.

    For more advanced use cases, e.g. multiple recordings setups, see [`rerun.new_recording`][].

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For example, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    recording_id : Optional[str]
        Set the recording ID that this process is logging to, as a UUIDv4.

        The default recording_id is based on `multiprocessing.current_process().authkey`
        which means that all processes spawned with `multiprocessing`
        will have the same default recording_id.

        If you are not using `multiprocessing` and still want several different Python
        processes to log to the same Rerun instance (and be part of the same recording),
        you will need to manually assign them all the same recording_id.
        Any random UUIDv4 will work, or copy the recording id for the parent process.
    spawn : bool
        Spawn a Rerun Viewer and stream logging data to it.
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

    new_recording(
        application_id,
        recording_id,
        True,  # make_default
        False,  # make_thread_default
        spawn,
        default_enabled,
    )


def new_recording(
    application_id: str,
    recording_id: Optional[str] = None,
    make_default: bool = False,
    make_thread_default: bool = False,
    spawn: bool = False,
    default_enabled: bool = True,
) -> RecordingStream:
    """
    Creates a new recording with a user-chosen application id (name) that can be used to log data.

    If you only need a single global recording, [`rerun.init`][] might be simpler.

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For example, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    recording_id : Optional[str]
        Set the recording ID that this process is logging to, as a UUIDv4.

        The default recording_id is based on `multiprocessing.current_process().authkey`
        which means that all processes spawned with `multiprocessing`
        will have the same default recording_id.

        If you are not using `multiprocessing` and still want several different Python
        processes to log to the same Rerun instance (and be part of the same recording),
        you will need to manually assign them all the same recording_id.
        Any random UUIDv4 will work, or copy the recording id for the parent process.
    make_default : bool
        If true (_not_ the default), the newly initialized recording will replace the current
        active one (if any) in the global scope.
    make_thread_default : bool
        If true (_not_ the default), the newly initialized recording will replace the current
        active one (if any) in the thread-local scope.
    spawn : bool
        Spawn a Rerun Viewer and stream logging data to it.
        Short for calling `spawn` separately.
        If you don't call this, log events will be buffered indefinitely until
        you call either `connect`, `show`, or `save`
    default_enabled
        Should Rerun logging be on by default?
        Can overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.

    Returns
    -------
    RecordingStream
        A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

    """

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
            path = pathlib.Path(str(filename)).resolve()  # normalize before comparison!
            if "rerun/examples" in str(path):
                application_path = path
    except Exception:
        pass

    recording = RecordingStream(
        bindings.new_recording(
            application_id=application_id,
            recording_id=recording_id,
            make_default=make_default,
            make_thread_default=make_thread_default,
            application_path=application_path,
            default_enabled=default_enabled,
        )
    )

    if spawn:
        from rerun.sinks import spawn as _spawn

        _spawn(recording=recording)

    return recording


def version() -> str:
    """
    Returns a verbose version string of the Rerun SDK.

    Example: `rerun_py 0.6.0-alpha.0 [rustc 1.69.0 (84c898d65 2023-04-16), LLVM 15.0.7] aarch64-apple-darwin main bd8a072, built 2023-05-11T08:25:17Z`
    """  # noqa: E501 line too long
    return bindings.version()  # type: ignore[no-any-return]


def rerun_shutdown() -> None:
    bindings.shutdown()


atexit.register(rerun_shutdown)


def unregister_shutdown() -> None:
    atexit.unregister(rerun_shutdown)


# ---


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
        logging.warning(
            "Rerun is disabled - start_web_viewer_server() call ignored. You must call rerun.init before starting the"
            + " web viewer server."
        )
        return

    bindings.start_web_viewer_server(port)
