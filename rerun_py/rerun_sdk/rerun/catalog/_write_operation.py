from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun_bindings import WriteOperationInternal


class WriteStatus(StrEnum):
    """The current phase or terminal outcome of a write."""

    FAILED = "failed"
    REGISTERING = "registering"
    SUCCEEDED = "succeeded"
    UPLOADING = "uploading"


@dataclass(frozen=True)
class WriteResult:
    """Result of a completed upload and registration."""

    segment_ids: list[str]
    """The IDs of the registered segments."""

    storage_url: str
    """The uploaded RRD's storage URL."""


class WriteOperation:
    """A finite upload followed by registration. Call `wait()` to query completion or raise errors."""

    def __init__(self, internal: WriteOperationInternal) -> None:
        self._internal = internal

    def status(self) -> WriteStatus:
        """Return the last observed status. Call `wait()` to confirm registration success."""
        return WriteStatus(self._internal.status())

    def wait(self, timeout_secs: int | None = None) -> WriteResult:
        """
        Wait for the upload to finish, then for registration, or raise the write failure.

        `timeout_secs` bounds only the wait for server-side registration.
        The upload has no deadline: its duration depends on the payload size and the network.
        Use `status()` to see which phase the write is in.

        A registration timeout or query error does not cancel the write; call `wait()` again to query completion.
        A failed registration is final: `status()` reports `FAILED`, and `wait()` raises the same error again.
        Interrupting this call (Ctrl-C) stops waiting but does not cancel the upload or registration.

        Parameters
        ----------
        timeout_secs
            Registration timeout in seconds. None for blocking. Note that using None doesn't guarantee that a
            TimeoutError will never be eventually raised for long-running tasks. Setting a timeout and polling
            is recommended for monitoring very large registration batches.

        """
        segment_ids, storage_url = self._internal.wait(timeout_secs)
        return WriteResult(segment_ids=segment_ids, storage_url=storage_url)
