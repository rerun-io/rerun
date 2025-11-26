from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Generic, TypeVar

# TODO: rename
from rerun_bindings import DatasetEntry as DatasetEntryInternal, TableEntry as TableEntryInternal

from . import EntryId, EntryKind

if TYPE_CHECKING:
    from datetime import datetime

    from . import CatalogClient

InternalT = TypeVar("InternalT", DatasetEntryInternal, TableEntryInternal)


class Entry(ABC, Generic[InternalT]):
    """An entry in the catalog."""

    __slots__ = ("_internal",)

    def __init__(self, inner: InternalT) -> None:
        self._internal = inner

    @property
    def id(self) -> EntryId:
        """The entry's id."""
        return self._internal.id

    @property
    def name(self) -> str:
        """The entry's name."""
        return self._internal.name

    # TODO(RR-2938): this should return `CatalogClient`
    @property
    def catalog(self) -> CatalogClient:
        """The catalog client that this entry belongs to."""

        from . import CatalogClient

        return CatalogClient._from_internal(self._internal.catalog())

    @property
    def kind(self) -> EntryKind:
        """The entry's kind."""

        return self._internal.kind

    @property
    def created_at(self) -> datetime:
        """The entry's creation date and time."""

        return self._internal.created_at

    @property
    def updated_at(self) -> datetime:
        """The entry's last updated date and time."""

        return self._internal.updated_at

    def delete(self) -> None:
        """Delete this entry from the catalog."""

        self._internal.delete()

    def update(self, *, name: str | None = None) -> None:
        """
        Update this entry's properties.

        Parameters
        ----------
        name : str | None
            New name for the entry

        """

        self._internal.update(name=name)
