from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun_bindings import DatasetPolicyHandleInternal


class DatasetPolicyHandle:
    """Handle to wait until a new dataset policy applies to the registered data."""

    def __init__(self, internal: DatasetPolicyHandleInternal) -> None:
        self._internal = internal

    def wait(self, timeout_secs: int | None = None) -> None:
        """
        Block until the policy applies to the registered data.

        Parameters
        ----------
        timeout_secs
            Timeout in seconds. None waits up to one hour.

        Raises
        ------
        RuntimeError
            If applying the policy fails.
        TimeoutError
            If the timeout is reached before the policy applies.

        """
        self._internal.wait(timeout_secs)
