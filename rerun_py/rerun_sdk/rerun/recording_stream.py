from __future__ import annotations

import contextvars
import functools
import inspect
import uuid
from collections.abc import Iterable
from datetime import datetime, timedelta
from pathlib import Path
from types import TracebackType
from typing import TYPE_CHECKING, Any, Callable, TypeVar, overload

import numpy as np
from typing_extensions import deprecated

import rerun as rr
from rerun import bindings
from rerun.memory import MemoryRecording

if TYPE_CHECKING:
    from rerun import AsComponents, BlueprintLike, ComponentColumn, DescribedComponentBatch
    from rerun.sinks import LogSinkLike

    from ._send_columns import TimeColumnLike


# TODO(#3793): defaulting recording_id to authkey should be opt-in


@deprecated(
    """Please migrate to `rr.RecordingStream(…)`.
    See: https://www.rerun.io/docs/reference/migration/migration-0-23 for more details.""",
)
def new_recording(
    application_id: str,
    *,
    recording_id: str | uuid.UUID | None = None,
    make_default: bool = False,
    make_thread_default: bool = False,
    spawn: bool = False,
    default_enabled: bool = True,
) -> RecordingStream:
    """
    Creates a new recording with a user-chosen application id (name) that can be used to log data.

    If you only need a single global recording, [`rerun.init`][] might be simpler.

    Note that unless setting `spawn=True` new recording streams always begin connected to a buffered sink.
    To send the data to a viewer or file you will likely want to call [`rerun.connect_grpc`][] or [`rerun.save`][]
    explicitly.

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
        you call either `connect_grpc`, `show`, or `save`
    default_enabled
        Should Rerun logging be on by default?
        Can be overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.

    Returns
    -------
    RecordingStream
        A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

    """

    recording_stream = RecordingStream(
        application_id,
        recording_id=recording_id,
        make_default=make_default,
        make_thread_default=make_thread_default,
        default_enabled=default_enabled,
    )

    if spawn:
        from rerun.sinks import spawn as _spawn

        _spawn(recording=recording_stream)  # NOLINT

    return recording_stream


active_recording_stream: contextvars.ContextVar[RecordingStream] = contextvars.ContextVar("active_recording_stream")
"""
A context variable that tracks the currently active recording stream.

Used to manage and detect interactions between generators and RecordingStream context-manager objects.
"""


def binary_stream(recording: RecordingStream | None = None) -> BinaryStream:
    """
    Sends all log-data to a [`rerun.BinaryStream`] object that can be read from.

    The contents of this stream are encoded in the Rerun Record Data format (rrd).

    This stream has no mechanism of limiting memory or creating back-pressure. If you do not
    read from it, it will buffer all messages that you have logged.

    Example
    -------
    ```python
    stream = rr.binary_stream()

    rr.log("stream", rr.TextLog("Hello world"))

    with open("output.rrd", "wb") as f:
        f.write(stream.read())
    ```

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    BinaryStream
        An object that can be used to flush or read the data.

    """
    inner = bindings.binary_stream(recording=recording.to_native() if recording is not None else None)
    if inner is None:
        raise RuntimeError("No recording stream was provided and set as current")
    return BinaryStream(inner)


def is_enabled(
    recording: RecordingStream | None = None,
) -> bool:
    """
    Is this Rerun recording enabled.

    If false, all calls to the recording are ignored.

    The default can be set in [`rerun.init`][], but is otherwise `True`.

    This can be controlled with the environment variable `RERUN` (e.g. `RERUN=on` or `RERUN=off`).

    """
    return bindings.is_enabled(recording=recording.to_native() if recording is not None else None)  # type: ignore[no-any-return]


def get_application_id(
    recording: RecordingStream | None = None,
) -> str | None:
    """
    Get the application ID that this recording is associated with, if any.

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    str
        The application ID that this recording is associated with.

    """
    app_id = bindings.get_application_id(recording=recording.to_native() if recording is not None else None)
    return str(app_id) if app_id is not None else None


def get_recording_id(
    recording: RecordingStream | None = None,
) -> str | None:
    """
    Get the recording ID that this recording is logging to, as a UUIDv4, if any.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python
    processes to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    str
        The recording ID that this recording is logging to.

    """
    rec_id = bindings.get_recording_id(recording=recording.to_native() if recording is not None else None)
    return str(rec_id) if rec_id is not None else None


