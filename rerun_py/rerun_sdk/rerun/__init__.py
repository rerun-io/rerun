from __future__ import annotations

import functools
import random
from typing import Any, Callable, TypeVar, cast
from uuid import UUID

import numpy as np

__version__ = "0.18.1-rc.1"
__version_info__ = (0, 18, 1, "rc.1")

# =====================================
# API RE-EXPORTS
# Important: always us the `import _ as _` format to make it explicit to type-checkers that these are public APIs.
# Background: https://github.com/microsoft/pyright/blob/1.1.365/docs/typed-libraries.md#library-interface
#
import rerun_bindings as bindings  # type: ignore[attr-defined]

from . import (
    blueprint as blueprint,
    experimental as experimental,
    notebook as notebook,
)
from ._baseclasses import (
    ComponentColumn as ComponentColumn,
)
from ._image_encoded import (
    ImageEncoded as ImageEncoded,
    ImageFormat as ImageFormat,
)
from ._log import (
    AsComponents as AsComponents,
    ComponentBatchLike as ComponentBatchLike,
    IndicatorComponentBatch as IndicatorComponentBatch,
    escape_entity_path_part as escape_entity_path_part,
    log as log,
    log_components as log_components,
    log_file_from_contents as log_file_from_contents,
    log_file_from_path as log_file_from_path,
    new_entity_path as new_entity_path,
)
from ._send_columns import (
    TimeNanosColumn as TimeNanosColumn,
    TimeSecondsColumn as TimeSecondsColumn,
    TimeSequenceColumn as TimeSequenceColumn,
    send_columns as send_columns,
)
from .any_value import (
    AnyValues as AnyValues,
)
from .archetypes import (
    AnnotationContext as AnnotationContext,
    Arrows2D as Arrows2D,
    Arrows3D as Arrows3D,
    Asset3D as Asset3D,
    BarChart as BarChart,
    Boxes2D as Boxes2D,
    Boxes3D as Boxes3D,
    Clear as Clear,
    DepthImage as DepthImage,
    DisconnectedSpace as DisconnectedSpace,
    Ellipsoids3D as Ellipsoids3D,
    EncodedImage as EncodedImage,
    Image as Image,
    InstancePoses3D as InstancePoses3D,
    LineStrips2D as LineStrips2D,
    LineStrips3D as LineStrips3D,
    Mesh3D as Mesh3D,
    Pinhole as Pinhole,
    Points2D as Points2D,
    Points3D as Points3D,
    Scalar as Scalar,
    SegmentationImage as SegmentationImage,
    SeriesLine as SeriesLine,
    SeriesPoint as SeriesPoint,
    Tensor as Tensor,
    TextDocument as TextDocument,
    TextLog as TextLog,
    Transform3D as Transform3D,
    ViewCoordinates as ViewCoordinates,
)
from .archetypes.boxes2d_ext import (
    Box2DFormat as Box2DFormat,
)
from .blueprint.api import (
    BlueprintLike as BlueprintLike,
)
from .components import (
    AlbedoFactor as AlbedoFactor,
    MediaType as MediaType,
    Radius as Radius,
    Scale3D as Scale3D,
    TensorDimensionIndexSelection as TensorDimensionIndexSelection,
    TextLogLevel as TextLogLevel,
    TransformRelation as TransformRelation,
)
from .datatypes import (
    Angle as Angle,
    AnnotationInfo as AnnotationInfo,
    ChannelDatatype as ChannelDatatype,
    ClassDescription as ClassDescription,
    ColorModel as ColorModel,
    PixelFormat as PixelFormat,
    Quaternion as Quaternion,
    RotationAxisAngle as RotationAxisAngle,
    TensorData as TensorData,
    TensorDimensionSelection as TensorDimensionSelection,
    TimeInt as TimeInt,
    TimeRange as TimeRange,
    TimeRangeBoundary as TimeRangeBoundary,
    VisibleTimeRange as VisibleTimeRange,
)
from .error_utils import (
    set_strict_mode as set_strict_mode,
)
from .legacy_notebook import (
    legacy_notebook_show as legacy_notebook_show,
)
from .logging_handler import (
    LoggingHandler as LoggingHandler,
)
from .memory import (
    MemoryRecording as MemoryRecording,
    memory_recording as memory_recording,
)
from .notebook import (
    notebook_show as notebook_show,
)
from .recording_stream import (
    BinaryStream as BinaryStream,
    RecordingStream as RecordingStream,
    binary_stream as binary_stream,
    get_application_id as get_application_id,
    get_data_recording as get_data_recording,
    get_global_data_recording as get_global_data_recording,
    get_recording_id as get_recording_id,
    get_thread_local_data_recording as get_thread_local_data_recording,
    is_enabled as is_enabled,
    new_recording as new_recording,
    recording_stream_generator_ctx as recording_stream_generator_ctx,
    set_global_data_recording as set_global_data_recording,
    set_thread_local_data_recording as set_thread_local_data_recording,
    thread_local_stream as thread_local_stream,
)
from .script_helpers import (
    script_add_args as script_add_args,
    script_setup as script_setup,
    script_teardown as script_teardown,
)
from .sinks import (
    connect as connect,
    disconnect as disconnect,
    save as save,
    send_blueprint as send_blueprint,
    serve as serve,
    spawn as spawn,
    stdout as stdout,
)
from .time import (
    disable_timeline as disable_timeline,
    reset_time as reset_time,
    set_time_nanos as set_time_nanos,
    set_time_seconds as set_time_seconds,
    set_time_sequence as set_time_sequence,
)

# =====================================
# UTILITIES


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
        [
            binary_stream,
            connect,
            save,
            stdout,
            disconnect,
            memory_recording,
            serve,
            spawn,
            send_blueprint,
            notebook_show,
        ]
        + [
            set_time_sequence,
            set_time_seconds,
            set_time_nanos,
            disable_timeline,
            reset_time,
            log,
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
    strict: bool | None = None,
    default_blueprint: BlueprintLike | None = None,
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
        If `True`, an exception is raised on use error (wrong parameter types, etc.).
        If `False`, errors are logged as warnings instead.
        If unset, this can alternatively be overridden using the RERUN_STRICT environment variable.
        If not otherwise specified, the default behavior will be equivalent to `False`.
    default_blueprint
        Optionally set a default blueprint to use for this application. If the application
        already has an active blueprint, the new blueprint won't become active until the user
        clicks the "reset blueprint" button. If you want to activate the new blueprint
        immediately, instead use the [`rerun.send_blueprint`][] API.

    """

    if application_id.startswith("rerun_example_"):
        # Make all our example code deterministic.
        random.seed(0)
        np.random.seed(0)

    if strict is not None:
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

    if spawn:
        from rerun.sinks import spawn as _spawn

        _spawn(default_blueprint=default_blueprint)


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

        os.register_at_fork(after_in_child=cleanup_if_forked_child)  # type: ignore[attr-defined]
    except AttributeError:
        # not defined on all OSes
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

    bindings.start_web_viewer_server(port)
