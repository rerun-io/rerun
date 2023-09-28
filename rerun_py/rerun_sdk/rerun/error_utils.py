from __future__ import annotations

import functools
import inspect
import threading
import warnings
from typing import Any, Callable, TypeVar, cast

from .recording_stream import RecordingStream

__all__ = [
    "_send_warning",
]

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])

# If `True`, we raise exceptions on use error (wrong parameter types, etc.).
# If `False` we catch all errors and log a warning instead.
_strict_mode = False

_rerun_exception_ctx = threading.local()
_rerun_exception_ctx.strict_mode = None
_rerun_exception_ctx.depth = 0


def check_strict_mode() -> bool:
    """
    Strict mode enabled.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is on, such problems are instead logged as warnings.

    The default is OFF.
    """
    # If strict was set explicitly, we are in struct mode
    if _rerun_exception_ctx.strict_mode is not None:
        return _rerun_exception_ctx.strict_mode  # type: ignore[no-any-return]
    else:
        return _strict_mode


def set_strict_mode(mode: bool) -> None:
    """
    Turn strict mode on/off.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is off, such problems are instead logged as warnings.

    The default is OFF.
    """
    global _strict_mode

    _strict_mode = mode


class RerunWarning(Warning):
    """A custom warning class that we use to identify warnings that are emitted by the Rerun SDK itself."""

    pass


def _build_warning_context_string(skip_first: int) -> str:
    """Builds a string describing the user context of a warning."""
    outer_stack = inspect.stack()[skip_first:]
    return "\n".join(f'File "{frame.filename}", line {frame.lineno}, in {frame.function}' for frame in outer_stack)


def _send_warning(
    message: str,
    depth_to_user_code: int,
    recording: RecordingStream | None = None,
) -> None:
    """
    Sends a warning about the usage of the Rerun SDK.

    Used for recoverable problems.
    You can also use this for unrecoverable problems,
    or raise an exception and let the @log_decorator handle it instead.
    """
    from rerun._log import log
    from rerun.archetypes import TextLog

    if check_strict_mode():
        raise TypeError(message)

    context_descriptor = _build_warning_context_string(skip_first=depth_to_user_code + 1)

    # TODO(jleibs): Context/stack should be its component.
    log("rerun", TextLog(body=f"{message}\n{context_descriptor}", level="WARN"), recording=recording)
    warnings.warn(message, category=RerunWarning, stacklevel=depth_to_user_code + 1)


def catch_and_log_exceptions(func: _TFunc) -> _TFunc:
    """
    A decorator we add to any function we want to catch exceptions if we're not in strict mode.

    This function checks for a strict kwarg and uses it to override the global strict mode
    if provided. Additionally it tracks the depth of the call stack to the user code -- the
    highest point in the stack where the user called a decorated function.

    This is important in order not to crash the users application
    just because they misused the Rerun API (or because we have a bug!).
    """

    @functools.wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        try:
            original_strict = _rerun_exception_ctx.strict_mode
            _rerun_exception_ctx.depth += 2
            if "strict" in kwargs:
                _rerun_exception_ctx.strict_mode = kwargs["strict"]

            if check_strict_mode():
                # Pass on any exceptions to the caller
                return func(*args, **kwargs)
            else:
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    warning = f"{type(e).__name__}: {e}"

                    _send_warning(warning, depth_to_user_code=_rerun_exception_ctx.depth)
        finally:
            _rerun_exception_ctx.strict_mode = original_strict
            _rerun_exception_ctx.depth -= 2

    return cast(_TFunc, wrapper)