class RecordingStream:
    """
    A RecordingStream is used to send data to Rerun.

    You can instantiate a RecordingStream by calling either [`rerun.init`][] (to create a global
    recording) or [`rerun.RecordingStream`][] (for more advanced use cases).

    Multithreading
    --------------

    A RecordingStream can safely be copied and sent to other threads.
    You can also set a recording as the global active one for all threads ([`rerun.set_global_data_recording`][])
    or just for the current thread ([`rerun.set_thread_local_data_recording`][]).

    Similarly, the `with` keyword can be used to temporarily set the active recording for the
    current thread, e.g.:
    ```
    with rec:
        rr.log(...)
    ```
    WARNING: if using a RecordingStream as a context manager, yielding from a generator function
    while holding the context open will leak the context and likely cause your program to send data
    to the wrong stream. See: <https://github.com/rerun-io/rerun/issues/6238>. You can work around this
    by using the [`rerun.recording_stream_generator_ctx`][] decorator.

    Flushing or context manager exit guarantees that all previous data sent by the calling thread
    has been recorded and (if applicable) flushed to the underlying OS-managed file descriptor,
    but other threads may still have data in flight.

    See also: [`rerun.get_data_recording`][], [`rerun.get_global_data_recording`][],
    [`rerun.get_thread_local_data_recording`][].

    Available methods
    -----------------

    Every function in the Rerun SDK that takes an optional RecordingStream as a parameter can also
    be called as a method on RecordingStream itself.

    This includes, but isn't limited to:

    - Metadata-related functions:
        [`rerun.is_enabled`][], [`rerun.get_recording_id`][], …
    - Sink-related functions:
        [`rerun.connect_grpc`][], [`rerun.spawn`][], …
    - Time-related functions:
        [`rerun.set_time`][], [`rerun.disable_timeline`][], [`rerun.reset_time`][], …
    - Log-related functions:
        [`rerun.log`][], …

    For an exhaustive list, see `help(rerun.RecordingStream)`.

    Micro-batching
    --------------

    Micro-batching using both space and time triggers (whichever comes first) is done automatically
    in a dedicated background thread.

    You can configure the frequency of the batches using the following environment variables:

    - `RERUN_FLUSH_TICK_SECS`:
        Flush frequency in seconds (default: `0.05` (50ms)).
    - `RERUN_FLUSH_NUM_BYTES`:
        Flush threshold in bytes (default: `1048576` (1MiB)).
    - `RERUN_FLUSH_NUM_ROWS`:
        Flush threshold in number of rows (default: `18446744073709551615` (u64::MAX)).

    """

    def __init__(
        self,
        application_id: str,
        *,
        recording_id: str | uuid.UUID | None = None,
        make_default: bool = False,
        make_thread_default: bool = False,
        default_enabled: bool = True,
        send_properties: bool = True,
    ) -> None:
        """
        Creates a new recording stream with a user-chosen application id (name) that can be used to log data.

        If you only need a single global recording, [`rerun.init`][] might be simpler.

        Note that new recording streams always begin connected to a buffered sink.
        To send the data to a viewer or file you will likely want to call [`rerun.connect_grpc`][] or [`rerun.save`][]
        explicitly.

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
            rec = rr.RecordingStream(application_id="test", recording_id=uuid4())
            rec = rr.RecordingStream(application_id="test", recording_id=uuid4())
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
        default_enabled : bool
            Should Rerun logging be on by default?
            Can be overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.
        send_properties
            Immediately send the recording properties to the viewer (default: True)

        Returns
        -------
        RecordingStream
            A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

        Examples
        --------
        Using a recording stream object directly.
        ```python
        from uuid import uuid4
        stream = rr.RecordingStream("my_app", recording_id=uuid4())
        stream.connect_grpc()
        stream.log("hello", rr.TextLog("Hello world"))
        ```

        """

        if recording_id is not None:
            recording_id = str(recording_id)

        self.inner = bindings.new_recording(
            application_id=application_id,
            recording_id=recording_id,
            make_default=make_default,
            make_thread_default=make_thread_default,
            default_enabled=default_enabled,
            send_properties=send_properties,
        )

        self._prev: RecordingStream | None = None
        self.context_token: contextvars.Token[RecordingStream] | None = None

    @classmethod
    def _from_native(cls, native_recording: bindings.PyRecordingStream) -> RecordingStream:
        self = cls.__new__(cls)
        self.inner = native_recording
        self._prev = None
        self.context_token = None
        return self

    def __enter__(self) -> RecordingStream:
        self.context_token = active_recording_stream.set(self)
        self._prev = set_thread_local_data_recording(self)
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        self.flush(blocking=True)

        current_recording = active_recording_stream.get(None)

        # Restore the context state
        if self.context_token is not None:
            active_recording_stream.reset(self.context_token)

        # Restore the recording stream state
        set_thread_local_data_recording(self._prev)
        self._prev = None

        # Sanity check: we set this context-var on enter. If it's not still set, something weird
        # happened. The user is probably doing something sketch with generators or async code.
        if current_recording is not self:
            raise RuntimeError(
                "RecordingStream context manager exited while not active. Likely mixing context managers with generators or async code. See: `recording_stream_generator_ctx`.",
            )

    # NOTE: The type is a string because we cannot reference `RecordingStream` yet at this point.
    def to_native(self) -> bindings.PyRecordingStream:
        return self.inner

    def flush(self, blocking: bool = True) -> None:
        """
        Initiates a flush the batching pipeline and optionally waits for it to propagate to the underlying file descriptor (if any).

        Parameters
        ----------
        blocking:
            If true, the flush will block until the flush is complete.

        """
        bindings.flush(blocking, recording=self.to_native())

    def __del__(self) -> None:  # type: ignore[no-untyped-def]
        recording = self.to_native()
        # TODO(jleibs): I'm 98% sure this flush is redundant, but removing it requires more thorough testing.
        # However, it's definitely a problem if we are in a forked child process. The rerun SDK will still
        # detect this case and prevent a hang internally, but will do so with a warning that we should avoid.
        #
        # See: https://github.com/rerun-io/rerun/issues/6223 for context on why this is necessary.
        if not recording.is_forked_child():
            bindings.flush(blocking=False, recording=recording)  # NOLINT

    # any free function taking a `RecordingStream` as the first argument can also be a method
    binary_stream = binary_stream
    get_application_id = get_application_id
    get_recording_id = get_recording_id
    is_enabled = is_enabled

    def set_sinks(
        self,
        *sinks: LogSinkLike,
        default_blueprint: BlueprintLike | None = None,
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

        Example
        -------
        ```py
        rec = rr.RecordingStream("rerun_example_tee")
        rec.set_sinks(
            rr.GrpcSink(),
            rr.FileSink("data.rrd")
        )
        rec.log("my/point", rr.Points3D(position=[1.0, 2.0, 3.0]))
        ```

        """

        from .sinks import set_sinks

        set_sinks(*sinks, default_blueprint=default_blueprint, recording=self)

    def connect_grpc(
        self,
        url: str | None = None,
        *,
        flush_timeout_sec: float | None = 2.0,
        default_blueprint: BlueprintLike | None = None,
    ) -> None:
        """
        Connect to a remote Rerun Viewer on the given URL.

        This function returns immediately.

        Parameters
        ----------
        url:
            The URL to connect to

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

        """

        from .sinks import connect_grpc

        connect_grpc(url, flush_timeout_sec=flush_timeout_sec, default_blueprint=default_blueprint, recording=self)

    def save(self, path: str | Path, default_blueprint: BlueprintLike | None = None) -> None:
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

        """

        from .sinks import save

        save(path, default_blueprint, recording=self)

    def stdout(self, default_blueprint: BlueprintLike | None = None) -> None:
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

        """

        from .sinks import stdout

        stdout(default_blueprint, recording=self)

    def memory_recording(self) -> MemoryRecording:
        """
        Streams all log-data to a memory buffer.

        This can be used to display the RRD to alternative formats such as html.
        See: [rerun.notebook_show][].

        Returns
        -------
        MemoryRecording
            A memory recording object that can be used to read the data.

        """

        from .memory import memory_recording

        return memory_recording(self)

    def disconnect(self) -> None:
        """
        Closes all gRPC connections, servers, and files.

        Closes all gRPC connections, servers, and files that have been opened with
        [`rerun.RecordingStream.connect_grpc`], [`rerun.RecordingStream.serve`], [`rerun.RecordingStream.save`] or
        [`rerun.RecordingStream.spawn`].
        """

        from .sinks import disconnect

        disconnect(recording=self)

    def serve_grpc(
        self,
        *,
        grpc_port: int | None = None,
        default_blueprint: BlueprintLike | None = None,
        server_memory_limit: str = "25%",
    ) -> str:
        """
        Serve log-data over gRPC.

        You can to this server with the native viewer using `rerun rerun+http://localhost:{grpc_port}/proxy`.

        The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
        You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
        Once reached, the earliest logged data will be dropped. Static data is never dropped.

        It is highly recommended that you set the memory limit to `0B` if both the server and client are running
        on the same machine, otherwise you're potentially doubling your memory usage!

        Returns the URI of the server so you can connect the viewer to it.

        This function returns immediately.

        Parameters
        ----------
        grpc_port:
            The port to serve the gRPC server on (defaults to 9876)
        default_blueprint:
            Optionally set a default blueprint to use for this application. If the application
            already has an active blueprint, the new blueprint won't become active until the user
            clicks the "reset blueprint" button. If you want to activate the new blueprint
            immediately, instead use the [`rerun.RecordingStream.send_blueprint`][] API.
        server_memory_limit:
            Maximum amount of memory to use for buffering log data for clients that connect late.
            This can be a percentage of the total ram (e.g. "50%") or an absolute value (e.g. "4GB").

        """

        from .sinks import serve_grpc

        return serve_grpc(
            grpc_port=grpc_port,
            default_blueprint=default_blueprint,
            server_memory_limit=server_memory_limit,
            recording=self,
        )

    @deprecated(
        """Use a combination of `serve_grpc` and `rr.serve_web_viewer` instead.
        See: https://www.rerun.io/docs/reference/migration/migration-0-24?speculative-link for more details.""",
    )
    def serve_web(
        self,
        *,
        open_browser: bool = True,
        web_port: int | None = None,
        grpc_port: int | None = None,
        default_blueprint: BlueprintLike | None = None,
        server_memory_limit: str = "25%",
    ) -> None:
        """
        Serve log-data over gRPC and serve a Rerun web viewer over HTTP.

        You can also connect to this server with the native viewer using `rerun localhost:9090`.

        The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
        You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
        Once reached, the earliest logged data will be dropped. Static data is never dropped.

        This function returns immediately.

        Calling `serve_web` is equivalent to calling [`rerun.RecordingStream.serve_grpc`][] followed by [`rerun.serve_web_viewer`][]:
        ```
        server_uri = rec.serve_grpc(grpc_port=grpc_port, default_blueprint=default_blueprint, server_memory_limit=server_memory_limit)
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
            immediately, instead use the [`rerun.RecordingStream.send_blueprint`][] API.
        server_memory_limit:
            Maximum amount of memory to use for buffering log data for clients that connect late.
            This can be a percentage of the total ram (e.g. "50%") or an absolute value (e.g. "4GB").

        """

        from .sinks import serve_web

        serve_web(
            open_browser=open_browser,
            web_port=web_port,
            grpc_port=grpc_port,
            default_blueprint=default_blueprint,
            server_memory_limit=server_memory_limit,
            recording=self,
        )

    def send_blueprint(
        self,
        blueprint: BlueprintLike,
        *,
        make_active: bool = True,
        make_default: bool = True,
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

        """

        from .sinks import send_blueprint

        send_blueprint(blueprint=blueprint, make_active=make_active, make_default=make_default, recording=self)

    def send_recording(self, recording: rr.dataframe.Recording) -> None:
        """
        Send a `Recording` loaded from a `.rrd` to the `RecordingStream`.

        .. warning::
            ⚠️ This API is experimental and may change or be removed in future versions! ⚠️

        Parameters
        ----------
        recording:
            A `Recording` loaded from a `.rrd`.

        """

        from .sinks import send_recording

        send_recording(rrd=recording, recording=self)

    def spawn(
        self,
        *,
        port: int = 9876,
        connect: bool = True,
        memory_limit: str = "75%",
        hide_welcome_screen: bool = False,
        detach_process: bool = True,
        default_blueprint: BlueprintLike | None = None,
    ) -> None:
        """
        Spawn a Rerun Viewer, listening on the given port.

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
        detach_process:
            Detach Rerun Viewer process from the application process.
        default_blueprint
            Optionally set a default blueprint to use for this application. If the application
            already has an active blueprint, the new blueprint won't become active until the user
            clicks the "reset blueprint" button. If you want to activate the new blueprint
            immediately, instead use the [`rerun.RecordingStream.send_blueprint`][] API.

        """

        from .sinks import spawn

        spawn(
            port=port,
            connect=connect,
            memory_limit=memory_limit,
            hide_welcome_screen=hide_welcome_screen,
            detach_process=detach_process,
            default_blueprint=default_blueprint,
            recording=self,
        )

    def notebook_show(
        self,
        *,
        width: int | None = None,
        height: int | None = None,
        blueprint: BlueprintLike | None = None,
    ) -> None:
        """
        Output the Rerun viewer in a notebook using IPython [IPython.core.display.HTML][].

        Any data logged to the recording after initialization will be sent directly to the viewer.

        Note that this can be called at any point during cell execution. The call will block until the embedded
        viewer is initialized and ready to receive data. Thereafter any log calls will immediately send data
        to the viewer.

        Parameters
        ----------
        width : int
            The width of the viewer in pixels.
        height : int
            The height of the viewer in pixels.
        blueprint : BlueprintLike
            A blueprint object to send to the viewer.
            It will be made active and set as the default blueprint in the recording.

            Setting this is equivalent to calling [`rerun.RecordingStream.send_blueprint`][] before initializing the viewer.

        """
        try:
            from .notebook import Viewer

            Viewer(
                width=width,
                height=height,
                blueprint=blueprint,
                recording=self,
            ).display()
        except ImportError as e:
            raise Exception("Could not import rerun_notebook. Please install `rerun-notebook`.") from e
        except FileNotFoundError as e:
            raise Exception(
                "rerun_notebook package is missing widget assets. Please run `py-build-notebook` in your pixi env."
            ) from e

    def send_property(
        self,
        name: str,
        values: AsComponents | Iterable[DescribedComponentBatch],
    ) -> None:
        """
            Send a property of the recording.

        Parameters
        ----------
        name:
            Name of the property.

        values:
            Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype,
            or an iterable of (described)component batches.

        """

        from ._properties import send_property

        send_property(name=name, values=values, recording=self)

    def send_recording_name(self, name: str) -> None:
        """
        Send the name of the recording.

        This name is shown in the Rerun Viewer.

        Parameters
        ----------
        name : str
            The name of the recording.

        """

        from ._properties import send_recording_name

        send_recording_name(name, recording=self)

    def send_recording_start_time_nanos(self, nanos: int) -> None:
        """
        Send the start time of the recording.

        This timestamp is shown in the Rerun Viewer.

        Parameters
        ----------
        nanos : int
            The start time of the recording.

        """

        from ._properties import send_recording_start_time_nanos

        send_recording_start_time_nanos(nanos, recording=self)

    @overload
    def set_time(self, timeline: str, *, sequence: int) -> None: ...

    @overload
    def set_time(self, timeline: str, *, duration: int | float | timedelta | np.timedelta64) -> None: ...

    @overload
    def set_time(self, timeline: str, *, timestamp: int | float | datetime | np.datetime64) -> None: ...

    def set_time(
        self,
        timeline: str,
        *,
        sequence: int | None = None,
        duration: int | float | timedelta | np.timedelta64 | None = None,
        timestamp: int | float | datetime | np.datetime64 | None = None,
    ) -> None:
        """
        Set the current time of a timeline for this thread.

        Used for all subsequent logging on the same thread, until the next call to
        [`rerun.RecordingStream.set_time`][], [`rerun.RecordingStream.reset_time`][] or
        [`rerun.RecordingStream.disable_timeline`][].

        For example: `set_time("frame_nr", sequence=frame_nr)`.

        There is no requirement of monotonicity. You can move the time backwards if you like.

        You are expected to set exactly ONE of the arguments `sequence`, `duration`, or `timestamp`.
        You may NOT change the type of a timeline, so if you use `duration` for a specific timeline,
        you must only use `duration` for that timeline going forward.

        The columnar equivalent to this function is [`rerun.TimeColumn`][].

        Parameters
        ----------
        timeline : str
            The name of the timeline to set the time for.
        sequence:
            Used for sequential indices, like `frame_nr`.
            Must be an integer.
        duration:
            Used for relative times, like `time_since_start`.
            Must either be in seconds, a [`datetime.timedelta`][], or [`numpy.timedelta64`][].
            For nanosecond precision, use `numpy.timedelta64(nanoseconds, 'ns')`.
        timestamp:
            Used for absolute time indices, like `capture_time`.
            Must either be in seconds since Unix epoch, a [`datetime.datetime`][], or [`numpy.datetime64`][].
            For nanosecond precision, use `numpy.datetime64(nanoseconds, 'ns')`.

        """

        from .time import set_time

        # mypy appears to not be smart enough to understand how the above @overload make the following call valid.
        set_time(  # type: ignore[call-overload]
            timeline=timeline,
            duration=duration,
            sequence=sequence,
            timestamp=timestamp,
            recording=self,
        )

    @deprecated(
        """Use `RecordingStream.set_time(sequence=…)` instead.
        See: https://www.rerun.io/docs/reference/migration/migration-0-23 for more details.""",
    )
    def set_time_sequence(self, timeline: str, sequence: int) -> None:
        """
        Set the current time for this thread as an integer sequence.

        Used for all subsequent logging on the same thread,
        until the next call to `set_time_sequence`.

        For example: `set_time_sequence("frame_nr", frame_nr)`.

        You can remove a timeline again using `disable_timeline("frame_nr")`.

        There is no requirement of monotonicity. You can move the time backwards if you like.

        This function marks the timeline as being of a _squential_ type.
        You should not use the temporal functions ([`rerun.set_time_seconds`][], [`rerun.set_time_nanos`][])
        on the same timeline, as that will produce undefined behavior.

        Parameters
        ----------
        timeline : str
            The name of the timeline to set the time for.
        sequence : int
            The current time on the timeline in integer units.

        """

        from .time import set_time_sequence

        set_time_sequence(timeline=timeline, sequence=sequence, recording=self)

    @deprecated(
        """Use `RecordingStream.set_time(timestamp=seconds)` or `RecordingStream.set_time(duration=seconds)` instead.
        See: https://www.rerun.io/docs/reference/migration/migration-0-23 for more details.""",
    )
    def set_time_seconds(self, timeline: str, seconds: float) -> None:
        """
        Set the current time for this thread in seconds.

        Used for all subsequent logging on the same thread,
        until the next call to [`rerun.set_time_seconds`][] or [`rerun.set_time_nanos`][].

        For example: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

        You can remove a timeline again using `disable_timeline("capture_time")`.

        Very large values will automatically be interpreted as seconds since unix epoch (1970-01-01).
        Small values (less than a few years) will be interpreted as relative
        some unknown point in time, and will be shown as e.g. `+3.132s`.

        The bindings has a built-in time which is `log_time`, and is logged as seconds
        since unix epoch.

        There is no requirement of monotonicity. You can move the time backwards if you like.

        This function marks the timeline as being of a _temporal_ type.
        You should not use the sequential function [`rerun.set_time_sequence`][]
        on the same timeline, as that will produce undefined behavior.

        Parameters
        ----------
        timeline : str
            The name of the timeline to set the time for.
        seconds : float
            The current time on the timeline in seconds.

        """

        from .time import set_time_seconds

        set_time_seconds(timeline=timeline, seconds=seconds, recording=self)

    @deprecated(
        """Use `RecordingStream.set_time(timestamp=1e-9 * nanos)` or `RecordingStream.set_time(duration=1e-9 * nanos)` instead.
        See: https://www.rerun.io/docs/reference/migration/migration-0-23 for more details.""",
    )
    def set_time_nanos(self, timeline: str, nanos: int) -> None:
        """
        Set the current time for this thread.

        Used for all subsequent logging on the same thread,
        until the next call to [`rerun.set_time_nanos`][] or [`rerun.set_time_seconds`][].

        For example: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

        You can remove a timeline again using `disable_timeline("capture_time")`.

        Very large values will automatically be interpreted as nanoseconds since unix epoch (1970-01-01).
        Small values (less than a few years) will be interpreted as relative
        some unknown point in time, and will be shown as e.g. `+3.132s`.

        The bindings has a built-in time which is `log_time`, and is logged as nanos since
        unix epoch.

        There is no requirement of monotonicity. You can move the time backwards if you like.

        This function marks the timeline as being of a _temporal_ type.
        You should not use the sequential function [`rerun.set_time_sequence`][]
        on the same timeline, as that will produce undefined behavior.

        Parameters
        ----------
        timeline : str
            The name of the timeline to set the time for.
        nanos : int
            The current time on the timeline in nanoseconds.

        """

        from .time import set_time_nanos

        set_time_nanos(timeline=timeline, nanos=nanos, recording=self)

    def disable_timeline(self, timeline: str) -> None:
        """
        Clear time information for the specified timeline on this thread.

        Parameters
        ----------
        timeline : str
            The name of the timeline to clear the time for.

        """

        from .time import disable_timeline

        disable_timeline(timeline=timeline, recording=self)

    # TODO(emilk): rename to something with the word `index`, and maybe unify with `disable_timeline`?
    def reset_time(self) -> None:
        """
        Clear all timeline information on this thread.

        This is the same as calling `disable_timeline` for all of the active timelines.

        Used for all subsequent logging on the same thread,
        until the next call to [`rerun.RecordingStream.set_time`][].
        """

        bindings.reset_time(recording=self.to_native())

    def log(
        self,
        entity_path: str | list[object],
        entity: AsComponents | Iterable[DescribedComponentBatch],
        *extra: AsComponents | Iterable[DescribedComponentBatch],
        static: bool = False,
        strict: bool | None = None,
    ) -> None:
        r"""
        Log data to Rerun.

        This is the main entry point for logging data to rerun. It can be used to log anything
        that implements the [`rerun.AsComponents`][] interface, or a collection of `ComponentBatchLike` objects.

        When logging data, you must always provide an [entity_path](https://www.rerun.io/docs/concepts/entity-path)
        for identifying the data. Note that paths prefixed with "__" are considered reserved for use by the Rerun SDK
        itself and should not be used for logging user data. This is where Rerun will log additional information
        such as properties and warnings.

        The most common way to log is with one of the rerun archetypes, all of which implement
        the `AsComponents` interface.

        For example, to log a 3D point:
        ```py
        rr.log("my/point", rr.Points3D(position=[1.0, 2.0, 3.0]))
        ```

        The `log` function can flexibly accept an arbitrary number of additional objects which will
        be merged into the first entity so long as they don't expose conflicting components, for instance:
        ```py
        # Log three points with arrows sticking out of them,
        # and a custom "confidence" component.
        rr.log(
            "my/points",
            rr.Points3D([[0.2, 0.5, 0.3], [0.9, 1.2, 0.1], [1.0, 4.2, 0.3]], radii=[0.1, 0.2, 0.3]),
            rr.Arrows3D(vectors=[[0.3, 2.1, 0.2], [0.9, -1.1, 2.3], [-0.4, 0.5, 2.9]]),
            rr.AnyValues(confidence=[0.3, 0.4, 0.9]),
        )
        ```

        Parameters
        ----------
        entity_path:
            Path to the entity in the space hierarchy.

            The entity path can either be a string
            (with special characters escaped, split on unescaped slashes)
            or a list of unescaped strings.
            This means that logging to `"world/my\ image\!"` is the same as logging
            to ["world", "my image!"].

            See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.

        entity:
            Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype.

        *extra:
            An arbitrary number of additional component bundles implementing the [`rerun.AsComponents`][]
            interface, that are logged to the same entity path.

        static:
            If true, the components will be logged as static data.

            Static data has no time associated with it, exists on all timelines, and unconditionally shadows
            any temporal data of the same type.

            Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
            Additional timelines set by [`rerun.RecordingStream.set_time`][] will also be included.

        strict:
            If True, raise exceptions on non-loggable data.
            If False, warn on non-loggable data.
            if None, use the global default from `rerun.strict_mode()`

        """

        from ._log import log

        log(entity_path, entity, *extra, static=static, strict=strict, recording=self)

    def log_file_from_contents(
        self,
        file_path: str | Path,
        file_contents: bytes,
        *,
        entity_path_prefix: str | None = None,
        static: bool = False,
    ) -> None:
        r"""
        Logs the given `file_contents` using all `DataLoader`s available.

        A single `path` might be handled by more than one loader.

        This method blocks until either at least one `DataLoader` starts
        streaming data in or all of them fail.

        See <https://www.rerun.io/docs/getting-started/data-in/open-any-file> for more information.

        Parameters
        ----------
        file_path:
            Path to the file that the `file_contents` belong to.

        file_contents:
            Contents to be logged.

        entity_path_prefix:
            What should the logged entity paths be prefixed with?

        static:
            If true, the components will be logged as static data.

            Static data has no time associated with it, exists on all timelines, and unconditionally shadows
            any temporal data of the same type.

            Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
            Additional timelines set by [`rerun.RecordingStream.set_time`][] will also be included.

        """

        from ._log import log_file_from_contents

        log_file_from_contents(
            file_path=file_path,
            file_contents=file_contents,
            entity_path_prefix=entity_path_prefix,
            static=static,
            recording=self,
        )

    def log_file_from_path(
        self,
        file_path: str | Path,
        *,
        entity_path_prefix: str | None = None,
        static: bool = False,
    ) -> None:
        r"""
        Logs the file at the given `path` using all `DataLoader`s available.

        A single `path` might be handled by more than one loader.

        This method blocks until either at least one `DataLoader` starts
        streaming data in or all of them fail.

        See <https://www.rerun.io/docs/getting-started/data-in/open-any-file> for more information.

        Parameters
        ----------
        file_path:
            Path to the file to be logged.

        entity_path_prefix:
            What should the logged entity paths be prefixed with?

        static:
            If true, the components will be logged as static data.

            Static data has no time associated with it, exists on all timelines, and unconditionally shadows
            any temporal data of the same type.

            Otherwise, the data will be timestamped automatically with `log_time` and `log_tick`.
            Additional timelines set by [`rerun.RecordingStream.set_time`][] will also be included.

        """

        from ._log import log_file_from_path

        log_file_from_path(
            file_path=file_path,
            entity_path_prefix=entity_path_prefix,
            static=static,
            recording=self,
        )

    def send_columns(
        self,
        entity_path: str,
        indexes: Iterable[TimeColumnLike],
        columns: Iterable[ComponentColumn],
        strict: bool | None = None,
    ) -> None:
        r"""
        Send columnar data to Rerun.

        Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
        in a columnar form. Each `TimeColumnLike` and `ComponentColumn` object represents a column
        of data that will be sent to Rerun. The lengths of all these columns must match, and all
        data that shares the same index across the different columns will act as a single logical row,
        equivalent to a single call to `rr.log()`.

        Note that this API ignores any stateful time set on the log stream via [`rerun.RecordingStream.set_time`][].
        Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.

        Parameters
        ----------
        entity_path:
            Path to the entity in the space hierarchy.

            See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
        indexes:
            The time values of this batch of data. Each `TimeColumnLike` object represents a single column
            of timestamps. Generally, you should use one of the provided classes: [`TimeSequenceColumn`][rerun.TimeSequenceColumn],
            [`TimeSecondsColumn`][rerun.TimeSecondsColumn], or [`TimeNanosColumn`][rerun.TimeNanosColumn].
        columns:
            The columns of components to log. Each object represents a single column of data.

            In order to send multiple components per time value, explicitly create a [`ComponentColumn`][rerun.ComponentColumn]
            either by constructing it directly, or by calling the `.columns()` method on an `Archetype` type.
        strict:
            If True, raise exceptions on non-loggable data.
            If False, warn on non-loggable data.
            If None, use the global default from `rerun.strict_mode()`

        """

        from ._send_columns import send_columns

        send_columns(entity_path=entity_path, indexes=indexes, columns=columns, strict=strict, recording=self)


