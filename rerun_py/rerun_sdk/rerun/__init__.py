from __future__ import annotations

import functools
import random
from typing import Any, Callable, TypeVar, cast
from uuid import UUID

import numpy as np

# =====================================
# API RE-EXPORTS

__all__ = [
    "AnnotationContext",
    "AnnotationInfo",
    "AnyValues",
    "Arrows3D",
    "AsComponents",
    "Asset3D",
    "BarChart",
    "Box2DFormat",
    "Boxes2D",
    "Boxes3D",
    "ClassDescription",
    "Clear",
    "ComponentBatchLike",
    "DepthImage",
    "DisconnectedSpace",
    "Image",
    "ImageEncoded",
    "ImageFormat",
    "IndicatorComponentBatch",
    "LineStrips2D",
    "LineStrips3D",
    "LoggingHandler",
    "Material",
    "MediaType",
    "MemoryRecording",
    "Mesh3D",
    "MeshProperties",
    "OutOfTreeTransform3D",
    "OutOfTreeTransform3DBatch",
    "Pinhole",
    "Points2D",
    "Points3D",
    "Quaternion",
    "RecordingStream",
    "RotationAxisAngle",
    "Scalar",
    "Scale3D",
    "SegmentationImage",
    "SeriesLine",
    "SeriesPoint",
    "Tensor",
    "TensorData",
    "TextDocument",
    "TextLog",
    "TextLogLevel",
    "TimeSeriesScalar",
    "Transform3D",
    "TranslationAndMat3x3",
    "TranslationRotationScale3D",
    "ViewCoordinates",
    "archetypes",
    "bindings",
    "components",
    "connect",
    "datatypes",
    "disable_timeline",
    "disconnect",
    "escape_entity_path_part",
    "experimental",
    "get_application_id",
    "get_data_recording",
    "get_global_data_recording",
    "get_recording_id",
    "get_thread_local_data_recording",
    "is_enabled",
    "log_components",
    "log",
    "memory_recording",
    "new_entity_path",
    "reset_time",
    "save",
    "script_add_args",
    "script_setup",
    "script_teardown",
    "serve",
    "set_global_data_recording",
    "set_thread_local_data_recording",
    "set_time_nanos",
    "set_time_seconds",
    "set_time_sequence",
    "spawn",
]

import rerun_bindings as bindings  # type: ignore[attr-defined]

