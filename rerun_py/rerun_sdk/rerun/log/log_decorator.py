import functools
import logging
import traceback
from typing import Any, Callable, TypeVar, cast

from rerun import bindings
from rerun.log.text import LogLevel, log_text_entry

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


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
        if not bindings.is_enabled():
            return

        strict = False  # TODO: settable by the user

        if strict:
            # Pass on any exceptions to the caller
            return func(*args, **kwargs)
        else:
            try:
                return func(*args, **kwargs)
            except Exception as e:
                warning = "".join(traceback.format_exception(e.__class__, e, e.__traceback__))
                log_text_entry("rerun", warning, level=LogLevel.WARN)
                logging.warning(f"Ignoring rerun log call: {warning}")

    return cast(_TFunc, wrapper)
