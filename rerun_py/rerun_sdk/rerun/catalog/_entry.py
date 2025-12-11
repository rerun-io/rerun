from __future__ import annotations

from abc import ABC
from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Generic, TypeAlias, TypeVar

import datafusion
import pyarrow as pa
from pyarrow import RecordBatchReader
from typing_extensions import deprecated

from rerun_bindings import (
    DatasetEntryInternal,
    DatasetViewInternal,
    TableEntryInternal,
    TableInsertMode,
)

#: Type alias for supported batch input types for TableEntry write methods.
_BatchesType: TypeAlias = (
    RecordBatchReader | pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]]
)

if TYPE_CHECKING:
    from datetime import datetime

    from rerun.dataframe import Recording

    from . import (
        CatalogClient,
        ComponentColumnDescriptor,
        ComponentColumnSelector,
        DataFusionTable,
        EntryId,
        EntryKind,
        IndexColumnSelector,
        IndexConfig,
        IndexingResult,
        IndexValuesLike,
        Schema,
        Tasks,
        VectorDistanceMetric,
    )


InternalEntryT = TypeVar("InternalEntryT", DatasetEntryInternal, TableEntryInternal)


class Entry(ABC, Generic[InternalEntryT]):
    """An entry in the catalog."""

    __slots__ = ("_internal",)

    def __init__(self, inner: InternalEntryT) -> None:
        self._internal: InternalEntryT = inner

    def __repr__(self) -> str:
        return f"Entry({self.kind}, '{self.name}')"

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


