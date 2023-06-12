from __future__ import annotations

import functools
import logging
import traceback
import warnings
from typing import Any, Callable, TypeVar, cast

import rerun
from rerun import bindings
from rerun.log.text_internal import LogLevel, log_text_entry_internal
from rerun.recording_stream import RecordingStream

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])

# Default formatting for the `warnings` package is... non optimal.
warnings.formatwarning = lambda msg, *args, **kwargs: f"WARNING:rerun:{msg}\n"


def log_decorator(func: _TFunc) -> _TFunc:
    """
    A decorator we add to all our logging function.

    It does two things:
    * It early-outs if logging is disabled
    * It catches any exceptions and logs them

    The latter is important in order not to crash the users application
    just because they misused the Rerun API (or because we have a bug!).
    """

    @functools.wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        recording = RecordingStream.to_native(kwargs.get("recording"))
        if not bindings.is_enabled(recording):
            # NOTE: use `warnings` which handles runtime deduplication.
            warnings.warn(
                f"Rerun is disabled - {func.__name__}() call ignored. You must call rerun.init before using log APIs."
            )
            return

        if rerun.strict_mode():
            # Pass on any exceptions to the caller
            return func(*args, **kwargs)
        else:
            try:
                return func(*args, **kwargs)
            except Exception as e:
                warning = "".join(traceback.format_exception(e.__class__, e, e.__traceback__))
                log_text_entry_internal("rerun", warning, level=LogLevel.WARN, recording=recording)
                logging.warning(f"Ignoring rerun log call: {warning}")

    return cast(_TFunc, wrapper)
