from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any

import pyarrow as pa
from pyarrow import RecordBatch, RecordBatchReader

from rerun_bindings import (
    AlreadyExistsError as AlreadyExistsError,
    CatalogClientInternal,
    DataframeQueryView as DataframeQueryView,
    DatasetEntry as DatasetEntry,
    Entry as Entry,
    EntryId as EntryId,
    EntryKind as EntryKind,
    NotFoundError as NotFoundError,
    TableEntry as TableEntry,
    TableInsertMode as TableInsertMode,
    Task as Task,
    VectorDistanceMetric as VectorDistanceMetric,
)
from rerun_bindings.types import (
    IndexValuesLike as IndexValuesLike,
    VectorDistanceMetricLike as VectorDistanceMetricLike,
)

from .error_utils import RerunIncompatibleDependencyVersionError, RerunMissingDependencyError

if TYPE_CHECKING:
    import datafusion


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

        self._raw_client = CatalogClientInternal(address, token)

    def __repr__(self) -> str:
        return self._raw_client.__repr__()

    @property
    def url(self) -> str:
        """Returns the catalog URL."""
        return self._raw_client.url

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

    def create_table_entry(self, name: str, schema: pa.Schema, url: str) -> TableEntry:
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
        return self._raw_client.create_table_entry(name, schema, url)

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
        if not isinstance(batches, RecordBatchReader):

            def flatten_batches(
                batches: RecordBatch | Sequence[RecordBatch] | Sequence[Sequence[RecordBatch]],
            ) -> list[RecordBatch]:
                """Convenience function to convert inputs to a list of batches."""
                if isinstance(batches, RecordBatch):
                    return [batches]

                if isinstance(batches, Sequence):
                    result = []
                    for item in batches:
                        if isinstance(item, RecordBatch):
                            result.append(item)
                        elif isinstance(item, Sequence):
                            result.extend(item)
                        else:
                            raise TypeError(f"Unexpected type: {type(item)}")
                    return result

                raise TypeError(f"Expected RecordBatch or Sequence, got {type(batches)}")

            batches = flatten_batches(batches)
            if len(batches) == 0:
                return
            schema = batches[0].schema
            batches = RecordBatchReader.from_batches(schema, batches)

        return self._raw_client.write_table(name, batches, insert_mode)

    def append_to_table(self, table_name: str, **named_params: Any) -> None:
        """
        Convert Python objects into columns of data and append them to a table.

        This is a convenience method to quickly turn Python objects into rows
        of data. You may pass in any parameter name which will be used for the
        column name. If you need more control over the data written to the
        server, you can also use [`CatalogClient.write_table`] to write record
        batches to the server.

        If you wish to send multiple rows at once, then all parameters should
        be a list of the same length. This function will query the table to
        determine the schema and attempt to coerce data types as appropriate.


        Parameters
        ----------
        table_name
            The name of the table entry to write to. This table must already exist.

        named_params
            Pairwise combinations of column names and the data to write.
            For example if you pass `age=3` it will attempt to create a column
            named `age` and cast the value `3` to the appropriate type.

        """
        if not named_params:
            return
        params = named_params.items()
        schema = self.get_table(name=table_name).df.schema()

        cast_params = {}
        expected_len = None
        for name, value in params:
            field = schema.field(name)
            if field is None:
                raise ValueError(f"Column {name} does not exist in table")

            try:
                cast_value = pa.array(value, type=field.type)
            except TypeError:
                cast_value = pa.array([value], type=field.type)

            cast_params[name] = cast_value

            if expected_len is None:
                expected_len = len(cast_value)
            else:
                if len(cast_value) != expected_len:
                    raise ValueError("Columns have mismatched number of rows")

        if expected_len is None or expected_len == 0:
            return

        columns = []
        for field in schema:
            if field.name in cast_params:
                columns.append(cast_params[field.name])
            else:
                columns.append(pa.array([None] * expected_len, type=field.type))

        rb = pa.RecordBatch.from_arrays(columns, schema=schema)
        self.write_table(table_name, rb, TableInsertMode.APPEND)

    def do_global_maintenance(self) -> None:
        """Perform maintenance tasks on the whole system."""
        return self._raw_client.do_global_maintenance()

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