class DatasetEntry(Entry[DatasetEntryInternal]):
    """A dataset entry in the catalog."""

    @property
    def manifest_url(self) -> str:
        """Return the dataset manifest URL."""

        return self._internal.manifest_url

    def arrow_schema(self) -> pa.Schema:
        """Return the Arrow schema of the data contained in the dataset."""

        return self._internal.arrow_schema()

    def blueprint_dataset_id(self) -> EntryId | None:
        """The ID of the associated blueprint dataset, if any."""

        return self._internal.blueprint_dataset_id()

    def blueprint_dataset(self) -> DatasetEntry | None:
        """The associated blueprint dataset, if any."""

        ds = self._internal.blueprint_dataset()
        return None if ds is None else DatasetEntry(ds)

    def default_blueprint_segment_id(self) -> str | None:
        """The default blueprint segment ID for this dataset, if any."""

        return self._internal.default_blueprint_segment_id()

    def set_default_blueprint_segment_id(self, segment_id: str | None) -> None:
        """
        Set the default blueprint segment ID for this dataset.

        Pass `None` to clear the blueprint. This fails if the change cannot be made to the remote server.
        """

        return self._internal.set_default_blueprint_segment_id(segment_id)

    @deprecated("Use default_blueprint_segment_id() instead")
    def default_blueprint_partition_id(self) -> str | None:
        """The default blueprint partition ID for this dataset, if any."""
        return self.default_blueprint_segment_id()

    @deprecated("Use set_default_blueprint_segment_id() instead")
    def set_default_blueprint_partition_id(self, partition_id: str | None) -> None:
        """Set the default blueprint partition ID for this dataset."""
        return self.set_default_blueprint_segment_id(partition_id)

    def schema(self) -> Schema:
        """Return the schema of the data contained in the dataset."""
        from ._schema import Schema

        return Schema(self._internal.schema())

    def segment_ids(self) -> list[str]:
        """Returns a list of segment IDs for the dataset."""

        return self._internal.segment_ids()

    @deprecated("Use segment_ids() instead")
    def partition_ids(self) -> list[str]:
        """Returns a list of partition IDs for the dataset."""
        return self.segment_ids()

    def segment_table(self) -> DataFusionTable:
        """Return the segment table as a Datafusion table provider."""

        return self._internal.segment_table()

    @deprecated("Use segment_table() instead")
    def partition_table(self) -> DataFusionTable:
        """Return the partition table as a Datafusion table provider."""
        return self.segment_table()

    def manifest(self) -> DataFusionTable:
        """Return the dataset manifest as a Datafusion table provider."""

        return self._internal.manifest()

    def segment_url(  # noqa: PLR0917
        self,
        segment_id: str,
        timeline: str | None = None,
        start: datetime | int | None = None,
        end: datetime | int | None = None,
    ) -> str:
        """
        Return the URL for the given segment.

        Parameters
        ----------
        segment_id: str
            The ID of the segment to get the URL for.

        timeline: str | None
            The name of the timeline to display.

        start: int | datetime | None
            The start time for the segment.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        end: int | datetime | None
            The end time for the segment.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        Examples
        --------
        # With ticks
        >>> start_tick, end_time = 0, 10
        >>> dataset.segment_url("some_id", "log_tick", start_tick, end_time)

        # With timestamps
        >>> start_time, end_time = datetime.now() - timedelta(seconds=4), datetime.now()
        >>> dataset.segment_url("some_id", "real_time", start_time, end_time)

        Returns
        -------
        str
            The URL for the given segment.

        """

        return self._internal.segment_url(segment_id, timeline, start, end)

    @deprecated("Use segment_url() instead")
    def partition_url(  # noqa: PLR0917
        self,
        partition_id: str,
        timeline: str | None = None,
        start: datetime | int | None = None,
        end: datetime | int | None = None,
    ) -> str:
        """Return the URL for the given partition."""
        return self.segment_url(partition_id, timeline, start, end)

    def register(self, recording_uri: str, *, recording_layer: str = "base", timeout_secs: int = 60) -> str:
        """
        Register a RRD URI to the dataset and wait for completion.

        This method registers a single recording to the dataset and blocks until the registration is
        complete, or after a timeout (in which case, a `TimeoutError` is raised).

        Parameters
        ----------
        recording_uri: str
            The URI of the RRD to register.

        recording_layer: str
            The layer to which the recording will be registered to.

        timeout_secs: int
            The timeout after which this method raises a `TimeoutError` if the task is not completed.

        Returns
        -------
        segment_id: str
            The segment ID of the registered RRD.

        """

        return self._internal.register(recording_uri, recording_layer=recording_layer, timeout_secs=timeout_secs)

    def register_batch(self, recording_uris: list[str], *, recording_layers: list[str] | None = None) -> Tasks:
        """
        Register a batch of RRD URIs to the dataset and return a handle to the tasks.

        This method initiates the registration of multiple recordings to the dataset, and returns
        the corresponding task ids in a [`Tasks`] object.

        Parameters
        ----------
        recording_uris: list[str]
            The URIs of the RRDs to register.

        recording_layers: list[str]
            The layers to which the recordings will be registered to:
            * When empty, this defaults to `["base"]`.
            * If longer than `recording_uris`, `recording_layers` will be truncated.
            * If shorter than `recording_uris`, `recording_layers` will be extended by repeating its last value.
              I.e. an empty `recording_layers` will result in `"base"` begin repeated `len(recording_layers)` times.

        """

        if recording_layers is None:
            recording_layers = []

        return self._internal.register_batch(recording_uris, recording_layers=recording_layers)

    def register_prefix(self, recordings_prefix: str, layer_name: str | None = None) -> Tasks:
        """
        Register all RRDs under a given prefix to the dataset and return a handle to the tasks.

        A prefix is a directory-like path in an object store (e.g. an S3 bucket or ABS container).
        All RRDs that are recursively found under the given prefix will be registered to the dataset.

        This method initiates the registration of the recordings to the dataset, and returns
        the corresponding task ids in a [`Tasks`] object.

        Parameters
        ----------
        recordings_prefix: str
            The prefix under which to register all RRDs.

        layer_name: Optional[str]
            The layer to which the recordings will be registered to.
            If `None`, this defaults to `"base"`.

        """

        return self._internal.register_prefix(recordings_prefix, layer_name=layer_name)

    def download_segment(self, segment_id: str) -> Recording:
        """Download a segment from the dataset."""

        return self._internal.download_segment(segment_id)

    @deprecated("Use download_segment() instead")
    def download_partition(self, partition_id: str) -> Recording:
        """Download a partition from the dataset."""
        return self.download_segment(partition_id)

    def filter_segments(self, segment_ids: datafusion.DataFrame | Sequence[str]) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Parameters
        ----------
        segment_ids
            A list of segment ID strings or a DataFusion DataFrame
            with a column named 'rerun_segment_id'. When passing a DataFrame,
            if there are additional columns, they will be ignored.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        Examples
        --------
        ```python
        # Filter to specific segments
        view = dataset.filter_segments(["recording_0", "recording_1"])

        # Filter using a DataFrame
        good_segments = metadata_table.df().filter(col("success"))
        view = dataset.filter_segments(good_segments)

        # Read data from the filtered view
        df = view.reader(index="timeline")
        ```

        """

        if isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Parameters
        ----------
        exprs : Sequence[str]
            Entity path expressions.

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths.

        Examples
        --------
        ```python
        # Filter to specific entity paths
        view = dataset.filter_contents(["/points/**"])

        # Exclude certain paths
        view = dataset.filter_contents(["/points/**", "-/text/**"])

        # Chain with segment filters
        view = dataset.filter_segments(["recording_0"]).filter_contents(["/points/**"])
        ```

        """

        return DatasetView(self._internal.filter_contents(list(exprs)))

    def reader(
        self,
        *,
        index: str | None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: IndexValuesLike | None = None,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this dataset.

        Returns a DataFusion DataFrame.

        Parameters
        ----------
        index : str | None
            The index (timeline) to use for the view.
            Pass `None` to read only static data.
        include_semantically_empty_columns : bool
            Whether to include columns that are semantically empty.
        include_tombstone_columns : bool
            Whether to include tombstone columns.
        fill_latest_at : bool
            Whether to fill null values with the latest valid data.
        using_index_values : IndexValuesLike | None
            If provided, specifies the exact index values to sample for all segments.
            Can be a numpy array (datetime64[ns] or int64), a pyarrow Array, or a sequence.
            Use with `fill_latest_at=True` to populate rows with the most recent data.

        Returns
        -------
        datafusion.DataFrame
            A DataFusion DataFrame.

        """

        return self.filter_contents(["/**"]).reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )

    def create_fts_index(
        self,
        *,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        time_index: IndexColumnSelector,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        """Create a full-text search index on the given column."""

        return self._internal.create_fts_index(
            column=column,
            time_index=time_index,
            store_position=store_position,
            base_tokenizer=base_tokenizer,
        )

    def create_vector_index(
        self,
        *,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        time_index: IndexColumnSelector,
        target_partition_num_rows: int | None = None,
        num_sub_vectors: int = 16,
        distance_metric: VectorDistanceMetric | str = "Cosine",
    ) -> IndexingResult:
        """
        Create a vector index on the given column.

        This will enable indexing and build the vector index over all existing values
        in the specified component column.

        Results can be retrieved using the `search_vector` API, which will include
        the time-point on the indexed timeline.

        Only one index can be created per component column -- executing this a second
        time for the same component column will replace the existing index.

        Parameters
        ----------
        column
            The component column to create the index on.
        time_index
            Which timeline this index will map to.
        target_partition_num_rows
            The target size (in number of rows) for each partition.
            The underlying indexer (lance) will pick a default when no value
            is specified - today this is 8192. It will also cap the
            maximum number of partitions independently of this setting - currently
            4096.
        num_sub_vectors
            The number of sub-vectors to use when building the index.
        distance_metric
            The distance metric to use for the index. ("L2", "Cosine", "Dot", "Hamming")

        """

        return self._internal.create_vector_index(
            column=column,
            time_index=time_index,
            target_partition_num_rows=target_partition_num_rows,
            num_sub_vectors=num_sub_vectors,
            distance_metric=distance_metric,
        )

    def list_indexes(self) -> list[IndexingResult]:
        """List all user-defined indexes in this dataset."""

        return self._internal.list_indexes()

    def delete_indexes(
        self,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> list[IndexConfig]:
        """Deletes all user-defined indexes for the specified column."""

        return self._internal.delete_indexes(column)

    def search_fts(
        self,
        query: str,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> DataFusionTable:
        """Search the dataset using a full-text search query."""

        return self._internal.search_fts(query, column)

    def search_vector(
        self,
        query: Any,  # VectorLike
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        top_k: int,
    ) -> DataFusionTable:
        """Search the dataset using a vector search query."""

        return self._internal.search_vector(query, column, top_k)

    def do_maintenance(  # noqa: PLR0917
        self,
        optimize_indexes: bool = False,
        retrain_indexes: bool = False,
        compact_fragments: bool = False,
        cleanup_before: datetime | None = None,
        unsafe_allow_recent_cleanup: bool = False,
    ) -> None:
        """Perform maintenance tasks on the datasets."""

        return self._internal.do_maintenance(
            optimize_indexes, retrain_indexes, compact_fragments, cleanup_before, unsafe_allow_recent_cleanup
        )


class DatasetView:
    """
    A filtered view over a dataset in the catalog.

    A `DatasetView` provides lazy filtering over a dataset's segments and entity paths.
    Filters are composed lazily and only applied when data is actually read.

    Create a `DatasetView` by calling `filter_segments()` or `filter_contents()` on a
    `DatasetEntry`.

    Examples
    --------
    ```python
    # Filter to specific segments
    view = dataset.filter_segments(["recording_0", "recording_1"])

    # Filter to specific entity paths
    view = dataset.filter_contents(["/points/**"])

    # Chain filters
    view = dataset.filter_segments(["recording_0"]).filter_contents(["/points/**"])

    # Read data
    df = view.reader(index="timeline")
    ```

    """

    def __init__(self, internal: DatasetViewInternal) -> None:
        """
        Create a new DatasetView wrapper.

        Parameters
        ----------
        internal : DatasetViewInternal
            The internal Rust-side DatasetView object.

        """
        self._internal = internal

    @property
    def dataset(self) -> DatasetEntry:
        return DatasetEntry(self._internal.dataset)

    def schema(self) -> Schema:
        """
        Return the filtered schema for this view.

        The schema reflects any content filters applied to the view.

        Returns
        -------
        Schema
            The filtered schema.

        """
        from ._schema import Schema

        return Schema(self._internal.schema())

    def arrow_schema(self) -> pa.Schema:
        """
        Return the filtered Arrow schema for this view.

        Returns
        -------
        pa.Schema
            The filtered Arrow schema.

        """
        return self._internal.arrow_schema()

    def segment_ids(self) -> list[str]:
        """
        Return the segment IDs for this view.

        If segment filters have been applied, only matching segments are returned.

        Returns
        -------
        list[str]
            The list of segment IDs.

        """
        return self._internal.segment_ids()

    def segment_table(
        self,
        join_meta: TableEntry | datafusion.DataFrame | None = None,
        join_key: str = "rerun_segment_id",
    ) -> datafusion.DataFrame:
        """
        Return the segment table as a DataFusion DataFrame.

        Parameters
        ----------
        join_meta
            Optional metadata table or DataFrame to join with the segment table.
        join_key
            The column name to use for joining, defaults to "rerun_segment_id".

        Returns
        -------
        The segment metadata table.

        """

        segment_table_df = self.dataset.segment_table().df()

        filtered_segment_ids = self._internal.filtered_segment_ids
        if filtered_segment_ids is not None:
            from datafusion import col, functions as F, literal

            segment_table_df = segment_table_df.filter(
                F.in_list(col("rerun_segment_id"), [literal(seg) for seg in filtered_segment_ids])
            )

        if join_meta is not None:
            if isinstance(join_meta, TableEntry):
                join_meta = join_meta.reader()

            if join_key not in segment_table_df.schema().names:
                raise ValueError(f"Dataset segment table must contain join_key column '{join_key}'.")
            if join_key not in join_meta.schema().names:
                raise ValueError(f"join_meta must contain join_key column '{join_key}'.")

            meta_join_key = join_key + "_meta"
            join_meta = join_meta.with_column_renamed(join_key, meta_join_key)

            return segment_table_df.join(
                join_meta,
                left_on=join_key,
                right_on=meta_join_key,
                how="left",
            ).drop(meta_join_key)

        return segment_table_df

    @deprecated("This method is deprecated and will be removed in a future release")
    def download_segment(self, segment_id: str) -> Recording:
        """
        Download a specific segment from the dataset.

        Parameters
        ----------
        segment_id : str
            The ID of the segment to download.

        Returns
        -------
        Recording
            The downloaded recording.

        """

        return self.dataset.download_segment(segment_id)

    def reader(
        self,
        *,
        index: str | None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: IndexValuesLike | None = None,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this DatasetView.

        Returns a DataFusion DataFrame.

        Parameters
        ----------
        index
            The index (timeline) to use for the view.
            Pass `None` to read only static data.
        include_semantically_empty_columns
            Whether to include columns that are semantically empty.
        include_tombstone_columns
            Whether to include tombstone columns.
        fill_latest_at
            Whether to fill null values with the latest valid data.
        using_index_values
            If provided, specifies the exact index values to sample for all segments.
            Can be a numpy array (datetime64[ns] or int64), a pyarrow Array, or a sequence.
            Use with `fill_latest_at=True` to populate rows with the most recent data.

        Returns
        -------
        A DataFusion DataFrame.

        """
        return self._internal.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )

    def filter_segments(self, segment_ids: Sequence[str] | datafusion.DataFrame) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Filters are composed: if this view already has a segment filter,
        the result is the intersection of both filters.

        Parameters
        ----------
        segment_ids : Sequence[str] | datafusion.DataFrame
            Either a list of segment ID strings or a DataFusion DataFrame
            with a column named 'rerun_segment_id'.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        """
        if isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(self, exprs: Sequence[str]) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Parameters
        ----------
        exprs : Sequence[str]
            Entity path expressions.

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths.

        """
        return DatasetView(self._internal.filter_contents(list(exprs)))

    def __repr__(self) -> str:
        """Return a string representation of the DatasetView."""

        dataset_str = str(self.dataset)

        filter_segment_ids = self._internal.filtered_segment_ids
        if filter_segment_ids is not None:
            segment_str = f"{len(filter_segment_ids)} segments"
        else:
            segment_str = "all segments"

        content_filters = self._internal.content_filters
        if content_filters:
            content_str = f"content_filters={content_filters!r}"
        else:
            content_str = "no content filter"

        return f"DatasetView({dataset_str}, {segment_str}, {content_str})"


