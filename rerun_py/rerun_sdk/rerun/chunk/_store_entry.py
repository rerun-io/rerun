from __future__ import annotations

from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from rerun_bindings import StoreEntryInternal


class StoreEntry:
    """Describes a store found in an RRD file."""

    _internal: StoreEntryInternal

    def __init__(self, internal: StoreEntryInternal) -> None:
        self._internal = internal

    @property
    def kind(self) -> Literal["recording", "blueprint"]:
        """Store kind: `"recording"` or `"blueprint"`."""
        return self._internal.kind

    @property
    def application_id(self) -> str:
        """The application ID of the store."""
        return self._internal.application_id

    @property
    def recording_id(self) -> str:
        """The recording ID of the store."""
        return self._internal.recording_id

    def __repr__(self) -> str:
        return (
            f"StoreEntry(kind={self.kind!r}, "
            f"application_id={self.application_id!r}, "
            f"recording_id={self.recording_id!r})"
        )

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, StoreEntry):
            return NotImplemented
        return self._internal == other._internal

    def __hash__(self) -> int:
        return self._internal.__hash__()
