from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

from typing_extensions import deprecated

from rerun_bindings import (
    CatalogClientInternal,
)

from ..error_utils import RerunIncompatibleDependencyVersionError, RerunMissingDependencyError
from . import EntryId, TableInsertMode

if TYPE_CHECKING:
    from collections.abc import Sequence

    import datafusion
    import pyarrow as pa
    from pyarrow import RecordBatch, RecordBatchReader

    from . import DatasetEntry, TableEntry


# Known FFI compatible releases of Datafusion.
DATAFUSION_MAJOR_VERSION_COMPATIBILITY_SETS = [
    {49, 50},
]


def _are_datafusion_versions_compatible(v1: int, v2: int) -> bool:
    """
    Determine compatibility between two DataFusion versions.

    In some rare cases, we may need to have a mismatch, e.g. in some deployed Rerun Cloud docker images. So we have a
    carefully crafted compatibility allowlist for known-to-be-ffi-compatible DataFusion releases.
    """

    if v1 == v2:
        return True

    for compat_set in DATAFUSION_MAJOR_VERSION_COMPATIBILITY_SETS:
        if v1 in compat_set and v2 in compat_set:
            return True

    return False


def _compatible_datafusion_version(version: int) -> list[int]:
    """Returns a list of compatible DataFusion versions for the given version."""

    for compat_set in DATAFUSION_MAJOR_VERSION_COMPATIBILITY_SETS:
        if version in compat_set:
            return sorted(compat_set)
    return [version]


