from __future__ import annotations

from typing import Any

from rerun_bindings import (
    CatalogClientInternal,
    DataframeQueryView as DataframeQueryView,
    Dataset as Dataset,
    Entry as Entry,
    EntryId as EntryId,
    EntryKind as EntryKind,
    Table as Table,
    Task as Task,
    VectorDistanceMetric as VectorDistanceMetric,
)


class CatalogClient:
    """Client for a remote Rerun catalog server."""

    def __init__(self, address: str, token: str | None = None) -> None:
        self._raw_client = CatalogClientInternal(address, token)

    def entries(self) -> list[Entry]:
        """Returns a list of all entries in the catalog."""
        return self._raw_client.entries()

    def get_dataset(self, *, id: EntryId | str | None = None, name: str | None = None) -> Dataset:
        """Returns a dataset by its ID or name."""

        return self._raw_client.get_dataset(self._resolve_name_or_id(id, name))

    def create_dataset(self, name: str) -> Dataset:
        """Creates a new dataset with the given name."""

        return self._raw_client.create_dataset(name)

    def get_table(self, *, id: EntryId | str | None = None, name: str | None = None) -> Table:
        """Returns a table by its ID or name."""

        return self._raw_client.get_table(self._resolve_name_or_id(id, name))

    def entries_table(self) -> Table:
        """Returns a table containing all entries in the catalog."""

        return self.get_table(name="__entries")

    @property
    def ctx(self) -> Any:
        """
        Returns a DataFusion session context for querying the catalog.

        Note: the `datafusion` package is required to use this method.
        """

        return self._raw_client.ctx()

    def _resolve_name_or_id(self, id: EntryId | str | None = None, name: str | None = None) -> EntryId:
        """Helper method to resolve either ID or name. Returns the id or throw an error."""

        # TODO(ab): this screams for a `match` statement in Python 3.10+
        if id is not None and name is not None:
            raise ValueError("Only one of 'id' or 'name' must be provided.")
        elif id is not None:
            if isinstance(id, str):
                id = EntryId(id)
            return id
        elif name is not None:
            return self._raw_client._entry_id_from_entry_name(name)
        else:
            raise ValueError("Either 'id' or 'name' must be provided.")