class BinaryStream:
    """An encoded stream of bytes that can be saved as an rrd or sent to the viewer."""

    def __init__(self, storage: bindings.PyBinarySinkStorage) -> None:
        self.storage = storage

    def read(self, *, flush: bool = True) -> bytes | None:
        """
        Reads the available bytes from the stream.

        If using `flush`, the read call will first block until the flush is complete.

        Parameters
        ----------
        flush:
            If true (default), the stream will be flushed before reading.

        """
        return self.storage.read(flush=flush)  # type: ignore[no-any-return]

    def flush(self) -> None:
        """
        Flushes the recording stream and ensures that all logged messages have been encoded into the stream.

        This will block until the flush is complete.
        """
        self.storage.flush()


# ---


def get_data_recording(
    recording: RecordingStream | None = None,
) -> RecordingStream | None:
    """
    Returns the most appropriate recording to log data to, in the current context, if any.

    * If `recording` is specified, returns that one;
    * Otherwise, falls back to the currently active thread-local recording, if there is one;
    * Otherwise, falls back to the currently active global recording, if there is one;
    * Otherwise, returns None.

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    Optional[RecordingStream]
        The most appropriate recording to log data to, in the current context, if any.

    """
    result = bindings.get_data_recording(recording=recording.to_native() if recording is not None else None)
    return RecordingStream._from_native(result) if result is not None else None


