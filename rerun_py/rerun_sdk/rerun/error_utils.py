from __future__ import annotations

import functools
import inspect
import os
import threading
import warnings
from types import TracebackType
from typing import Any, Callable, TypeVar, cast

from .recording_stream import RecordingStream

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


def default_strict_mode() -> bool:
    if "RERUN_STRICT" in os.environ:
        var = os.environ["RERUN_STRICT"].lower()
        if var in ("0", "false", "off", "no"):
            return False
        elif var in ("1", "true", "on", "yes"):
            return True
        else:
            print(f"Expected RERUN_STRICT to be one of 0/1 false/true off/on no/yes, found {var}")
            return _strict_mode
    else:
        return False


# If `True`, we raise exceptions on use error (wrong parameter types, etc.).
# If `False` we catch all errors and log a warning instead.
_strict_mode = default_strict_mode()

_rerun_exception_ctx = threading.local()


def strict_mode() -> bool:
    """
    Strict mode enabled.

    In strict mode, incorrect use of the Rerun API (wrong parameter types etc.)
    will result in exception being raised.
    When strict mode is on, such problems are instead logged as warnings.

    The default is controlled with the `RERUN_STRICT` environment variable,
    or `False` if it is not set.
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

    The default is controlled with the `RERUN_STRICT` environment variable,
    or `False` if it is not set.
    """
    global _strict_mode

    _strict_mode = mode


class RerunWarning(Warning):
    """A custom warning class that we use to identify warnings that are emitted by the Rerun SDK itself."""


def _build_warning_context_string(skip_first: int) -> str:
    """Builds a string describing the user context of a warning."""
    outer_stack = inspect.stack()[skip_first:]
    return "\n".join(f'File "{frame.filename}", line {frame.lineno}, in {frame.function}' for frame in outer_stack)


def _send_warning_or_raise(
    message: str,
    depth_to_user_code: int = 1,
    *,
    recording: RecordingStream | None = None,
    exception_type: type[Exception] = ValueError,
    warning_type: type[Warning] = RerunWarning,
) -> None:
    """
    Sends a warning about the usage of the Rerun SDK.

    Note: in strict mode this will instead raise the specified exception type
    (defaults to ValueError).

    This will both send a message to the Rerun viewer and log a warning using
    `warning.warn` with a custom `RerunWarning` class.

    This should generally be used for recoverable problems where you want execution
    to continue in the local scope.

    For unrecoverable problems where execution cannot otherwise continue, you should
    instead raise an exception and let the `catch_and_log_exceptions` handle it.
    """
    from rerun._log import log
    from rerun.archetypes import TextLog

    if strict_mode():
        raise exception_type(message)

    # Send the warning to the user first
    warnings.warn(message, category=warning_type, stacklevel=depth_to_user_code + 1)

    # Logging the warning to Rerun is a complex operation could produce another warning. Avoid recursion.
    if not getattr(_rerun_exception_ctx, "sending_warning", False):
        _rerun_exception_ctx.sending_warning = True

        # TODO(jleibs): Context/stack should be its own component.
        context_descriptor = _build_warning_context_string(skip_first=depth_to_user_code + 1)

        log("__warnings", TextLog(text=f"{message}\n{context_descriptor}", level="WARN"), recording=recording)  # NOLINT
        _rerun_exception_ctx.sending_warning = False
    else:
        warnings.warn(
            "Encountered Error while sending warning",
            category=warning_type,
            stacklevel=depth_to_user_code + 1,
        )


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

    Parameters
    ----------
    context:
        A string describing the context of the exception.
        If not provided, the function name will be used.
    depth_to_user_code:
        The number of frames to skip when building the warning context.
        This should be the number of frames between the user code and the
        context manager.
    exception_return_value:
        If an exception is caught, this value will be returned instead of
        the function's return value.

    """

    def __init__(
        self,
        context: str | None = None,
        depth_to_user_code: int = 1,
        exception_return_value: Any = None,
        strict: bool | None = None,
    ) -> None:
        self.depth_to_user_code = depth_to_user_code
        self.context = context
        self.exception_return_value = exception_return_value
        self.strict = strict

    def __enter__(self) -> catch_and_log_exceptions:
        # Track the original strict_mode setting in case it's being
        # overridden locally in this stack
        self.original_strict = getattr(_rerun_exception_ctx, "strict_mode", None)
        if self.strict is not None:
            _rerun_exception_ctx.strict_mode = self.strict
        if getattr(_rerun_exception_ctx, "depth", None) is None:
            _rerun_exception_ctx.depth = 1
        else:
            _rerun_exception_ctx.depth += 1

        return self

    def __call__(self, func: _TFunc) -> _TFunc:
        if self.context is None:
            self.context = func.__qualname__

        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            with self:
                if "strict" in kwargs:
                    _rerun_exception_ctx.strict_mode = kwargs["strict"]
                return func(*args, **kwargs)

            # If there was an exception before returning from func
            return self.exception_return_value

        return cast(_TFunc, wrapper)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        try:
            # Exceptions inheriting from `BaseException` others than via `Exception` are "exiting", and should be pass
            # through. This includes `KeyboardInterrupt` and `SystemExit`.
            if exc_type is not None and issubclass(exc_type, Exception) and not strict_mode():
                if getattr(_rerun_exception_ctx, "pending_warnings", None) is None:
                    _rerun_exception_ctx.pending_warnings = []

                context = f"{self.context}: " if self.context is not None else ""

                warning_message = f"{context}{exc_type.__name__}({exc_val})"

                _rerun_exception_ctx.pending_warnings.append(warning_message)
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
                        _send_warning_or_raise(warning, depth_to_user_code=self.depth_to_user_code + 2)

            # If we're back to the top of the stack, send out the pending warnings

            # Return the local context to the prior value
            _rerun_exception_ctx.strict_mode = self.original_strict


T = TypeVar("T", bound=Callable[..., Any])


def deprecated_param(name: str, *, use_instead: str | None = None, since: str | None = None) -> Callable[[T], T]:
    """
    Marks a parameter as deprecated.

    @deprecated_param(foo, use_instead="bar", since="0.23")
    def foo(foo: int | None = None, bar: str | None = None) -> None:
        ...
    """

    def decorator(func: T) -> T:
        sig = inspect.signature(func)

        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            if name in kwargs:
                message = f"The parameter '{name}' in function '{func.__name__}' is deprecated"
                if since:
                    message += f" since version {since}"
                if use_instead:
                    message += f", use {use_instead} instead"
                _send_warning_or_raise(message, depth_to_user_code=2, warning_type=DeprecationWarning)
            return func(*args, **kwargs)

        # Preserve the original signature
        wrapper.__signature__ = sig  # type: ignore[attr-defined]

        return cast(T, wrapper)

    return decorator
