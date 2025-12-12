from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rerun_bindings import RegistrationHandleInternal


@dataclass(frozen=True)
class SegmentRegistrationResult:
    """Result of a completed segment registration."""

    uri: str
    """The source URI that was registered."""

    segment_id: str | None
    """The resulting segment ID. May be `None` if registration failed."""

    error: str | None
    """Error message if registration failed, or `None` if successful."""

    @property
    def is_success(self) -> bool:
        """Returns True if the registration was successful."""
        return self.error is None

    @property
    def is_error(self) -> bool:
        """Returns True if the registration failed."""
        return self.error is not None


@dataclass(frozen=True)
class RegistrationResult:
    """Result of a completed registration batch."""

    segment_ids: list[str]
    """The ids of the registered segments."""


class RegistrationHandle:
    """Handle to track and wait on segment registration tasks."""

    def __init__(self, internal: RegistrationHandleInternal) -> None:
        self._internal = internal

    def iter_results(self, timeout_secs: int | None = None) -> Iterator[SegmentRegistrationResult]:
        """
        Stream completed registrations as they finish.

        Uses the server's streaming API to yield results as tasks complete.
        Each result is yielded exactly once when its task completes (success or error).

        Parameters
        ----------
        timeout_secs
            Timeout in seconds. None for blocking. Note that using None doesn't guarantee that a TimeoutError will
            never be eventually raised for long-running tasks. Setting a timeout and polling is recommended for
            monitoring very large registration batches.

        Yields
        ------
        SegmentRegistrationResult
            The result of each completed registration.

        Raises
        ------
        TimeoutError
            If the timeout is reached before all tasks complete.

        """
        for uri, segment_id, error in self._internal.iter_results(timeout_secs):
            yield SegmentRegistrationResult(
                uri=uri,
                segment_id=segment_id,
                error=error,
            )

    def wait(self, timeout_secs: int | None = None) -> RegistrationResult:
        """
        Block until all registrations complete.

        Parameters
        ----------
        timeout_secs
            Timeout in seconds. None for blocking. Note that using None doesn't guarantee that a TimeoutError will
            never be eventually raised for long-running tasks. Setting a timeout and polling is recommended for
            monitoring very large registration batches.

        Returns
        -------
        RegistrationResult
            The result containing the list of segment IDs in registration order.

        Raises
        ------
        ValueError
            If any registration fails.
        TimeoutError
            If the timeout is reached before all tasks complete.

        """
        segment_ids = self._internal.wait(timeout_secs)
        return RegistrationResult(segment_ids=segment_ids)
