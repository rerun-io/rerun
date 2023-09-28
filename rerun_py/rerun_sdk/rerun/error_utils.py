from __future__ import annotations

import functools
import inspect
import threading
import warnings
from types import TracebackType
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


def check_strict_mode() -> bool:
    """
    Strict mode enabled.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is on, such problems are instead logged as warnings.

    The default is OFF.
    """
    # If strict was set explicitly, we are in struct mode
    if getattr(_rerun_exception_ctx, "strict_mode", None) is not None:
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

    # TODO(jleibs): Context/stack should be its own component.
    log("rerun", TextLog(body=f"{message}\n{context_descriptor}", level="WARN"), recording=recording)
    warnings.warn(message, category=RerunWarning, stacklevel=depth_to_user_code + 1)


class catch_and_log_exceptions:
    """
    A hybrid decorator / context-manager.

    We can add this to any function or scope where we want to catch and log
    exceptions.

    Warnings are attached to a thread-local context, and are sent out when
    we leave the outer-most context object. This gives us a warning that
    points to the user call-site rather than somewhere buried in Rerun code.

    For functions, this decorator checks for a strict kwarg and uses it to
    override the global strict mode if provided.
    """

    def __init__(self, context: str = "", depth_to_user_code: int = 0) -> None:
        self.depth_to_user_code = depth_to_user_code
        self.context = context

    def __enter__(self) -> catch_and_log_exceptions:
        # Track the original strict_mode setting in case it's being
        # overridden locally in this stack
        self.original_strict = getattr(_rerun_exception_ctx, "strict_mode", None)
        if getattr(_rerun_exception_ctx, "depth", None) is None:
            _rerun_exception_ctx.depth = 1
        else:
            _rerun_exception_ctx.depth += 1

        return self

    def __call__(self, func: _TFunc) -> _TFunc:
        self.depth_to_user_code += 1

        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            with self:
                if "strict" in kwargs:
                    _rerun_exception_ctx.strict_mode = kwargs["strict"]
                return func(*args, **kwargs)

        return cast(_TFunc, wrapper)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        try:
            if exc_type is not None and not check_strict_mode():
                if getattr(_rerun_exception_ctx, "pending_warnings", None) is None:
                    _rerun_exception_ctx.pending_warnings = []
                _rerun_exception_ctx.pending_warnings.append(f"{self.context or exc_type.__name__}: {exc_val}")
                return True
            else:
                return False
        finally:
            if getattr(_rerun_exception_ctx, "depth", None) is not None:
                _rerun_exception_ctx.depth -= 1
                if _rerun_exception_ctx.depth == 0:
                    pending_warnings = getattr(_rerun_exception_ctx, "pending_warnings", [])
                    _rerun_exception_ctx.pending_warnings = []
                    _rerun_exception_ctx.depth = None

                    for warning in pending_warnings:
                        _send_warning(warning, depth_to_user_code=self.depth_to_user_code + 2)

            # If we're back to the top of the stack, send out the pending warnings

            # Return the local context to the prior value
            _rerun_exception_ctx.strict_mode = self.original_strict
