from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import (
    CatalogClientInternal,
    DataframeQueryView as DataframeQueryView,
    DatasetEntry as DatasetEntry,
    Entry as Entry,
    EntryId as EntryId,
    EntryKind as EntryKind,
    TableEntry as TableEntry,
    Task as Task,
    VectorDistanceMetric as VectorDistanceMetric,
)

if TYPE_CHECKING:
    import datafusion


class CatalogClient:
    """
    Client for a remote Rerun catalog server.

    Note: the `datafusion` package is required to use this client. Initialization will fail with an error if the package
    is not installed.
    """

    def __init__(self, address: str, token: str | None = None) -> None:
        from importlib.util import find_spec

        if find_spec("datafusion") is None:
            raise ImportError(
                "The 'datafusion' package is required to use `CatalogClient`. "
                "You can install it with `pip install datafusion`."
            )

        self._raw_client = CatalogClientInternal(address, token)

    def __repr__(self) -> str:
        return self._raw_client.__repr__()

    def all_entries(self) -> list[Entry]:
        """Returns a list of all entries in the catalog."""
        return self._raw_client.all_entries()

    def dataset_entries(self) -> list[DatasetEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return self._raw_client.dataset_entries()

    def table_entries(self) -> list[TableEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return self._raw_client.table_entries()

    # ---

    def entry_names(self) -> list[str]:
        """Returns a list of all entry names in the catalog."""
        return self._raw_client.entry_names()

    def dataset_names(self) -> list[str]:
        """Returns a list of all dataset names in the catalog."""
        return self._raw_client.dataset_names()

    def table_names(self) -> list[str]:
        """Returns a list of all table names in the catalog."""
        return self._raw_client.table_names()

    # ---

    def entries(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all entries in the catalog."""
        return self.get_table(name="__entries")

    def datasets(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all dataset entries in the catalog."""
        from datafusion import col

        return self.entries().filter(col("entry_kind") == int(EntryKind.DATASET)).drop("entry_kind")

    def tables(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all table entries in the catalog."""
        from datafusion import col

        return self.entries().filter(col("entry_kind") == int(EntryKind.TABLE)).drop("entry_kind")

    # ---

    def get_dataset_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        """Returns a dataset by its ID or name."""

        return self._raw_client.get_dataset_entry(self._resolve_name_or_id(id, name))

    def get_table_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> TableEntry:
        """Returns a table by its ID or name."""

        return self._raw_client.get_table_entry(self._resolve_name_or_id(id, name))

    # ---

    def get_dataset(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        """
        Returns a dataset by its ID or name.

        Note: This is currently an alias for `get_dataset_entry`. In the future, it will return a data-oriented dataset
        object instead.
        """
        return self.get_dataset_entry(id=id, name=name)

    def get_table(self, *, id: EntryId | str | None = None, name: str | None = None) -> datafusion.DataFrame:
        """Returns a table by its ID or name."""
        return self.get_table_entry(id=id, name=name).df()

    # ---

    def create_dataset(self, name: str) -> DatasetEntry:
        """Creates a new dataset with the given name."""
        return self._raw_client.create_dataset(name)

    def register_table(self, name: str, url: str) -> TableEntry:
        """
        Registers a foreign Lance table (identified by its URL) as a new table entry with the given name.

        Parameters
        ----------
        name
            The name of the table entry to create. It must be unique within all entries in the catalog. An exception
            will be raised if an entry with the same name already exists.

        url
            The URL of the Lance table to register.

        """
        return self._raw_client.register_table(name, url)

    @property
    def ctx(self) -> datafusion.SessionContext:
        """Returns a DataFusion session context for querying the catalog."""

        return self._raw_client.ctx()

    # ---

    def _resolve_name_or_id(self, id: EntryId | str | None = None, name: str | None = None) -> EntryId:
        """Helper method to resolve either ID or name. Returns the id or throw an error."""

        # TODO(ab): this screams for a `match` statement in Python 3.10+
        if id is not None and name is not None:
            raise ValueError("Only one of 'id' or 'name' must be provided.")
        elif id is not None:
            if isinstance(id, EntryId):
                return id
            else:
                return EntryId(id)
        elif name is not None:
            return self._raw_client._entry_id_from_entry_name(name)
        else:
            raise ValueError("Either 'id' or 'name' must be provided.")
