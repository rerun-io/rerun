from __future__ import annotations

import atexit
import copy
import itertools
import logging
import tempfile
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import TYPE_CHECKING, Any, cast

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

    def entries(self, *, include_hidden=False) -> list[Entry]:
        """Returns a list of all entries in the catalog."""
        return sorted(
            [
                Entry(e)
                for e in self._inner.all_entries()
                if (not e.name.startswith("__") and e.kind != _catalog.EntryKind.BLUEPRINT_DATASET) or include_hidden
            ],
            key=lambda e: e.name,
        )

    def datasets(self, *, include_hidden=False) -> list[DatasetEntry]:
        """Returns a list of all dataset entries in the catalog."""
        return [
            DatasetEntry(cast("_catalog.DatasetEntry", e._inner))
            for e in self.entries(include_hidden=include_hidden)
            if e.kind == _catalog.EntryKind.DATASET
        ]

    def tables(self, *, include_hidden=False) -> list[TableEntry]:
        """Returns a list of all table entries in the catalog."""
        return [
            TableEntry(cast("_catalog.TableEntry", e._inner))
            for e in self.entries(include_hidden=include_hidden)
            if e.kind == _catalog.EntryKind.TABLE
        ]

    def entry_names(self) -> list[str]:
        """Returns a list of all entry names in the catalog."""
        return self._inner.entry_names()

    def dataset_names(self) -> list[str]:
        """Returns a list of all dataset names in the catalog."""
        return self._inner.dataset_names()

    def table_names(self) -> list[str]:
        """Returns a list of all table names in the catalog."""
        return self._inner.table_names()

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
        self._inner: _catalog.DatasetEntry = inner

    def arrow_schema(self) -> pa.Schema:
        return self._inner.arrow_schema()

    def register_blueprint(self, uri: str, set_default: bool = True) -> None:
        """
        Register an existing .rbl visible to the server.

        By default, also set this blueprint as default.
        """

        blueprint_dataset = self._inner.blueprint_dataset()
        segment_id = blueprint_dataset.register(uri)

        if set_default:
            self._inner.set_default_blueprint_partition_id(segment_id)

    def blueprints(self) -> list[str]:
        """Lists all blueprints currently registered with this dataset."""

        return self._inner.blueprint_dataset().partition_ids()

    def set_default_blueprint(self, blueprint_name: str) -> None:
        """Set an already-registered blueprint as default for this dataset."""

        self._inner.set_default_blueprint_partition_id(blueprint_name)

    def default_blueprint(self) -> str | None:
        """Return the name currently set blueprint."""

        return self._inner.default_blueprint_partition_id()

    def schema(self) -> Schema:
        return Schema(self._inner.schema(), _LazyDatasetState())

    def segment_ids(self) -> list[str]:
        return self._inner.partition_ids()

    def segment_table(
        self, join_meta: TableEntry | datafusion.DataFrame | None = None, join_key: str = "rerun_segment_id"
    ) -> datafusion.DataFrame:
        view = DatasetView(self._inner, _LazyDatasetState())
        return view.segment_table(join_meta=join_meta, join_key=join_key)

    def manifest(self) -> datafusion.DataFrame:
        return self._inner.manifest().df().with_column_renamed("rerun_partition_id", "rerun_segment_id")

    def segment_url(
        self,
        segment_id: str,
        timeline: str | None = None,
        start=None,
        end=None,
    ) -> str:
        return self._inner.partition_url(segment_id, timeline, start, end)

    def register(self, recording_uri: str | Sequence[str], *, layer_name: str | Sequence[str] = "base") -> Tasks:
        if isinstance(recording_uri, str):
            recording_uri = [recording_uri]
        else:
            recording_uri = list(recording_uri)

        if isinstance(layer_name, str):
            layer_name = [layer_name] * len(recording_uri)
        else:
            layer_name = list(layer_name)
            if len(layer_name) != len(recording_uri):
                raise ValueError("`layer_name` must be the same length as `recording_uri`")

        return Tasks(self._inner.register_batch(recording_uri, recording_layers=layer_name))

    # TODO(ab): are we merging this into `register` as well?
    def register_prefix(self, recordings_prefix: str, layer_name: str | None = None) -> Tasks:
        return Tasks(self._inner.register_prefix(recordings_prefix, layer_name))

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

    def get_index_ranges(self, index: str | IndexColumnDescriptor) -> datafusion.DataFrame:
        view = DatasetView(self._inner, _LazyDatasetState())
        return view.get_index_ranges(index)

    def create_fts_search_index(
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

    def create_vector_search_index(
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

    def list_search_indexes(self) -> list:
        return self._inner.list_indexes()

    def delete_search_indexes(self, column: Any) -> list[Any]:
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

    def get_index_ranges(self, index: str | IndexColumnDescriptor) -> datafusion.DataFrame:
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
        self._inner: _catalog.TableEntry = inner

    def client(self) -> CatalogClient:
        """Returns the CatalogClient associated with this table."""
        inner_catalog = _catalog.CatalogClient.__new__(_catalog.CatalogClient)  # bypass __init__
        inner_catalog._internal = self._inner.catalog
        outer_catalog = CatalogClient.__new__(CatalogClient)  # bypass __init__
        outer_catalog._inner = inner_catalog

        return outer_catalog

    def _python_objects_to_record_batch(self, schema: pa.Schema, named_params: dict[str, Any]) -> pa.RecordBatch:
        cast_params = {}
        expected_len = None

        for name, value in named_params.items():
            field = schema.field(name)
            if field is None:
                raise ValueError(f"Column {name} does not exist in table")

            if isinstance(value, str):
                value = [value]

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

        return pa.RecordBatch.from_arrays(columns, schema=schema)

    def _write_batches(
        self,
        batches: pa.RecordBatch | Iterator[pa.RecordBatch] | Iterator[Iterator[pa.RecordBatch]],
        insert_mode: TableInsertMode,
    ) -> None:
        """Internal helper to write batches to the table."""
        if isinstance(batches, pa.RecordBatch):
            batches = [batches]
        self.client()._inner.write_table(self._inner.name, batches, insert_mode=insert_mode)

    def _write_named_params(
        self,
        named_params: dict[str, Any],
        insert_mode: TableInsertMode,
    ) -> None:
        """Internal helper to write named parameters to the table."""
        batches = self._python_objects_to_record_batch(self.arrow_schema(), named_params)
        if batches is not None:
            self.client()._inner.write_table(self._inner.name, [batches], insert_mode=insert_mode)

    def append(
        self,
        batches: pa.RecordBatch | Iterator[pa.RecordBatch] | Iterator[Iterator[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """
        Append to the Table.

        Parameters
        ----------
        batches
            A sequence of Arrow RecordBatches to append to the table.
        **named_params
            Each named parameter corresponds to a column in the table.

        """
        if batches is not None and len(named_params) > 0:
            raise TypeError(
                "TableEntry.append can take a sequence of RecordBatches or a named set of columns, but not both"
            )

        if batches is not None:
            self._write_batches(batches, insert_mode=TableInsertMode.APPEND)
        else:
            self._write_named_params(named_params, insert_mode=TableInsertMode.APPEND)

    def overwrite(
        self,
        batches: pa.RecordBatch | Iterator[pa.RecordBatch] | Iterator[Iterator[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """
        Overwrite the Table with new data.

        Parameters
        ----------
        batches
            A sequence of Arrow RecordBatches to overwrite the table with.
        **named_params
            Each named parameter corresponds to a column in the table.

        """
        if batches is not None and len(named_params) > 0:
            raise TypeError(
                "TableEntry.overwrite can take a sequence of RecordBatches or a named set of columns, but not both"
            )

        if batches is not None:
            self._write_batches(batches, insert_mode=TableInsertMode.OVERWRITE)
        else:
            self._write_named_params(named_params, insert_mode=TableInsertMode.OVERWRITE)

    def upsert(
        self,
        batches: pa.RecordBatch | Iterator[pa.RecordBatch] | Iterator[Iterator[pa.RecordBatch]] | None = None,
        **named_params: Any,
    ) -> None:
        """
        Upsert data into the Table.

        To use upsert, the table must contain a column with the metadata:
        ```
            {"rerun:is_table_index" = "true"}
        ```

        Any row with a matching index value will have the new data inserted.
        Any row without a matching index value will be appended as a new row.

        Parameters
        ----------
        batches
            A sequence of Arrow RecordBatches to upsert into the table.
        **named_params
            Each named parameter corresponds to a column in the table

        """
        if batches is not None and len(named_params) > 0:
            raise TypeError(
                "TableEntry.upsert can take a sequence of RecordBatches or a named set of columns, but not both"
            )

        if batches is not None:
            self._write_batches(batches, insert_mode=TableInsertMode.REPLACE)
        else:
            self._write_named_params(named_params, insert_mode=TableInsertMode.REPLACE)

    def reader(self) -> datafusion.DataFrame:
        """
        Exposes the contents of the table via a datafusion DataFrame.

        Note: this is equivalent to `catalog.ctx.table(<tablename>)`.

        This operation is lazy. The data will not be read from the source table until consumed
        from the DataFrame.
        """
        return self._inner.df()

    def arrow_schema(self) -> pa.Schema:
        """Returns the schema of the table."""
        return self.reader().schema()

    def to_polars(self) -> Any:
        """Returns the table as a Polars DataFrame."""
        return self.reader().to_polars()


class Tasks:
    def __init__(self, inner: _catalog.Tasks) -> None:
        self._inner: _catalog.Tasks = inner

    def wait(self, timeout_secs: int = 60) -> None:
        self._inner.wait(timeout_secs)

    def status_table(self) -> datafusion.DataFrame:
        return self._inner.status_table().df()

    def __len__(self) -> int:
        return self._inner.__len__()

    def __getitem__(self, index: int) -> Task:
        return self._inner.__getitem__(index)


AlreadyExistsError = _catalog.AlreadyExistsError
DataframeQueryView = _catalog.DataframeQueryView
EntryId = _catalog.EntryId
EntryKind = _catalog.EntryKind
NotFoundError = _catalog.NotFoundError
TableInsertMode = _catalog.TableInsertMode
Task = _catalog.Task
VectorDistanceMetric = _catalog.VectorDistanceMetric