class TableEntry(Entry[TableEntryInternal]):
    """
    A table entry in the catalog.

    Note: this object acts as a table provider for DataFusion.
    """

    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

        return self._internal.__datafusion_table_provider__()

    def reader(self) -> datafusion.DataFrame:
        """Registers the table with the DataFusion context and return a DataFrame."""

        return self._internal.reader()

    def to_arrow_reader(self) -> pa.RecordBatchReader:
        """Convert this table to a [`pyarrow.RecordBatchReader`][]."""

        return self._internal.to_arrow_reader()

    @property
    def storage_url(self) -> str:
        """The table's storage URL."""

        return self._internal.storage_url

    def arrow_schema(self) -> pa.Schema:
        """Returns the Arrow schema of the table."""

        return self.reader().schema()

    # ---

    def append(
        self,
        batches: _BatchesType | None = None,
        **named_params: Any,
    ) -> None:
        """
        Append to the Table.

        Parameters
        ----------
        batches
            Arrow data to append to the table. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).
        **named_params
            Each named parameter corresponds to a column in the table.

        """
        self._write(batches, named_params, TableInsertMode.APPEND)

    def overwrite(
        self,
        batches: _BatchesType | None = None,
        **named_params: Any,
    ) -> None:
        """
        Overwrite the Table with new data.

        Parameters
        ----------
        batches
            Arrow data to overwrite the table with. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).

        **named_params
            Each named parameter corresponds to a column in the table.

        """
        self._write(batches, named_params, TableInsertMode.OVERWRITE)

    def upsert(
        self,
        batches: _BatchesType | None = None,
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
            Arrow data to upsert into the table. Can be a RecordBatchReader, a single RecordBatch, a list of
            RecordBatches, or a list of lists of RecordBatches (as returned by `datafusion.DataFrame.collect()`).
        **named_params
            Each named parameter corresponds to a column in the table

        """
        self._write(batches, named_params, TableInsertMode.REPLACE)

    def _write(
        self,
        batches: _BatchesType | None,
        named_params: dict[str, Any],
        insert_mode: TableInsertMode,
    ) -> None:
        """Internal helper that implements append/overwrite/upsert."""
        if batches is not None and len(named_params) > 0:
            raise TypeError("Cannot pass both batches and named parameters. Use one or the other.")

        if batches is not None:
            self._write_batches(batches, insert_mode=insert_mode)
        else:
            self._write_named_params(named_params, insert_mode=insert_mode)

    def _write_batches(
        self,
        batches: _BatchesType,
        insert_mode: TableInsertMode,
    ) -> None:
        """Internal helper to write batches to the table."""
        # If already a RecordBatchReader, pass it directly
        if isinstance(batches, RecordBatchReader):
            self._internal.write_batches(batches, insert_mode=insert_mode)
            return

        flat_batches = self._flatten_batches(batches)
        if len(flat_batches) == 0:
            return
        schema = flat_batches[0].schema
        reader = RecordBatchReader.from_batches(schema, flat_batches)
        self._internal.write_batches(reader, insert_mode=insert_mode)

    def _flatten_batches(
        self,
        batches: pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]],
    ) -> list[pa.RecordBatch]:
        """Flatten batches to a list of RecordBatches."""
        if isinstance(batches, pa.RecordBatch):
            return [batches]

        result = []
        for item in batches:
            if isinstance(item, pa.RecordBatch):
                result.append(item)
            elif isinstance(item, Sequence):
                result.extend(item)
            else:
                raise TypeError(f"Unexpected type: {type(item)}")
        return result

    def _write_named_params(
        self,
        named_params: dict[str, Any],
        insert_mode: TableInsertMode,
    ) -> None:
        """Internal helper to write named parameters to the table."""
        batch = self._python_objects_to_record_batch(self.arrow_schema(), named_params)
        if batch is not None:
            reader = RecordBatchReader.from_batches(batch.schema, [batch])
            self._internal.write_batches(reader, insert_mode=insert_mode)

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
