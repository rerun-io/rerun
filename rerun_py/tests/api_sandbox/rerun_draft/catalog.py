from __future__ import annotations

import atexit
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING, Any

from rerun import catalog as _catalog

if TYPE_CHECKING:
    from collections.abc import Sequence
    from datetime import datetime

    import datafusion
    import pyarrow as pa
    from rerun.catalog import RegistrationHandle

    from rerun_bindings import IndexValuesLike  # noqa: TID251


class CatalogClient:
    """Client for a remote Rerun catalog server."""

    def __init__(self, url: str, *, token: str | None = None) -> None:
        self._inner = _catalog.CatalogClient(url, token=token)
        self.tmpdirs = []
        atexit.register(self._cleanup)

    def __repr__(self) -> str:
        return repr(self._inner)

    def entries(self, *, include_hidden=False) -> list[Entry]:
        """Returns a list of all entries in the catalog."""
        return sorted(
            [Entry(e) for e in self._inner.entries(include_hidden=include_hidden)],
            key=lambda e: e.name,
        )

    def datasets(self, *, include_hidden=False) -> list[DatasetEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return sorted(
            [DatasetEntry(e) for e in self._inner.datasets(include_hidden=include_hidden)],
            key=lambda e: e.name,
        )

    def tables(self, *, include_hidden=False) -> list[TableEntry]:
        """Returns a list of all table entries in the catalog."""
        return sorted(
            [TableEntry(e) for e in self._inner.tables(include_hidden=include_hidden)],
            key=lambda e: e.name,
        )

    def entry_names(self, *, include_hidden: bool = False) -> list[str]:
        """Returns a list of all entry names in the catalog."""
        return self._inner.entry_names(include_hidden=include_hidden)

    def dataset_names(self, *, include_hidden: bool = False) -> list[str]:
        """Returns a list of all dataset names in the catalog."""
        return self._inner.dataset_names(include_hidden=include_hidden)

    def table_names(self, *, include_hidden: bool = False) -> list[str]:
        """Returns a list of all table names in the catalog."""
        return self._inner.table_names(include_hidden=include_hidden)

    def get_table(self, *, id: EntryId | str | None = None, name: str | None = None) -> TableEntry:
        """Returns a table entry by its ID or name."""
        return TableEntry(self._inner.get_table(id=id, name=name))

    def get_dataset(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        """Returns a dataset by its ID or name."""
        return DatasetEntry(self._inner.get_dataset(id=id, name=name))

    def create_dataset(self, name: str) -> DatasetEntry:
        """Creates a new dataset with the given name."""
        return DatasetEntry(self._inner.create_dataset(name))

    def register_table(self, name: str, url: str) -> TableEntry:
        """Registers a foreign Lance table as a new table entry."""
        return TableEntry(self._inner.register_table(name, url))

    def create_table(self, name: str, schema, url: str | None = None) -> TableEntry:
        """Create and register a new table."""
        if url is None:
            tmpdir = tempfile.TemporaryDirectory()
            self.tmpdirs.append(tmpdir)
            url = Path(tmpdir.name).as_uri()
        return TableEntry(self._inner.create_table(name, schema, url))

    def do_global_maintenance(self) -> None:
        """Perform maintenance tasks on the whole system."""
        return self._inner.do_global_maintenance()

    @property
    def ctx(self) -> datafusion.SessionContext:
        """Returns a DataFusion session context for querying the catalog."""
        return self._inner.ctx

    def _cleanup(self) -> None:
        # Safety net: avoid warning if GC happens late
        try:
            for tmpdir in self.tmpdirs:
                tmpdir.cleanup()
        except Exception:
            pass


class Entry:
    """An entry in the catalog."""

    def __init__(self, inner: _catalog.Entry) -> None:
        self._inner = inner

    def __repr__(self) -> str:
        return repr(self._inner)

    @property
    def id(self) -> EntryId:
        return self._inner.id

    @property
    def name(self) -> str:
        return self._inner.name

    @property
    def kind(self) -> EntryKind:
        return self._inner.kind

    @property
    def created_at(self) -> datetime:
        return self._inner.created_at

    @property
    def updated_at(self) -> datetime:
        return self._inner.updated_at

    def delete(self) -> None:
        return self._inner.delete()

    def set_name(self, name: str) -> None:
        return self._inner.set_name(name)


# Re-export Schema from the SDK
Schema = _catalog.Schema


class DatasetEntry(Entry):
    """A dataset entry in the catalog."""

    def __init__(self, inner: _catalog.DatasetEntry) -> None:
        self._inner: _catalog.DatasetEntry = inner

    def arrow_schema(self) -> pa.Schema:
        return self._inner.arrow_schema()

    def register_blueprint(self, uri: str, set_default: bool = True) -> None:
        """
        Register an existing .rbl visible to the server.

        By default, also set this blueprint as default.
        """

        self._inner.register_blueprint(uri, set_default=set_default)

    def blueprints(self) -> list[str]:
        """Lists all blueprints currently registered with this dataset."""

        return self._inner.blueprints()

    def set_default_blueprint(self, blueprint_name: str) -> None:
        """Set an already-registered blueprint as default for this dataset."""

        self._inner.set_default_blueprint(blueprint_name=blueprint_name)

    def default_blueprint(self) -> str | None:
        """Return the name currently set blueprint."""

        return self._inner.default_blueprint()

    def schema(self) -> Schema:
        return self._inner.schema()

    def segment_ids(self) -> list[str]:
        return self._inner.segment_ids()

    def segment_table(
        self, join_meta: TableEntry | datafusion.DataFrame | None = None, join_key: str = "rerun_segment_id"
    ) -> datafusion.DataFrame:
        if isinstance(join_meta, TableEntry):
            join_meta = join_meta._inner

        return self._inner.segment_table(join_meta, join_key)

    def manifest(self) -> datafusion.DataFrame:
        return self._inner.manifest()

    def segment_url(
        self,
        segment_id: str,
        timeline: str | None = None,
        start=None,
        end=None,
    ) -> str:
        return self._inner.segment_url(segment_id, timeline, start, end)

    def register(
        self, recording_uri: str | Sequence[str], *, layer_name: str | Sequence[str] = "base"
    ) -> RegistrationHandle:
        return self._inner.register(recording_uri, layer_name=layer_name)

    def register_prefix(self, recordings_prefix: str, layer_name: str | None = None) -> RegistrationHandle:
        return self._inner.register_prefix(recordings_prefix, layer_name)

    def reader(
        self,
        *,
        index: str | None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        using_index_values: dict[str, IndexValuesLike] | datafusion.DataFrame | None = None,
        fill_latest_at: bool = False,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this dataset.

        Returns a DataFusion DataFrame.

        Parameters
        ----------
        index : str | None
            The index (timeline) to use for the view.
        include_semantically_empty_columns : bool
            Whether to include columns that are semantically empty.
        include_tombstone_columns : bool
            Whether to include tombstone columns.
        using_index_values : dict[str, IndexValuesLike] | datafusion.DataFrame | None
            If a dict is provided, keys are segment IDs and values are the index values
            to sample for that segment (per-segment semantics).
            If a DataFrame is provided, it must have 'rerun_segment_id' and index columns.
        fill_latest_at : bool
            Whether to fill null values with the latest valid data.

        """
        # Delegate to DatasetView which handles all the complex logic
        view = DatasetView(self._inner.filter_contents(["/**"]))
        return view.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            using_index_values=using_index_values,
            fill_latest_at=fill_latest_at,
        )

    def get_index_ranges(self) -> datafusion.DataFrame:
        """Returns the range bounds of all indexes per segment."""
        view = self.filter_contents(["/**"])
        return view.get_index_ranges()

    def create_fts_search_index(
        self,
        *,
        column: Any,
        time_index: Any,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        return self._inner.create_fts_search_index(
            column=column,
            time_index=time_index,
            store_position=store_position,
            base_tokenizer=base_tokenizer,
        )

    def create_vector_search_index(
        self,
        *,
        column: Any,
        time_index: Any,
        target_partition_num_rows: int | None = None,
        num_sub_vectors: int = 16,
        distance_metric: Any = ...,
    ) -> Any:
        return self._inner.create_vector_search_index(
            column=column,
            time_index=time_index,
            target_partition_num_rows=target_partition_num_rows,
            num_sub_vectors=num_sub_vectors,
            distance_metric=distance_metric,
        )

    def list_search_indexes(self) -> list:
        return self._inner.list_search_indexes()

    def delete_search_indexes(self, column: Any) -> list[Any]:
        return self._inner.delete_search_indexes(column)

    def search_fts(self, query: str, column: Any) -> datafusion.DataFrame:
        return self._inner.search_fts(query, column)

    def search_vector(self, query: Any, column: Any, top_k: int) -> datafusion.DataFrame:
        return self._inner.search_vector(query, column, top_k)

    def do_maintenance(
        self,
        optimize_indexes: bool = False,
        retrain_indexes: bool = False,
        compact_fragments: bool = False,
        cleanup_before=None,
        unsafe_allow_recent_cleanup: bool = False,
    ) -> None:
        return self._inner.do_maintenance(
            optimize_indexes=optimize_indexes,
            retrain_indexes=retrain_indexes,
            compact_fragments=compact_fragments,
            cleanup_before=cleanup_before,
            unsafe_allow_recent_cleanup=unsafe_allow_recent_cleanup,
        )

    def filter_segments(self, segment_ids: datafusion.DataFrame | Sequence[str]) -> DatasetView:
        """
        Returns a new DatasetView filtered to the given segment IDs.

        Takes either a DataFusion DataFrame with a column named 'rerun_segment_id'
        or a sequence of segment ID strings.
        """
        # Wrap in draft mock's DatasetView which adds dict-based using_index_values support
        return DatasetView(self._inner.filter_segments(segment_ids))

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """Returns a new DatasetView filtered to the given entity paths."""
        # Wrap in draft mock's DatasetView which adds dict-based using_index_values support
        return DatasetView(self._inner.filter_contents(list(exprs)))


class DatasetView:
    """
    A filtered view of a dataset.

    This is a wrapper around the SDK's DatasetView that adds dict-based
    using_index_values support for the reader() method.
    """

    def __init__(self, inner: _catalog.DatasetView) -> None:
        self._inner = inner

    def __repr__(self) -> str:
        return repr(self._inner)

    def segment_ids(self) -> list[str]:
        return self._inner.segment_ids()

    def segment_table(
        self, join_meta: TableEntry | datafusion.DataFrame | None = None, join_key: str = "rerun_segment_id"
    ) -> datafusion.DataFrame:
        # Need to unwrap TableEntry for the SDK
        if isinstance(join_meta, TableEntry):
            join_meta = join_meta.reader()
        return self._inner.segment_table(join_meta=join_meta, join_key=join_key)

    def schema(self) -> _catalog.Schema:
        return self._inner.schema()

    def arrow_schema(self) -> pa.Schema:
        return self._inner.arrow_schema()

    def reader(
        self,
        *,
        index: str | None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        using_index_values: dict[str, IndexValuesLike] | datafusion.DataFrame | None = None,
        fill_latest_at: bool = False,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this dataset view.

        Returns a DataFusion DataFrame.

        Parameters
        ----------
        index : str | None
            The index (timeline) to use for the view.
        include_semantically_empty_columns : bool
            Whether to include columns that are semantically empty.
        include_tombstone_columns : bool
            Whether to include tombstone columns.
        using_index_values : dict[str, IndexValuesLike] | datafusion.DataFrame | None
            If a dict is provided, keys are segment IDs and values are the index values
            to sample for that segment (per-segment semantics).
            If a DataFrame is provided, it must have 'rerun_segment_id' and index columns.
        fill_latest_at : bool
            Whether to fill null values with the latest valid data.

        """
        return self._inner.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )

    def get_index_ranges(self) -> datafusion.DataFrame:
        """Returns the range bounds of all indexes per segment."""
        exprs = ["rerun_segment_id"]
        for index_col in self.schema().index_columns():
            exprs.append(f"{index_col.name}:start")
            exprs.append(f"{index_col.name}:end")

        return self.segment_table().select(*exprs)

    def filter_segments(self, segment_ids: datafusion.DataFrame | Sequence[str]) -> DatasetView:
        """Returns a new DatasetView filtered to the given segment IDs."""
        return DatasetView(self._inner.filter_segments(segment_ids))

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """Returns a new DatasetView filtered to the given entity paths."""
        return DatasetView(self._inner.filter_contents(list(exprs)))


class TableEntry(Entry):
    """A table entry in the catalog."""

    def __init__(self, inner: _catalog.TableEntry) -> None:
        super().__init__(inner)
        self._inner: _catalog.TableEntry = inner

    def client(self) -> CatalogClient:
        """Returns the CatalogClient associated with this table."""
        inner_catalog = _catalog.CatalogClient.__new__(_catalog.CatalogClient)  # bypass __init__
        inner_catalog._internal = self._inner.catalog
        outer_catalog = CatalogClient.__new__(CatalogClient)  # bypass __init__
        outer_catalog._inner = inner_catalog

        return outer_catalog

    def append(
        self,
        batches: pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """Append to the Table."""
        self._inner.append(batches, **named_params)

    def overwrite(
        self,
        batches: pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """Overwrite the Table with new data."""
        self._inner.overwrite(batches, **named_params)

    def upsert(
        self,
        batches: pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """Upsert data into the Table."""
        self._inner.upsert(batches, **named_params)

    def reader(self) -> datafusion.DataFrame:
        """
        Exposes the contents of the table via a datafusion DataFrame.

        Note: this is equivalent to `catalog.ctx.table(<tablename>)`.

        This operation is lazy. The data will not be read from the source table until consumed
        from the DataFrame.
        """
        return self._inner.reader()

    def arrow_schema(self) -> pa.Schema:
        """Returns the schema of the table."""
        return self.reader().schema()


AlreadyExistsError = _catalog.AlreadyExistsError
EntryId = _catalog.EntryId
EntryKind = _catalog.EntryKind
NotFoundError = _catalog.NotFoundError
VectorDistanceMetric = _catalog.VectorDistanceMetric
