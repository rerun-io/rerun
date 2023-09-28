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


class catch_and_log_exceptions:
    """
    A decorator we add to any function we want to catch exceptions if we're not in strict mode.

    This decorator checks for a strict kwarg and uses it to override the global strict mode
    if provided. Additionally it tracks the depth of the call stack to the user code -- the
    highest point in the stack where the user called a decorated function.

    This is important in order not to crash the users application
    just because they misused the Rerun API (or because we have a bug!).
    """

    def __init__(self, bare_context: bool = True) -> None:
        self.bare_context = bare_context

    def __enter__(self) -> catch_and_log_exceptions:
        # Track the original strict_mode setting in case it's being
        # overridden locally in this stack
        self.original_strict = _rerun_exception_ctx.strict_mode
        self.added_depth = 0

        # Functions add a depth of 2
        # Bare context helpers don't add a depth
        if not self.bare_context:
            self.added_depth = 2
            _rerun_exception_ctx.depth += self.added_depth

        return self

    @classmethod
    def __call__(cls, func: _TFunc) -> _TFunc:
        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            with cls(bare_context=False):
                if "strict" in kwargs:
                    _rerun_exception_ctx.strict_mode = kwargs["strict"]

                func(*args, **kwargs)

        return cast(_TFunc, wrapper)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        try:
            if exc_type is not None and not check_strict_mode():
                warning = f"{exc_type.__name__}: {exc_val}"

                # If the raise comes directly from a bare context, we need
                # to add 1 extra layer of depth.
                if self.bare_context:
                    extra_depth = 2
                else:
                    extra_depth = 1
                _send_warning(warning, depth_to_user_code=_rerun_exception_ctx.depth + extra_depth)
                return True
            else:
                return False
        finally:
            # Return the local context to the prior value
            _rerun_exception_ctx.strict_mode = self.original_strict
            _rerun_exception_ctx.depth -= self.added_depth
        return False
