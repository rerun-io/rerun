from __future__ import annotations

import atexit
import copy
import itertools
import logging
import tempfile
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import TYPE_CHECKING, Any

import datafusion
import numpy as np
import pyarrow as pa
from rerun import catalog as _catalog
from rerun.dataframe import ComponentColumnDescriptor, IndexColumnDescriptor

if TYPE_CHECKING:
    from collections.abc import Iterator, Sequence
    from datetime import datetime

    from rerun_bindings import IndexValuesLike, Schema as _Schema  # noqa: TID251


class CatalogClient:
    """Client for a remote Rerun catalog server."""

    def __init__(self, address: str, token: str | None = None) -> None:
        self._inner = _catalog.CatalogClient(address, token)
        self.tmpdirs = []
        atexit.register(self._cleanup)

    def __repr__(self) -> str:
        return repr(self._inner)

    def all_entries(self) -> list[Entry]:
        """Returns a list of all entries in the catalog."""
        return [Entry(e) for e in self._inner.all_entries()]

    def dataset_entries(self) -> list[DatasetEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return [DatasetEntry(e) for e in self._inner.dataset_entries()]

    def table_entries(self) -> list[TableEntry]:
        """Returns a list of all table entries in the catalog."""
        return [TableEntry(e) for e in self._inner.table_entries()]

    def entry_names(self) -> list[str]:
        """Returns a list of all entry names in the catalog."""
        return self._inner.entry_names()

    def dataset_names(self) -> list[str]:
        """Returns a list of all dataset names in the catalog."""
        return self._inner.dataset_names()

    def table_names(self) -> list[str]:
        """Returns a list of all table names in the catalog."""
        return self._inner.table_names()

    def entries(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all entries in the catalog."""
        return self._inner.entries()

    def datasets(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all dataset entries in the catalog."""
        return self._inner.datasets()

    def tables(self) -> datafusion.DataFrame:
        """Returns a DataFrame containing all table entries in the catalog."""
        return self._inner.tables()

    def get_dataset_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        """Returns a dataset entry by its ID or name."""
        return DatasetEntry(self._inner.get_dataset_entry(id=id, name=name))

    def get_table(self, *, id: EntryId | str | None = None, name: str | None = None) -> TableEntry:
        """Returns a table entry by its ID or name."""
        return TableEntry(self._inner.get_table_entry(id=id, name=name))

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
        return TableEntry(self._inner.create_table_entry(name, schema, url))

    def write_table(self, name: str, batches, insert_mode) -> None:
        """Writes record batches into an existing table."""
        return self._inner.write_table(name, batches, insert_mode)

    def append_to_table(self, table_name: str, **named_params: Any) -> None:
        """Convert Python objects into columns of data and append them to a table."""
        return self._inner.append_to_table(table_name, **named_params)

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

    def update(self, *, name: str | None = None) -> None:
        return self._inner.update(name=name)


class Schema:
    """A schema view over a dataset in the catalog."""

    def __init__(self, inner: _Schema, lazy_state: _LazyDatasetState) -> None:
        self._inner: _Schema = inner
        self._component_columns: list[ComponentColumnDescriptor] = []
        self._index_columns: list[IndexColumnDescriptor] = []

        # Use lazy_state to filter component columns
        for col in self._inner:
            if isinstance(col, ComponentColumnDescriptor):
                if all(filter.matches(col.entity_path) for filter in lazy_state.content_path_filters):
                    self._component_columns.append(col)
            elif isinstance(col, IndexColumnDescriptor):
                self._index_columns.append(col)

    def __iter__(self) -> Iterator[IndexColumnDescriptor | ComponentColumnDescriptor]:
        return itertools.chain(self._index_columns, self._component_columns)

    def index_columns(self) -> list[IndexColumnDescriptor]:
        return self._index_columns

    def component_columns(self) -> list[ComponentColumnDescriptor]:
        return self._component_columns

    def column_for(self, entity_path: str, component: str) -> ComponentColumnDescriptor | None:
        for col in self._component_columns:
            if col.entity_path == entity_path and col.component == component:
                return col
        return None

    def column_names(self) -> list[str]:
        names = []
        for col in self:
            names.append(col.name)
        return names

    def __repr__(self) -> str:
        lines = []
        for col in self:
            lines.append(repr(col))
        return "\n".join(lines)


class DatasetEntry(Entry):
    """A dataset entry in the catalog."""

    def __init__(self, inner: _catalog.DatasetEntry) -> None:
        self._inner = inner

    @property
    def manifest_url(self) -> str:
        return self._inner.manifest_url

    def arrow_schema(self) -> pa.Schema:
        return self._inner.arrow_schema()

    def blueprint_dataset_id(self) -> EntryId | None:
        return self._inner.blueprint_dataset_id()

    def blueprint_dataset(self) -> DatasetEntry | None:
        result = self._inner.blueprint_dataset()
        return DatasetEntry(result) if result is not None else None

    def default_blueprint_segment_id(self) -> str | None:
        return self._inner.default_blueprint_partition_id()

    def set_default_blueprint_segment_id(self, segment_id: str | None) -> None:
        return self._inner.set_default_blueprint_partition_id(segment_id)

    def schema(self) -> Schema:
        return Schema(self._inner.schema(), _LazyDatasetState())

    def segment_ids(self) -> list[str]:
        return self._inner.partition_ids()

    def segment_table(
        self, join_meta: TableEntry | datafusion.DataFrame | None = None, join_key: str = "rerun_segment_id"
    ) -> datafusion.DataFrame:
        view = DatasetView(self._inner, _LazyDatasetState())
        return view.segment_table(join_meta=join_meta, join_key=join_key)

    def manifest(self) -> Any:
        return self._inner.manifest()

    def segment_url(
        self,
        segment_id: str,
        timeline: str | None = None,
        start=None,
        end=None,
    ) -> str:
        return self._inner.partition_url(segment_id, timeline, start, end)

    def register(self, recording_uri: str, *, recording_layer: str = "base", timeout_secs: int = 60) -> str:
        return self._inner.register(recording_uri, recording_layer=recording_layer, timeout_secs=timeout_secs)

    def register_batch(self, recording_uris: list[str], *, recording_layers: list[str] | None = None) -> Any:
        if recording_layers is None:
            recording_layers = []
        return self._inner.register_batch(recording_uris, recording_layers=recording_layers)

    def register_prefix(self, recordings_prefix: str, layer_name: str | None = None) -> Any:
        return self._inner.register_prefix(recordings_prefix, layer_name)

    def download_segment(self, segment_id: str) -> Any:
        return self._inner.download_partition(segment_id)

    def reader(
        self,
        *,
        index: str | None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        using_index_values: dict[str, IndexValuesLike] | datafusion.DataFrame | None = None,
        fill_latest_at: bool = False,
    ) -> datafusion.DataFrame:
        view = DatasetView(self._inner, _LazyDatasetState())
        return view.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            using_index_values=using_index_values,
            fill_latest_at=fill_latest_at,
        )

    def index_ranges(self, index: str | IndexColumnDescriptor) -> datafusion.DataFrame:
        view = DatasetView(self._inner, _LazyDatasetState())
        return view.index_ranges(index)

    def create_fts_index(
        self,
        *,
        column: Any,
        time_index: Any,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        return self._inner.create_fts_index(
            column=column,
            time_index=time_index,
            store_position=store_position,
            base_tokenizer=base_tokenizer,
        )

    def create_vector_index(
        self,
        *,
        column: Any,
        time_index: Any,
        target_partition_num_rows: int | None = None,
        num_sub_vectors: int = 16,
        distance_metric: Any = ...,
    ) -> Any:
        return self._inner.create_vector_index(
            column=column,
            time_index=time_index,
            target_partition_num_rows=target_partition_num_rows,
            num_sub_vectors=num_sub_vectors,
            distance_metric=distance_metric,
        )

    def list_indexes(self) -> list:
        return self._inner.list_indexes()

    def delete_indexes(self, column: Any) -> list[Any]:
        return self._inner.delete_indexes(column)

    def search_fts(self, query: str, column: Any) -> Any:
        return self._inner.search_fts(query, column)

    def search_vector(self, query: Any, column: Any, top_k: int) -> Any:
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
        Returns a new DatasetEntry filtered to the given segment IDs.

        Takes either a DataFusion DataFrame with a column named 'rerun_segment_id'
        or a sequence of segment ID strings.
        """
        new_lazy_state = _LazyDatasetState().with_segment_filters(segment_ids)

        return DatasetView(self._inner, lazy_state=new_lazy_state)

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """Returns a new DatasetEntry filtered to the given entity paths."""
        new_lazy_state = _LazyDatasetState().with_content_filters(exprs)

        return DatasetView(self._inner, lazy_state=new_lazy_state)


class _ContentMatcher:
    """
    Helper class to match contents expressions against entity paths.

    This is a poor version of the actual Rerun logic, but good enough for testing.
    """

    def normalize_path(self, path: str) -> str:
        prefix = "+"
        if path.startswith(("+", "-")):
            prefix = path[0]
            path = path[1:]
            path = path.lstrip("+-")

        normalized_path = str(PurePosixPath("/" + path.lstrip("/")))

        return prefix + normalized_path

    def __init__(self, exprs: str) -> None:
        # path -> included?
        self.exact: dict[str, bool] = {}
        self.prefix: list[tuple[str, bool]] = []

        # Split by whitespace, normalize each path
        # Add to exact or prefix lists based on /** suffix
        for raw_expr in exprs.split():
            expr = self.normalize_path(raw_expr)
            if expr.endswith("/**"):
                if expr.startswith("-"):
                    self.prefix.append((expr[1:-3], False))
                else:
                    self.prefix.append((expr[1:-3], True))
            else:
                if expr.startswith("-"):
                    self.exact[expr[1:]] = False
                else:
                    self.exact[expr[1:]] = True

        # Unless `/__properties__/**` was explicitly included, always exclude it
        if not any(prefix == "/__properties" for prefix, _ in self.prefix):
            self.prefix.append(("/__properties", False))

        # Sort prefix expressions by length (longest first)
        self.prefix.sort(key=lambda p: len(p[0]), reverse=True)

    def matches(self, path: str) -> bool:
        path = str(PurePosixPath("/" + path.lstrip("/")))

        # First check for exact match
        if path in self.exact:
            return self.exact[path]

        # Otherwise find the first matching prefix
        for prefix, included in self.prefix:
            if path.startswith(prefix):
                return included
        return False


@dataclass
class _LazyDatasetState:
    # None means no filtering
    # Otherwise we accumulate a set via intersection
    filtered_segments: set[str] | None = None
    content_path_filters: list[_ContentMatcher] = field(default_factory=list)

    def with_segment_filters(self, segment_ids: datafusion.DataFrame | Sequence[str]) -> _LazyDatasetState:
        new_lazy_state = copy.deepcopy(self)

        if isinstance(segment_ids, datafusion.DataFrame):
            if "rerun_segment_id" not in segment_ids.schema().names:
                raise ValueError("DataFrame segment_ids must have a column named 'rerun_segment_id'.")
            filt_segment_ids = {
                segment_id.as_py() for batch in segment_ids.collect() for segment_id in batch.column("rerun_segment_id")
            }
        else:
            filt_segment_ids = set(segment_ids)

        if new_lazy_state.filtered_segments is not None:
            new_lazy_state.filtered_segments &= filt_segment_ids
        else:
            new_lazy_state.filtered_segments = filt_segment_ids

        return new_lazy_state

    def with_content_filters(
        self,
        exprs: Sequence[str],
    ) -> _LazyDatasetState:
        new_lazy_state = copy.deepcopy(self)

        exprs = " ".join(exprs)

        new_lazy_state.content_path_filters.append(_ContentMatcher(exprs))

        return new_lazy_state


class DatasetView:
    """A view over a dataset in the catalog."""

    def __init__(self, inner: _catalog.DatasetEntry, lazy_state: _LazyDatasetState) -> None:
        self._inner: _catalog.DatasetEntry = inner
        self._lazy_state: _LazyDatasetState = lazy_state

    def schema(self) -> Schema:
        return Schema(self._inner.schema(), self._lazy_state)

    def arrow_schema(self) -> pa.Schema:
        filtered_schema = self._inner.arrow_schema()

        for filter in self._lazy_state.content_path_filters or [_ContentMatcher("/**")]:
            filtered_schema = pa.schema([
                field
                for field in filtered_schema
                if field.metadata.get(b"rerun:kind", None) != b"data"
                or filter.matches(field.metadata.get(b"rerun:entity_path", b"").decode("utf-8"))
            ])

        return filtered_schema

    def segment_ids(self) -> list[str]:
        if self._lazy_state.filtered_segments is not None:
            return [pid for pid in self._inner.partition_ids() if pid in self._lazy_state.filtered_segments]
        else:
            return self._inner.partition_ids()

    def download_segment(self, segment_id: str) -> Any:
        return self._inner.download_partition(segment_id)

    def segment_table(
        self, join_meta: TableEntry | datafusion.DataFrame | None = None, join_key: str = "rerun_segment_id"
    ) -> datafusion.DataFrame:
        # Get the partition table from the inner object

        partitions = self._inner.partition_table().df().with_column_renamed("rerun_partition_id", "rerun_segment_id")

        if self._lazy_state.filtered_segments is not None:
            ctx = datafusion.SessionContext()

            segment_df = ctx.from_arrow(
                pa.Table.from_arrays(
                    [pa.array(list(self._lazy_state.filtered_segments))], names=["filtered_segment_id"]
                ),
            )

            partitions = partitions.join(segment_df, left_on="rerun_segment_id", right_on="filtered_segment_id").drop(
                "filtered_segment_id"
            )

        if join_meta is not None:
            if isinstance(join_meta, TableEntry):
                join_meta = join_meta.reader()
            if join_key not in partitions.schema().names:
                raise ValueError(f"Dataset partition table must contain join_key column '{join_key}'.")
            if join_key not in join_meta.schema().names:
                raise ValueError(f"join_meta must contain join_key column '{join_key}'.")

            meta_join_key = join_key + "_meta"

            join_meta = join_meta.with_column_renamed(join_key, meta_join_key)

            return partitions.join(
                join_meta,
                left_on=join_key,
                right_on=meta_join_key,
                how="left",
            ).drop(meta_join_key)
        else:
            return partitions

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
        Create a reader over this DatasetView as a datafusion DataFrame.

        The reader will return rows for all data that exists on the specified index.
        It will either return 1 row per index value, or if `using_index_values` is provided,
        it will instead generate rows for each of the provided values.

        `using_index_values` can be provided in either of these forms:
        - a dictionary mapping segment IDs to index values
        - a DataFusion DataFrame with a column named 'rerun_segment_id' and a column named after the provided `index`

        The operation is lazy. The data will not be read from the source dataset until consumed.
        """
        full_contents = defaultdict(list)

        # Note: arrow_schema() here already describes the intended schema
        for fld in self.arrow_schema():
            if fld.metadata.get(b"rerun:kind", None) == b"data":
                entity_path = fld.metadata.get(b"rerun:entity_path", b"").decode("utf-8")
                component = fld.metadata.get(b"rerun:component", b"").decode("utf-8")
                full_contents[entity_path].append(component)

        view = self._inner.dataframe_query_view(
            index=index,
            contents=full_contents,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
        )

        # Apply fill_latest_at if requested
        if fill_latest_at:
            view = view.fill_latest_at()

        if using_index_values and index is None:
            raise ValueError("index must be provided when using_index_values is provided")

        if using_index_values is not None:
            # convert to dictionary representation
            if isinstance(using_index_values, datafusion.DataFrame):
                rows = using_index_values.select("rerun_segment_id", index).to_pylist()

                using_index_values = defaultdict(list)
                for row in rows:
                    using_index_values[row["rerun_segment_id"]].append(row[index])

                using_index_values = {k: np.array(v, dtype=np.datetime64) for k, v in using_index_values.items()}

            # Fake the intended behavior: index values are provided on a per-segment basis. If a segment is missing,
            # no rows are generated for it.
            segments = self._lazy_state.filtered_segments or self._inner.partition_ids()

            df = None
            for segment in segments:
                if segment in using_index_values:
                    index_values = using_index_values.pop(segment)
                else:
                    index_values = np.array([], dtype=np.datetime64)

                other_df = (
                    view.filter_partition_id(segment)
                    .using_index_values(index_values)
                    .df()
                    .with_column_renamed("rerun_partition_id", "rerun_segment_id")
                )

                if df is None:
                    df = other_df
                else:
                    df = df.union(other_df)

            if len(using_index_values) > 0:
                logging.warning(
                    "Index values for the following inexistent or filtered segments were ignored: "
                    f"{', '.join(using_index_values.keys())}"
                )

            if df is None:
                # Return an empty DataFrame with the correct schema
                return (
                    self._inner.dataframe_query_view(index=index, contents=full_contents)
                    .using_index_values(np.array([], dtype=np.datetime64))
                    .df()
                    .with_column_renamed("rerun_partition_id", "rerun_segment_id")
                )
            else:
                return df
        else:
            if self._lazy_state.filtered_segments is not None:
                view = view.filter_partition_id(*self._lazy_state.filtered_segments)

            return view.df().with_column_renamed("rerun_partition_id", "rerun_segment_id")

    def index_ranges(self, index: str | IndexColumnDescriptor) -> datafusion.DataFrame:
        import datafusion.functions as F
        from datafusion import col

        schema = self.schema()
        exprs = []

        for index_column in schema.index_columns():
            exprs.append(F.min(col(index_column.name)).alias(f"{index_column.name}:min"))
            exprs.append(F.max(col(index_column.name)).alias(f"{index_column.name}:max"))

        # TODO(ab, jleibs): we're still unsure about these, so let's keep them aside for now.
        # for component_column in schema.component_columns():
        #     if component_column.name.startswith("property:"):
        #         continue
        #     exprs.append(F.count(col(component_column.name)).alias(f"count({component_column.name})"))

        return self.reader(index=index).aggregate("rerun_segment_id", exprs)

    def filter_segments(self, segment_ids: datafusion.DataFrame | Sequence[str]) -> DatasetView:
        """
        Returns a new DatasetEntry filtered to the given segment IDs.

        Takes either a DataFusion DataFrame with a column named 'rerun_segment_id'
        or a sequence of segment ID strings.
        """
        new_lazy_state = self._lazy_state.with_segment_filters(segment_ids)

        return DatasetView(self._inner, lazy_state=new_lazy_state)

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """
        Returns a new DatasetEntry filtered to the given entity paths.

        NOTE: The choice of `contents` and `filter` are both intentional here.

        Contents as a string gives us more flexibility in specifying what to include/exclude in ways that
        include components. For example: `+/**:Points3D` or maybe `+/** -:Image`. This follows how we
        specify this in blueprints.

        We choose `filter` rather than `select` to make it clear that this is an operation that not only
        reduces the number of columns but ALSO reduces the number of rows. I.e. we remove every row for
        which there are no remaining non-index columns after filtering.
        """
        new_lazy_state = self._lazy_state.with_content_filters(exprs)

        return DatasetView(self._inner, lazy_state=new_lazy_state)


class TableEntry(Entry):
    """A table entry in the catalog."""

    def __init__(self, inner: _catalog.TableEntry) -> None:
        super().__init__(inner)
        self._inner = inner

    def client(self) -> CatalogClient:
        """Returns the CatalogClient associated with this table."""
        inner_catalog = _catalog.CatalogClient.__new__(_catalog.CatalogClient)  # bypass __init__
        inner_catalog._raw_client = self._inner.catalog
        outer_catalog = CatalogClient.__new__(CatalogClient)  # bypass __init__
        outer_catalog._inner = inner_catalog

        return outer_catalog

    def append(self, **named_params: Any) -> None:
        """Convert Python objects into columns of data and append them to a table."""
        self.client().append_to_table(self._inner.name, **named_params)

    def update(self, *, name: str | None = None) -> None:
        return self._inner.update(name=name)

    def reader(self) -> datafusion.DataFrame:
        """
        Exposes the contents of the table via a datafusion DataFrame.

        Note: this is equivalent to `catalog.ctx.table(<tablename>)`.

        This operation is lazy. The data will not be read from the source table until consumed
        from the DataFrame.
        """
        return self._inner.df()

    def schema(self) -> pa.Schema:
        """Returns the schema of the table."""
        return self.reader().schema()

    def to_polars(self) -> Any:
        """Returns the table as a Polars DataFrame."""
        return self.reader().to_polars()


AlreadyExistsError = _catalog.AlreadyExistsError
DataframeQueryView = _catalog.DataframeQueryView
EntryId = _catalog.EntryId
EntryKind = _catalog.EntryKind
NotFoundError = _catalog.NotFoundError
TableInsertMode = _catalog.TableInsertMode
Task = _catalog.Task
VectorDistanceMetric = _catalog.VectorDistanceMetric
