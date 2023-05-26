import inspect
import logging

import depthai_viewer
from depthai_viewer.log.text_internal import LogLevel, log_text_entry_internal

__all__ = [
    "_send_warning",
]


def _build_warning_context_string(skip_first: int) -> str:
    """Builds a string describing the user context of a warning."""
    outer_stack = inspect.stack()[skip_first:]
    return "\n".join(f'File "{frame.filename}", line {frame.lineno}, in {frame.function}' for frame in outer_stack)


def _send_warning(message: str, depth_to_user_code: int) -> None:
    """
    Sends a warning about the usage of the Rerun SDK.

    Used for recoverable problems.
    You can also use this for unrecoverable problems,
    or raise an exception and let the @log_decorator handle it instead.
    """

    if depthai_viewer.strict_mode():
        raise TypeError(message)

    context_descriptor = _build_warning_context_string(skip_first=depth_to_user_code + 2)
    warning = f"{message}\n{context_descriptor}"
    log_text_entry_internal("rerun", warning, level=LogLevel.WARN)
    logging.warning(warning)
