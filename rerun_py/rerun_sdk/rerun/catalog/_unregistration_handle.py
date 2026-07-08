from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun_bindings import UnregistrationHandleInternal


class UnregistrationHandle:
    """Handle to track and wait on segment unregistration tasks."""

    def __init__(self, internal: UnregistrationHandleInternal) -> None:
        self._internal = internal

    def wait(self, timeout_secs: int | None = None) -> None:
        """
        Block until the unregistriation completes.

        Parameters
        ----------
        timeout_secs
            Timeout in seconds. None for blocking. Note that using None doesn't guarantee that a TimeoutError will
            never be eventually raised for long-running tasks.

        Raises
        ------
        ValueError
            If the uregistration fails.
        TimeoutError
            If the timeout is reached before all tasks complete.

        """
        self._internal.wait(timeout_secs)

    def cancel(self) -> None:
        """
        Cancel unrregistration. If the unregistration is already done, this is a noop.

        Raises
        ------
        ValueError
            If the cancellation fails.

        """

        self._internal.cancel()