class CatalogClient:
    """
    Client for a remote Rerun catalog server.

    Note: the `datafusion` package is required to use this client. Initialization will fail with an error if the package
    is not installed.
    """

    __slots__ = ("_internal",)

    def __init__(self, address: str, token: str | None = None) -> None:
        from importlib.metadata import version
        from importlib.util import find_spec

        if find_spec("datafusion") is None:
            raise RerunMissingDependencyError("datafusion", "datafusion")

        # Check that we have a compatible version of datafusion.
        # We need a version match because the FFI is currently unstable, see:
        # https://github.com/apache/datafusion/issues/17374

        expected_df_version = CatalogClientInternal.datafusion_major_version()
        datafusion_version = version("datafusion")
        datafusion_major_version = int(datafusion_version.split(".")[0])

        if not _are_datafusion_versions_compatible(datafusion_major_version, expected_df_version):
            raise RerunIncompatibleDependencyVersionError(
                "datafusion", datafusion_version, _compatible_datafusion_version(expected_df_version)
            )

        self._internal = CatalogClientInternal(address, token)

    @classmethod
    def _from_internal(cls, internal: CatalogClientInternal) -> CatalogClient:
        """
        Wrap an existing internal client object.

        This is an internal API and should not be used directly.
        """
        instance = object.__new__(cls)
        instance._internal = internal
        return instance

    def __repr__(self) -> str:
        return self._internal.__repr__()

    @property
    def url(self) -> str:
        """Returns the catalog URL."""
        return self._internal.url

    def entries(self, *, include_hidden: bool = False) -> list[DatasetEntry | TableEntry]:
        """
        Returns a list of all entries in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include hidden entries (blueprint datasets and system tables like `__entries`).

        """
        return self.datasets(include_hidden=include_hidden) + self.tables(include_hidden=include_hidden)

    def datasets(self, *, include_hidden: bool = False) -> list[DatasetEntry]:
        """
        Returns a list of all dataset entries in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include blueprint datasets.

        """
        from . import DatasetEntry

        return [DatasetEntry(internal) for internal in self._internal.datasets(include_hidden=include_hidden)]

    def tables(self, *, include_hidden: bool = False) -> list[TableEntry]:
        """
        Returns a list of all table entries in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include system tables (e.g., `__entries`).

        """
        from . import TableEntry

        return [TableEntry(internal) for internal in self._internal.tables(include_hidden=include_hidden)]

    # ---

    @deprecated("Use entries() instead")
    def all_entries(self) -> list[DatasetEntry | TableEntry]:
        """Returns a list of all entries in the catalog."""

        return self.entries()

    @deprecated("Use datasets() instead")
    def dataset_entries(self) -> list[DatasetEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return self.datasets()

    @deprecated("Use tables() instead")
    def table_entries(self) -> list[TableEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return self.tables()

    # ---

    def entry_names(self, *, include_hidden: bool = False) -> list[str]:
        """
        Returns a list of all entry names in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include hidden entries (blueprint datasets and system tables like `__entries`).

        """
        return [e.name for e in self.entries(include_hidden=include_hidden)]

    def dataset_names(self, *, include_hidden: bool = False) -> list[str]:
        """
        Returns a list of all dataset names in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include blueprint datasets.

        """
        return [d.name for d in self.datasets(include_hidden=include_hidden)]

    def table_names(self, *, include_hidden: bool = False) -> list[str]:
        """
        Returns a list of all table names in the catalog.

        Parameters
        ----------
        include_hidden
            If True, include system tables (e.g., `__entries`).

        """
        return [t.name for t in self.tables(include_hidden=include_hidden)]

    # ---

    @overload
    def get_dataset(self, *, id: EntryId | str) -> DatasetEntry: ...

    @overload
    def get_dataset(self, name: str) -> DatasetEntry: ...

    def get_dataset(self, name: str | None = None, *, id: EntryId | str | None = None) -> DatasetEntry:
        """
        Returns a dataset by its ID or name.

        Exactly one of `id` or `name` must be provided.

        Parameters
        ----------
        name
            The name of the dataset.
        id
            The unique identifier of the dataset. Can be an `EntryId` object or its string representation.

        """
        from . import DatasetEntry

        return DatasetEntry(self._internal.get_dataset(self._resolve_name_or_id(id, name)))

    @overload
    def get_table(self, *, id: EntryId | str) -> TableEntry: ...

    @overload
    def get_table(self, name: str) -> TableEntry: ...

    def get_table(self, name: str | None = None, *, id: EntryId | str | None = None) -> TableEntry:
        """
        Returns a table by its ID or name.

        Exactly one of `id` or `name` must be provided.

        Parameters
        ----------
        name
            The name of the table.
        id
            The unique identifier of the table. Can be an `EntryId` object or its string representation.

        """
        from . import TableEntry

        return TableEntry(self._internal.get_table(self._resolve_name_or_id(id, name)))

    # ---

    @deprecated("Use get_dataset() instead")
    def get_dataset_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        """Returns a dataset by its ID or name."""
        return self.get_dataset(name=name, id=id)  # type: ignore[call-overload, no-any-return]

    @deprecated("Use get_table() instead")
    def get_table_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> TableEntry:
        """Returns a table by its ID or name."""
        return self.get_table(name=name, id=id)  # type: ignore[call-overload, no-any-return]

    # ---

    def create_dataset(self, name: str) -> DatasetEntry:
        """Creates a new dataset with the given name."""

        from . import DatasetEntry

        return DatasetEntry(self._internal.create_dataset(name))

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
        from . import TableEntry

        return TableEntry(self._internal.register_table(name, url))

    def create_table(self, name: str, schema: pa.Schema, url: str) -> TableEntry:
        """
        Create and register a new table.

        Parameters
        ----------
        name
            The name of the table entry to create. It must be unique within all entries in the catalog. An exception
            will be raised if an entry with the same name already exists.

        schema
            The schema of the table to create.

        url
            The URL of the directory for where to store the Lance table.

        """
        from . import TableEntry

        return TableEntry(self._internal.create_table(name, schema, url))

    @deprecated("Use create_table() instead")
    def create_table_entry(self, name: str, schema: pa.Schema, url: str) -> TableEntry:
        """Create and register a new table."""
        return self.create_table(name, schema, url)

    @deprecated("Use TableEntry.append(), overwrite(), or upsert() instead")
    def write_table(
        self,
        name: str,
        batches: RecordBatchReader | RecordBatch | Sequence[RecordBatch] | Sequence[Sequence[RecordBatch]],
        insert_mode: TableInsertMode,
    ) -> None:
        """
        Writes record batches into an existing table.

        Parameters
        ----------
        name
            The name of the table entry to write to. This table must already exist.

        batches
            One or more record batches to write into the table. For convenience, you can
            pass in a record batch, list of record batches, list of list of batches, or
            a [`pyarrow.RecordBatchReader`].

        insert_mode
            Determines how rows should be added to the existing table.

        """
        table = self.get_table(name=name)
        if insert_mode == TableInsertMode.APPEND:
            table.append(batches)
        elif insert_mode == TableInsertMode.OVERWRITE:
            table.overwrite(batches)
        elif insert_mode == TableInsertMode.REPLACE:
            table.upsert(batches)

    @deprecated("Use TableEntry.append() instead")
    def append_to_table(
        self,
        table_name: str,
        batches: RecordBatchReader
        | RecordBatch
        | Sequence[RecordBatch]
        | Sequence[Sequence[RecordBatch]]
        | None = None,
        **named_params: Any,
    ) -> None:
        """
        Append record batches to an existing table.

        Parameters
        ----------
        table_name
            The name of the table entry to write to. This table must already exist.

        batches
            One or more record batches to write into the table.

        **named_params
            Named parameters to write to the table as columns.

        """
        table = self.get_table(name=table_name)
        table.append(batches, **named_params)

    @deprecated("Use TableEntry.upsert() instead")
    def update_table(
        self,
        table_name: str,
        batches: RecordBatchReader
        | RecordBatch
        | Sequence[RecordBatch]
        | Sequence[Sequence[RecordBatch]]
        | None = None,
        **named_params: Any,
    ) -> None:
        """
        Upsert record batches to an existing table.

        Parameters
        ----------
        table_name
            The name of the table entry to write to. This table must already exist.

        batches
            One or more record batches to write into the table.

        **named_params
            Named parameters to write to the table as columns.

        """
        table = self.get_table(name=table_name)
        table.upsert(batches, **named_params)

    def do_global_maintenance(self) -> None:
        """Perform maintenance tasks on the whole system."""
        return self._internal.do_global_maintenance()

    @property
    def ctx(self) -> datafusion.SessionContext:
        """Returns a DataFusion session context for querying the catalog."""

        return self._internal.ctx()

    # ---

    def _resolve_name_or_id(self, id: EntryId | str | None = None, name: str | None = None) -> EntryId:
        """Helper method to resolve either ID or name. Returns the id or throw an error."""

        match id, name:
            case (None, None):
                raise ValueError("Either 'id' or 'name' must be provided.")

            case (EntryId(), None):
                return id

            case (str(id), None):
                return EntryId(id)

            case (None, str(name)):
                return self._internal._entry_id_from_entry_name(name)

            case _:
                raise ValueError("Only one of 'id' or 'name' must be provided.")