def get_global_data_recording() -> RecordingStream | None:
    """
    Returns the currently active global recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active global recording, if any.

    """
    result = bindings.get_global_data_recording()
    return RecordingStream._from_native(result) if result is not None else None


def set_global_data_recording(recording: RecordingStream) -> RecordingStream | None:
    """
    Replaces the currently active global recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active global recording.

    """
    result = bindings.set_global_data_recording(recording.to_native())
    return RecordingStream._from_native(result) if result is not None else None


def get_thread_local_data_recording() -> RecordingStream | None:
    """
    Returns the currently active thread-local recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active thread-local recording, if any.

    """
    result = bindings.get_thread_local_data_recording()
    return RecordingStream._from_native(result) if result is not None else None


def set_thread_local_data_recording(recording: RecordingStream | None) -> RecordingStream | None:
    """
    Replaces the currently active thread-local recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active thread-local recording.

    """
    result = bindings.set_thread_local_data_recording(
        recording=recording.to_native() if recording is not None else None,
    )
    return RecordingStream._from_native(result) if result is not None else None


_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


def thread_local_stream(application_id: str) -> Callable[[_TFunc], _TFunc]:
    """
    Create a thread-local recording stream and use it when executing the decorated function.

    This can be helpful for decorating a function that represents a job or a task that you want to
    to produce its own isolated recording.

    Example
    -------
    ```python
    @rr.thread_local_stream("rerun_example_job")
    def job(name: str) -> None:
        rr.save(f"job_{name}.rrd")
        for i in range(5):
            time.sleep(0.2)
            rr.log("hello", rr.TextLog(f"Hello {i) from Job {name}"))

    threading.Thread(target=job, args=("A",)).start()
    threading.Thread(target=job, args=("B",)).start()
    ```
    This will produce 2 separate rrd files, each only containing the logs from the respective threads.

    Parameters
    ----------
    application_id : str
        The application ID that this recording is associated with.

    """

    def decorator(func: _TFunc) -> _TFunc:
        if inspect.isgeneratorfunction(func):  # noqa: F821

            @functools.wraps(func)
            def generator_wrapper(*args: Any, **kwargs: Any) -> Any:
                # The following code is structured to avoid leaking the recording stream
                # context when yielding from the generator.
                # See: https://github.com/rerun-io/rerun/issues/6238
                #
                # The basic idea is to only ever hold the context object open while
                # the generator is actively running, but to release it prior to yielding.
                gen = func(*args, **kwargs)
                stream = RecordingStream(application_id, recording_id=uuid.uuid4())
                try:
                    with stream:
                        value = next(gen)  # Start the generator inside the context
                    while True:
                        cont = yield value  # Yield the value, suspending the generator
                        with stream:
                            value = gen.send(cont)  # Resume the generator inside the context
                except StopIteration:
                    pass
                finally:
                    gen.close()

            return generator_wrapper  # type: ignore[return-value]
        else:

            @functools.wraps(func)
            def wrapper(*args: Any, **kwargs: Any) -> Any:
                with RecordingStream(application_id, recording_id=uuid.uuid4()):
                    gen = func(*args, **kwargs)
                    return gen

            return wrapper  # type: ignore[return-value]

    return decorator


