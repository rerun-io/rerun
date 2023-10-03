from __future__ import annotations

import functools
import traceback
import warnings
from typing import Any, Callable, TypeVar, cast

from rerun import bindings
from rerun._log import log
from rerun.archetypes import TextLog
from rerun.recording_stream import RecordingStream

from ..error_utils import strict_mode

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


class RerunWarning(Warning):
    """A custom warning class that we use to identify warnings that are emitted by the Rerun SDK itself."""

    pass


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
                f"Rerun is disabled - {func.__name__}() call ignored. You must call rerun.init before using log APIs.",
                category=RerunWarning,
                stacklevel=2,
            )
            return

        if strict_mode():
            # Pass on any exceptions to the caller
            return func(*args, **kwargs)
        else:
            try:
                return func(*args, **kwargs)
            except Exception as e:
                warning = "".join(traceback.format_exception(e.__class__, e, e.__traceback__))
                log("rerun", TextLog(warning, level="WARN"), recording=recording)
                warnings.warn(f"Ignoring rerun log call: {warning}", category=RerunWarning, stacklevel=2)

    return cast(_TFunc, wrapper)
