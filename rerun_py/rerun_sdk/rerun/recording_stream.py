from __future__ import annotations

import contextvars
import functools
import inspect
import uuid
from typing import Any, Callable, TypeVar

from rerun import bindings


# ---
# TODO(#3793): defaulting recording_id to authkey should be opt-in
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
    To send the data to a viewer or file you will likely want to call [`rerun.connect`][] or [`rerun.save`][]
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
        you call either `connect`, `show`, or `save`
    default_enabled
        Should Rerun logging be on by default?
        Can be overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.

    Returns
    -------
    RecordingStream
        A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

    Examples
    --------
    Using a recording stream object directly.
    ```python
    from uuid import uuid4
    stream = rr.new_recording("my_app", recording_id=uuid4())
    stream.connect()
    stream.log("hello", rr.TextLog("Hello world"))
    ```

    Setting up a new global recording explicitly.
    ```python
    from uuid import uuid4
    rr.new_recording("my_app", make_default=True, recording_id=uuid4())
    rr.connect()
    rr.log("hello", rr.TextLog("Hello world"))
    ```

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


active_recording_stream: contextvars.ContextVar[RecordingStream] = contextvars.ContextVar("active_recording_stream")
"""
A context variable that tracks the currently active recording stream.

Used to managed and detect interactions between generators and RecordingStream context-manager objects.
"""


class RecordingStream:
    """
    A RecordingStream is used to send data to Rerun.

    You can instantiate a RecordingStream by calling either [`rerun.init`][] (to create a global
    recording) or [`rerun.new_recording`][] (for more advanced use cases).

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
    to the wrong stream. See: https://github.com/rerun-io/rerun/issues/6238. You can work around this
    by using the [`rerun.recording_stream_generator_ctx`][] decorator.

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
        [`rerun.connect`][], [`rerun.spawn`][], …
    - Time-related functions:
        [`rerun.set_time_seconds`][], [`rerun.set_time_sequence`][], …
    - Log-related functions:
        [`rerun.log`][], [`rerun.log_components`][], …

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

    def __init__(self, inner: bindings.PyRecordingStream) -> None:
        self.inner = inner
        self._prev: RecordingStream | None = None
        self.context_token: contextvars.Token[RecordingStream] | None = None

    def __enter__(self):  # type: ignore[no-untyped-def]
        self.context_token = active_recording_stream.set(self)
        self._prev = set_thread_local_data_recording(self)
        return self

    def __exit__(self, type, value, traceback):  # type: ignore[no-untyped-def]
        current_recording = active_recording_stream.get(None)

        # Restore the context state
        if self.context_token is not None:
            active_recording_stream.reset(self.context_token)

        # Restore the recording stream state
        set_thread_local_data_recording(self._prev)  # type: ignore[arg-type]
        self._prev = None

        # Sanity check: we set this context-var on enter. If it's not still set, something weird
        # happened. The user is probably doing something sketch with generators or async code.
        if current_recording is not self:
            raise RuntimeError(
                "RecordingStream context manager exited while not active. Likely mixing context managers with generators or async code. See: `recording_stream_generator_ctx`."
            )

    # NOTE: The type is a string because we cannot reference `RecordingStream` yet at this point.
    def to_native(self: RecordingStream | None) -> bindings.PyRecordingStream | None:
        return self.inner if self is not None else None

    def __del__(self):  # type: ignore[no-untyped-def]
        recording = RecordingStream.to_native(self)
        # TODO(jleibs): I'm 98% sure this flush is redundant, but removing it requires more thorough testing.
        # However, it's definitely a problem if we are in a forked child process. The rerun SDK will still
        # detect this case and prevent a hang internally, but will do so with a warning that we should avoid.
        #
        # See: https://github.com/rerun-io/rerun/issues/6223 for context on why this is necessary.
        if recording is not None and not recording.is_forked_child():
            bindings.flush(blocking=False, recording=recording)


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

    recording = RecordingStream.to_native(recording)
    return BinaryStream(bindings.binary_stream(recording=recording))


class BinaryStream:
    """An encoded stream of bytes that can be saved as an rrd or sent to the viewer."""

    def __init__(self, storage: bindings.PyMemorySinkStorage) -> None:
        self.storage = storage

    def read(self, *, flush: bool = True) -> bytes:
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


def _patch(funcs):  # type: ignore[no-untyped-def]
    """Adds the given functions as methods to the `RecordingStream` class; injects `recording=self` in passing."""
    import functools
    import os

    # If this is a special RERUN_APP_ONLY context (launched via .spawn), we
    # can bypass everything else, which keeps us from monkey patching methods
    # that never get used.
    if os.environ.get("RERUN_APP_ONLY"):
        return

    # NOTE: Python's closures capture by reference… make sure to copy `fn` early.
    def eager_wrap(fn):  # type: ignore[no-untyped-def]
        @functools.wraps(fn)
        def wrapper(self, *args: Any, **kwargs: Any) -> Any:  # type: ignore[no-untyped-def]
            kwargs["recording"] = self
            return fn(*args, **kwargs)

        return wrapper

    for fn in funcs:
        wrapper = eager_wrap(fn)  # type: ignore[no-untyped-call]
        setattr(RecordingStream, fn.__name__, wrapper)


# ---


def is_enabled(
    recording: RecordingStream | None = None,
) -> bool:
    """
    Is this Rerun recording enabled.

    If false, all calls to the recording are ignored.

    The default can be set in [`rerun.init`][], but is otherwise `True`.

    This can be controlled with the environment variable `RERUN` (e.g. `RERUN=on` or `RERUN=off`).

    """
    return bindings.is_enabled(recording=RecordingStream.to_native(recording))  # type: ignore[no-any-return]


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
    app_id = bindings.get_application_id(recording=RecordingStream.to_native(recording))
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
    rec_id = bindings.get_recording_id(recording=RecordingStream.to_native(recording))
    return str(rec_id) if rec_id is not None else None


_patch([is_enabled, get_application_id, get_recording_id])  # type: ignore[no-untyped-call]

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
    result = bindings.get_data_recording(recording=RecordingStream.to_native(recording))
    return RecordingStream(result) if result is not None else None


def get_global_data_recording() -> RecordingStream | None:
    """
    Returns the currently active global recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active global recording, if any.

    """
    result = bindings.get_global_data_recording()
    return RecordingStream(result) if result is not None else None


def set_global_data_recording(recording: RecordingStream) -> RecordingStream | None:
    """
    Replaces the currently active global recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active global recording.

    """
    result = bindings.set_global_data_recording(RecordingStream.to_native(recording))
    return RecordingStream(result) if result is not None else None


def get_thread_local_data_recording() -> RecordingStream | None:
    """
    Returns the currently active thread-local recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active thread-local recording, if any.

    """
    result = bindings.get_thread_local_data_recording()
    return RecordingStream(result) if result is not None else None


def set_thread_local_data_recording(recording: RecordingStream) -> RecordingStream | None:
    """
    Replaces the currently active thread-local recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active thread-local recording.

    """
    result = bindings.set_thread_local_data_recording(recording=RecordingStream.to_native(recording))
    return RecordingStream(result) if result is not None else None


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
                stream = new_recording(application_id, recording_id=uuid.uuid4())
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
                with new_recording(application_id, recording_id=uuid.uuid4()):
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
        with rr.new_recording(name):
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
