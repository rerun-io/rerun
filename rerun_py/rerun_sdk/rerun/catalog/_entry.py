from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Generic, TypeVar

# TODO: rename
from rerun_bindings import DatasetEntry as DatasetEntryInternal, TableEntry as TableEntryInternal

if TYPE_CHECKING:
    from datetime import datetime

    from . import CatalogClient, EntryId, EntryKind

InternalT = TypeVar("InternalT", DatasetEntryInternal, TableEntryInternal)


class Entry(ABC, Generic[InternalT]):
    """An entry in the catalog."""

    __slots__ = ("_internal",)

    def __init__(self, inner: InternalT) -> None:
        self._internal = inner

    def __repr__(self) -> str:
        return f"Entry({self.kind}, '{self.name}'"

    @property
    def id(self) -> EntryId:
        """The entry's id."""
        return self._internal.entry_details().id

    @property
    def name(self) -> str:
        """The entry's name."""
        return self._internal.entry_details().name

    @property
    def kind(self) -> EntryKind:
        """The entry's kind."""

        return self._internal.entry_details().kind

    @property
    def created_at(self) -> datetime:
        """The entry's creation date and time."""

        return self._internal.entry_details().created_at

    @property
    def updated_at(self) -> datetime:
        """The entry's last updated date and time."""

        return self._internal.entry_details().updated_at

    @property
    def catalog(self) -> CatalogClient:
        """The catalog client that this entry belongs to."""

        from . import CatalogClient

        return CatalogClient._from_internal(self._internal.catalog())

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
