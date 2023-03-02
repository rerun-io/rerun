import functools
from typing import Any, Callable, TypeVar, cast

from rerun import bindings

_TFunc = TypeVar("_TFunc", bound=Callable[..., Any])


def log_decorator(func: _TFunc) -> _TFunc:
    """
    A decorator we add to all our logging function.

    It early-outs if logging is disabled.
    """

    @functools.wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        if bindings.is_enabled():
            return func(*args, **kwargs)

    return cast(_TFunc, wrapper)