def recording_stream_generator_ctx(func: _TFunc) -> _TFunc:
    """
    Decorator to manage recording stream context for generator functions.

    This is only necessary if you need to implement a generator which yields while holding an open
    recording stream context which it created. This decorator will ensure that the recording stream
    context is suspended and then properly resumed upon re-entering the generator.

    See: https://github.com/rerun-io/rerun/issues/6238 for context on why this is necessary.

    There are plenty of things that can go wrong when mixing context managers with generators, so
    don't use this decorator unless you're sure you need it.

    If you can plumb through `RecordingStream` objects and use those directly instead of relying on
    the context manager, that will always be more robust.

    Example
    -------
    ```python
    @rr.recording_stream.recording_stream_generator_ctx
    def my_generator(name: str) -> Iterator[None]:
        with rr.RecordingStream(name):
            rr.save(f"{name}.rrd")
            for i in range(10):
                rr.log("stream", rr.TextLog(f"{name} {i}"))
                yield i

    for i in my_generator("foo"):
        pass
    ```

    """
    if inspect.isgeneratorfunction(func):  # noqa: F821

        @functools.wraps(func)
        def generator_wrapper(*args: Any, **kwargs: Any) -> Any:
            # The following code is structured to avoid leaking the recording stream
            # context when yielding from the generator.
            # See: https://github.com/rerun-io/rerun/issues/6238
            #
            # The basic idea is to only ever hold the context object open while
            # the generator is actively running, but to release it prior to yielding.
            gen = func(*args, **kwargs)
            current_recording = None
            try:
                value = next(gen)  # Get the first generated value
                while True:
                    current_recording = active_recording_stream.get(None)

                    if current_recording is not None:
                        # TODO(jleibs): Do we need to pass something through here?
                        # Probably not, since __exit__ doesn't use those args, but
                        # keep an eye on this.
                        current_recording.__exit__(None, None, None)  # Exit our context before we yield

                    cont = yield value  # Yield the value, suspending the generator

                    if current_recording is not None:
                        current_recording.__enter__()  # Restore our context before we continue

                    value = gen.send(cont)  # Resume the generator inside the context

            except StopIteration:
                # StopIteration is raised from inside `gen.send()`. This happens after a call
                # `__enter__` and means we don't need to enter during finally, below.
                current_recording = None
            finally:
                # If we never reached the end of the iterator (StopIteration wasn't raised), then
                # we need to enter again before finally closing the generator.
                if current_recording is not None:
                    current_recording.__enter__()
                gen.close()

        return generator_wrapper  # type: ignore[return-value]
    else:
        raise ValueError("Only generator functions can be decorated with `recording_stream_generator_ctx`")
