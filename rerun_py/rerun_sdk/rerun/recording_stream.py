from typing import Optional

from rerun import bindings

# ---


class RecordingStream:
    """
    A RecordingStream is used to send data to Rerun.

    You can instantiate a RecordingStream by calling either [`rr.init`][] (to create a global
    recording) or [`rr.new_recording`][] (for more advanced use cases).

    A RecordingStream can safely be copied and to other threads.

    Todo:
    ----
        - batching vars
        - global, thread-local
        - explain how this works, list synonyms (rr.get_recording_id(), rec.get_recording_id, etc...)
        - won't have docs, LSP will suck, etc -> suggest using `help()`
    """

    def __init__(self, inner: bindings.PyRecordingStream) -> None:
        self.inner = inner
        self._prev: Optional["RecordingStream"] = None

    def __enter__(self):  # type: ignore[no-untyped-def]
        self._prev = set_thread_local_data_recording(self)
        return self

    def __exit__(self, type, value, traceback):  # type: ignore[no-untyped-def]
        self._prev = set_thread_local_data_recording(self._prev)  # type: ignore[arg-type]

    # NOTE: The type is a string because we cannot reference `RecordingStream` yet at this point.
    def to_native(self: Optional["RecordingStream"]) -> Optional[bindings.PyRecordingStream]:
        return self.inner if self is not None else None

    def __del__(self):  # type: ignore[no-untyped-def]
        recording = RecordingStream.to_native(self)
        bindings.flush(blocking=False, recording=recording)


def _patch(funcs):  # type: ignore[no-untyped-def]
    """Adds the given functions as methods to the `RecordingStream` class; injects `recording=self` in passing."""
    import functools
    import os
    from typing import Any

    # If this is a special RERUN_APP_ONLY context (launched via .spawn), we
    # can bypass everything else, which keeps us from monkey patching methods
    # that never get used.
    if os.environ.get("RERUN_APP_ONLY"):
        return

    # NOTE: Python's closures capture by reference... make sure to copy `fn` early.
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
    recording: Optional[RecordingStream] = None,
) -> bool:
    """
    Is this Rerun recording enabled.

    If false, all calls to the recording are ignored.

    The default can be set in [`rerun.init`][], but is otherwise `True`.

    This can be controlled with the environment variable `RERUN` (e.g. `RERUN=on` or `RERUN=off`).

    """
    return bindings.is_enabled(recording=RecordingStream.to_native(recording))  # type: ignore[no-any-return]


def get_application_id(
    recording: Optional[RecordingStream] = None,
) -> Optional[str]:
    """
    Get the application ID that this recording is associated with, if any.

    Parameters
    ----------
    recording:
        Specifies the [`rerun.recording_stream.RecordingStream`][] to use.
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
    recording: Optional[RecordingStream] = None,
) -> Optional[str]:
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
        Specifies the [`rerun.recording_stream.RecordingStream`][] to use.
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
    recording: Optional[RecordingStream] = None,
) -> Optional[RecordingStream]:
    """
    Returns the most appropriate recording to log data to, in the current context, if any.

    * If `recording` is specified, returns that one;
    * Otherwise, falls back to the currently active thread-local recording, if there is one;
    * Otherwise, falls back to the currently active global recording, if there is one;
    * Otherwise, returns None.

    Parameters
    ----------
    recording:
        Specifies the [`rerun.recording_stream.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    Optional[RecordingStream]
        The most appropriate recording to log data to, in the current context, if any.
    """
    result = bindings.get_data_recording(recording=recording)
    return RecordingStream(result) if result is not None else None


def get_global_data_recording() -> Optional[RecordingStream]:
    """
    Returns the currently active global recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active global recording, if any.
    """
    result = bindings.get_global_data_recording()
    return RecordingStream(result) if result is not None else None


def set_global_data_recording(recording: RecordingStream) -> Optional[RecordingStream]:
    """
    Replaces the currently active global recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active global recording.
    """
    result = bindings.set_global_data_recording(RecordingStream.to_native(recording))
    return RecordingStream(result) if result is not None else None


def get_thread_local_data_recording() -> Optional[RecordingStream]:
    """
    Returns the currently active thread-local recording, if any.

    Returns
    -------
    Optional[RecordingStream]
        The currently active thread-local recording, if any.
    """
    result = bindings.get_thread_local_data_recording()
    return RecordingStream(result) if result is not None else None


def set_thread_local_data_recording(recording: RecordingStream) -> Optional[RecordingStream]:
    """
    Replaces the currently active thread-local recording with the specified one.

    Parameters
    ----------
    recording:
        The newly active thread-local recording.
    """
    result = bindings.set_thread_local_data_recording(recording=RecordingStream.to_native(recording))
    return RecordingStream(result) if result is not None else None