from ._image import ImageEncoded, ImageFormat
from ._log import (
    AsComponents,
    ComponentBatchLike,
    IndicatorComponentBatch,
    escape_entity_path_part,
    log,
    log_components,
    new_entity_path,
)
from .any_value import AnyValues
from .archetypes import (
    AnnotationContext,
    Arrows2D,
    Arrows3D,
    Asset3D,
    BarChart,
    Boxes2D,
    Boxes3D,
    Clear,
    DepthImage,
    DisconnectedSpace,
    Image,
    LineStrips2D,
    LineStrips3D,
    Mesh3D,
    Pinhole,
    Points2D,
    Points3D,
    Scalar,
    SegmentationImage,
    SeriesLine,
    SeriesPoint,
    Tensor,
    TextDocument,
    TextLog,
    TimeSeriesScalar,
    Transform3D,
    ViewCoordinates,
)
from .archetypes.boxes2d_ext import Box2DFormat
from .components import (
    Material,
    MediaType,
    MeshProperties,
    OutOfTreeTransform3D,
    OutOfTreeTransform3DBatch,
    TextLogLevel,
)
from .datatypes import (
    AnnotationInfo,
    ClassDescription,
    Quaternion,
    RotationAxisAngle,
    Scale3D,
    TensorData,
    TranslationAndMat3x3,
    TranslationRotationScale3D,
)
from .error_utils import set_strict_mode
from .logging_handler import LoggingHandler
from .recording import MemoryRecording
from .recording_stream import (
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
from .script_helpers import script_add_args, script_setup, script_teardown
from .sinks import connect, disconnect, memory_recording, save, serve, spawn, stdout
from .time import (
    disable_timeline,
    reset_time,
    set_time_nanos,
    set_time_seconds,
    set_time_sequence,
)

# Import experimental last
from . import experimental  # isort: skip


# =====================================
# UTILITIES

__all__ += [
    "EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE",
    "cleanup_if_forked_child",
    "init",
    "new_recording",
    "rerun_shutdown",
    "set_strict_mode",
    "shutdown_at_exit",
    "start_web_viewer_server",
    "unregister_shutdown",
    "version",
]


# NOTE: Always keep in sync with other languages.
EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE = 66
"""
When an external `DataLoader` is asked to load some data that it doesn't know how to load, it
should exit with this exit code.
"""


def _init_recording_stream() -> None:
    # Inject all relevant methods into the `RecordingStream` class.
    # We need to do this from here to avoid circular import issues.

    import sys
    from inspect import getmembers, isfunction

    from rerun.recording_stream import _patch as recording_stream_patch

    recording_stream_patch(
        [connect, save, stdout, disconnect, memory_recording, serve, spawn]
        + [
            set_time_sequence,
            set_time_seconds,
            set_time_nanos,
            disable_timeline,
            reset_time,
        ]
        + [fn for name, fn in getmembers(sys.modules[__name__], isfunction) if name.startswith("log_")]
    )


_init_recording_stream()


# TODO(#3793): defaulting recording_id to authkey should be opt-in
def init(
    application_id: str,
    *,
    recording_id: str | UUID | None = None,
    spawn: bool = False,
    init_logging: bool = True,
    default_enabled: bool = True,
    strict: bool = False,
    exp_init_blueprint: bool = False,
    exp_add_to_app_default_blueprint: bool = True,
) -> None:
    """
    Initialize the Rerun SDK with a user-chosen application id (name).

    You must call this function first in order to initialize a global recording.
    Without an active recording, all methods of the SDK will turn into no-ops.

    For more advanced use cases, e.g. multiple recordings setups, see [`rerun.new_recording`][].

    !!! Warning
        If you don't specify a `recording_id`, it will default to a random value that is generated once
        at the start of the process.
        That value will be kept around for the whole lifetime of the process, and even inherited by all
        its subprocesses, if any.

        This makes it trivial to log data to the same recording in a multiprocess setup, but it also means
        that the following code will _not_ create two distinct recordings:
        ```
        rr.init("my_app")
        rr.init("my_app")
        ```

        To create distinct recordings from the same process, specify distinct recording IDs:
        ```
        from uuid import uuid4
        rr.init("my_app", recording_id=uuid4())
        rr.init("my_app", recording_id=uuid4())
        ```

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For example, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.

        Application ids starting with `rerun_example_` are reserved for Rerun examples,
        and will be treated specially by the Rerun Viewer.
        In particular, it will opt-in to more analytics, and will also
        seed the global random number generator deterministically.
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
        Can be overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.
    init_logging
        Should we initialize the logging for this application?
    strict
        If `True`, an exceptions is raised on use error (wrong parameter types, etc.).
        If `False`, errors are logged as warnings instead.
    exp_init_blueprint
        (Experimental) Should we initialize the blueprint for this application?
    exp_add_to_app_default_blueprint
        (Experimental) Should the blueprint append to the existing app-default blueprint instead of creating a new one.

    """

    if application_id.startswith("rerun_example_"):
        # Make all our example code deterministic.
        random.seed(0)
        np.random.seed(0)

    set_strict_mode(strict)

    # Always check whether we are a forked child when calling init. This should have happened
    # via `_register_on_fork` but it's worth being conservative.
    cleanup_if_forked_child()

    if recording_id is not None:
        recording_id = str(recording_id)

    if init_logging:
        new_recording(
            application_id=application_id,
            recording_id=recording_id,
            make_default=True,
            make_thread_default=False,
            spawn=False,
            default_enabled=default_enabled,
        )
    if exp_init_blueprint:
        experimental.new_blueprint(
            application_id=application_id,
            blueprint_id=recording_id,
            make_default=True,
            make_thread_default=False,
            spawn=False,
            add_to_app_default_blueprint=exp_add_to_app_default_blueprint,
            default_enabled=default_enabled,
        )

    if spawn:
        from rerun.sinks import spawn as _spawn

        _spawn()


# TODO(#3793): defaulting recording_id to authkey should be opt-in
def new_recording(
    *,
    application_id: str,
    recording_id: str | UUID | None = None,
    make_default: bool = False,
    make_thread_default: bool = False,
    spawn: bool = False,
    default_enabled: bool = True,
) -> RecordingStream:
    """
    Creates a new recording with a user-chosen application id (name) that can be used to log data.

    If you only need a single global recording, [`rerun.init`][] might be simpler.

    !!! Warning
        If you don't specify a `recording_id`, it will default to a random value that is generated once
        at the start of the process.
        That value will be kept around for the whole lifetime of the process, and even inherited by all
        its subprocesses, if any.

        This makes it trivial to log data to the same recording in a multiprocess setup, but it also means
        that the following code will _not_ create two distinct recordings:
        ```
        rr.init("my_app")
        rr.init("my_app")
        ```

        To create distinct recordings from the same process, specify distinct recording IDs:
        ```
        from uuid import uuid4
        rec = rr.new_recording(application_id="test", recording_id=uuid4())
        rec = rr.new_recording(application_id="test", recording_id=uuid4())
        ```

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
        Can be overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.

    Returns
    -------
    RecordingStream
        A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

    """

    application_path = None

    # NOTE: It'd be even nicer to do such thing on the Rust-side so that this little trick would
    # only need to be written once and just work for all languages out of the box… unfortunately
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

    if recording_id is not None:
        recording_id = str(recording_id)

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


def _register_shutdown() -> None:
    import atexit

    atexit.register(rerun_shutdown)


_register_shutdown()


def unregister_shutdown() -> None:
    import atexit

    atexit.unregister(rerun_shutdown)


def cleanup_if_forked_child() -> None:
    bindings.cleanup_if_forked_child()


def _register_on_fork() -> None:
    # Only relevant on Linux
    try:
        import os

        os.register_at_fork(after_in_child=cleanup_if_forked_child)
    except AttributeError:
        pass


_register_on_fork()


_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


def shutdown_at_exit(func: _TFunc) -> _TFunc:
    """
    Decorator to shutdown Rerun cleanly when this function exits.

    Normally, Rerun installs an atexit-handler that attempts to shutdown cleanly and
    flush all outgoing data before terminating. However, some cases, such as forked
    processes will always skip this at-exit handler. In these cases, you can use this
    decorator on the entry-point to your subprocess to ensure cleanup happens as
    expected without losing data.
    """

    @functools.wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        try:
            return func(*args, **kwargs)
        finally:
            rerun_shutdown()

    return cast(_TFunc, wrapper)


# ---


def start_web_viewer_server(port: int = 0) -> None:
    """
    Start an HTTP server that hosts the rerun web viewer.

    This only provides the web-server that makes the viewer available and
    does not otherwise provide a rerun websocket server or facilitate any routing of
    data.

    This is generally only necessary for application such as running a jupyter notebook
    in a context where app.rerun.io is unavailable, or does not have the matching
    resources for your build (such as when running from source.)

    Parameters
    ----------
    port
        Port to serve assets on. Defaults to 0 (random port).
    """

    if not bindings.is_enabled():
        import logging

        logging.warning(
            "Rerun is disabled - start_web_viewer_server() call ignored. You must call rerun.init before starting the"
            + " web viewer server."
        )
        return

    bindings.start_web_viewer_server(port)
