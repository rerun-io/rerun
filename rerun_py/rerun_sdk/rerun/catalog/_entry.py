from __future__ import annotations

from abc import ABC
from collections.abc import Sequence
from enum import Enum
from typing import TYPE_CHECKING, Any, Generic, TypeAlias, TypeVar

import pyarrow as pa
from pyarrow import RecordBatchReader
from typing_extensions import deprecated

from rerun_bindings import (
    DatasetEntryInternal,
    DatasetViewInternal,
    TableEntryInternal,
    TableInsertModeInternal,
)

from . import EntryId

#: Type alias for supported batch input types for TableEntry write methods.
_BatchesType: TypeAlias = (
    RecordBatchReader | pa.RecordBatch | Sequence[pa.RecordBatch] | Sequence[Sequence[pa.RecordBatch]]
)

if TYPE_CHECKING:
    from datetime import datetime

    import datafusion

    from rerun.recording import Recording

    from . import (
        CatalogClient,
        ComponentColumnDescriptor,
        ComponentColumnSelector,
        EntryKind,
        IndexColumnSelector,
        IndexConfig,
        IndexingResult,
        IndexValuesLike,
        RegistrationHandle,
        Schema,
        VectorDistanceMetric,
    )


# TODO(#12612): switch to `StrEnum` when we drop Python 3.10
class OnDuplicateSegmentLayer(str, Enum):
    """
    How to handle duplicate segment layers when registering recordings to a dataset.

    Attributes:
        ERROR: Raise an error if a segment layer with the same name already exists.
        SKIP: Skip the duplicate segment layer.
        REPLACE: Replace the existing segment layer with the new one.

    """

    ERROR = "error"
    SKIP = "skip"
    REPLACE = "replace"


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

    def set_name(self, name: str) -> None:
        """
        Change the name of this entry.

        **Note**: entry names must be unique within the catalog. If the new name is not unique, an error will be raised.

        Parameters
        ----------
        name : str
            New name for the entry

        """
        self._internal.set_name(name)

    @deprecated("Entry.update() is deprecated. Use Entry.set_name() instead.")
    def update(self, *, name: str | None = None) -> None:
        """
        Update this entry's properties.

        .. deprecated::
            Use :meth:`set_name` instead.

        Parameters
        ----------
        name : str | None
            New name for the entry

        """

        if name is not None:
            self._internal.set_name(name)

    def __eq__(self, other: object) -> bool:
        """
        Compare this entry to another object.

        Supports comparison with `str` and `EntryId` to enable the following patterns:
        ```py
        "entry_name" in client.entries()
        entry_id in client.entries()
        ```
        """
        match other:
            case Entry():
                return self.id == other.id

            case str():
                return self.name == other

            case EntryId():
                return self.id == other

            case _:
                return NotImplemented

    # Make it explicit that `Entries` are view objects for which hashing cannot sensibly be implemented
    __hash__ = None  # type: ignore[assignment]


class DatasetEntry(Entry[DatasetEntryInternal]):
    """A dataset entry in the catalog."""

    @property
    def manifest_url(self) -> str:
        """Return the dataset manifest URL."""

        return self._internal.manifest_url

    def arrow_schema(self) -> pa.Schema:
        """Return the Arrow schema of the data contained in the dataset."""

        return self._internal.arrow_schema()

    def register_blueprint(self, uri: str, set_default: bool = True) -> None:
        """
        Register an existing .rbl visible to the server.

        By default, also set this blueprint as default.
        """

        blueprint_dataset = self.blueprint_dataset()

        if blueprint_dataset is None:
            raise LookupError("a blueprint dataset is not configured for this dataset")

        segment_id = blueprint_dataset.register(uri).wait().segment_ids[0]

        if set_default:
            self.set_default_blueprint(segment_id)

    def blueprints(self) -> list[str]:
        """Lists all blueprints currently registered with this dataset."""

        blueprint_dataset = self.blueprint_dataset()
        if blueprint_dataset is None:
            return []
        else:
            return blueprint_dataset.segment_ids()

    def set_default_blueprint(self, blueprint_name: str | None) -> None:
        """Set an already-registered blueprint as default for this dataset."""

        return self._internal.set_default_blueprint_segment_id(blueprint_name)

    def default_blueprint(self) -> str | None:
        """Return the name currently set blueprint."""

        return self._internal.default_blueprint_segment_id()

    def blueprint_dataset(self) -> DatasetEntry | None:
        """The associated blueprint dataset, if any."""

        ds = self._internal.blueprint_dataset()
        return None if ds is None else DatasetEntry(ds)

    def schema(self) -> Schema:
        """Return the schema of the data contained in the dataset."""
        from ._schema import Schema

        return Schema(self._internal.schema())

    def segment_ids(self) -> list[str]:
        """Returns a list of segment IDs for the dataset."""

        return self._internal.segment_ids()

    def segment_table(
        self,
        join_meta: TableEntry | datafusion.DataFrame | None = None,
        join_key: str = "rerun_segment_id",
    ) -> datafusion.DataFrame:
        """
        Return the segment table as a DataFusion DataFrame.

        The segment table contains metadata about each segment in the dataset,
        including segment IDs, layer names, storage URLs, and size information.

        Parameters
        ----------
        join_meta
            Optional metadata table or DataFrame to join with the segment table.
            If a `TableEntry` is provided, it will be converted to a DataFrame
            using `reader()`.
        join_key
            The column name to use for joining, defaults to "rerun_segment_id".
            Both the segment table and `join_meta` must contain this column.

        Returns
        -------
        datafusion.DataFrame
            The segment metadata table, optionally joined with `join_meta`.

        """
        segment_table_df = self._internal.segment_table()

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

    def manifest(self, include_diagnostic_data: bool = False) -> datafusion.DataFrame:
        """
        Return the dataset manifest as a DataFusion DataFrame.

        Parameters
        ----------
        include_diagnostic_data:
            Include diagnostic data in the manifest. That may include rows that correspond to layers which failed
            registration, were deleted, or are in pending states.

            !!! note

                Diagnostic data is subject to change in any release and should not be relied on for production.

        """

        from datafusion import col

        df = self._internal.manifest()

        if not include_diagnostic_data:
            df = df.filter(col("rerun_registration_status") == "done").drop("rerun_registration_status")

        return df

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
            The start selected time for the segment.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        end: int | datetime | None
            The end selected time for the segment.
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

    def register(
        self,
        recording_uri: str | Sequence[str],
        *,
        layer_name: str | Sequence[str] = "base",
        on_duplicate: OnDuplicateSegmentLayer = OnDuplicateSegmentLayer.ERROR,
    ) -> RegistrationHandle:
        """
        Register RRD URIs to the dataset and return a handle to track progress.

        This method initiates the registration of recordings to the dataset, and returns
        a handle that can be used to wait for completion or iterate over results.

        Parameters
        ----------
        recording_uri:
            The URI(s) of the RRD(s) to register. Can be a single URI string or a sequence of URIs.

        layer_name:
            The layer(s) to which the recordings will be registered to.
            Can be a single layer name (applied to all recordings) or a sequence of layer names
            (must match the length of `recording_uri`).
            Defaults to `"base"`.

        on_duplicate:
            How to handle the cases where the segment id and layer name already exist in the dataset?
            Defaults to `OnDuplicateSegmentLayer.ERROR`.

        Returns
        -------
        RegistrationHandle
            A handle to track and wait on the registration tasks.

        """
        from ._registration_handle import RegistrationHandle

        if isinstance(recording_uri, str):
            recording_uris = [recording_uri]
        else:
            recording_uris = list(recording_uri)

        if isinstance(layer_name, str):
            layer_names = [layer_name] * len(recording_uris)
        else:
            layer_names = list(layer_name)
            if len(layer_names) != len(recording_uris):
                raise ValueError("`layer_name` must be the same length as `recording_uri`")

        return RegistrationHandle(
            self._internal.register(recording_uris, recording_layers=layer_names, on_duplicate=on_duplicate)
        )

    def unregister(
        self,
        *,
        segments_to_drop: str | Sequence[str],
        layers_to_drop: str | Sequence[str],
        force: bool = False,
    ) -> None:
        """
        Unregisters segments and layers from the dataset.

        Excluding IO errors, this will always succeed as long the target dataset exists.
        Corollary: unregistering data that doesn't exist is a no-op.

        This method acts as a *product* filter:
        * empty `segments_to_drop` + empty `layers_to_drop`: invalid argument error
        * empty `segments_to_drop` + non-empty `layers_to_drop`: remove specified layers for *all* segments
        * non-empty `segments_to_drop` + empty `layers_to_drop`: remove *all* layers for specified segments
        * non-empty `segments_to_drop` + non-empty `layers_to_drop`: delete *all* specified layers for *all* specified segments

        Parameters
        ----------
        segments_to_drop: list[str]
            The segment IDs to drop. All of them if empty.
            The final filter will be the *outer product* of this and `layers_to_drop`.

        layers_to_drop: list[str]
            The layer names to drop. All of them if empty.
            The final filter will be the *outer product* of this and `segments_to_drop`.

        force: bool
            If true, deletion will go through regardless of the segments/layers' current statuses.
            This is only useful in the very specific, catatrophic scenario where the contents of the
            task queue were lost and some tasks are now stuck in `status=pending` forever.
            Do not use this unless you know exactly what you're doing.

        """
        if isinstance(segments_to_drop, str):
            segments_to_drop = [segments_to_drop]
        else:
            segments_to_drop = list(segments_to_drop)

        if isinstance(layers_to_drop, str):
            layers_to_drop = [layers_to_drop]
        else:
            layers_to_drop = list(layers_to_drop)

        self._internal.unregister(segments_to_drop=segments_to_drop, layers_to_drop=layers_to_drop, force=force)

    def register_prefix(
        self,
        recordings_prefix: str,
        layer_name: str | None = None,
        on_duplicate: OnDuplicateSegmentLayer = OnDuplicateSegmentLayer.ERROR,
    ) -> RegistrationHandle:
        """
        Register all RRDs under a given prefix to the dataset and return a handle to track progress.

        A prefix is a directory-like path in an object store (e.g. an S3 bucket or ABS container).
        All RRDs that are recursively found under the given prefix will be registered to the dataset.

        This method initiates the registration of the recordings to the dataset, and returns
        a handle that can be used to wait for completion or iterate over results.

        Parameters
        ----------
        recordings_prefix: str
            The prefix under which to register all RRDs.

        layer_name: Optional[str]
            The layer to which the recordings will be registered to.
            If `None`, this defaults to `"base"`.

        on_duplicate:
            How to handle the cases where the segment id and layer name already exist in the dataset?
            Defaults to `OnDuplicateSegmentLayer.ERROR`.

        Returns
        -------
        A handle to track and wait on the registration tasks.

        """
        from ._registration_handle import RegistrationHandle

        if layer_name is None:
            layer_name = "base"

        return RegistrationHandle(self._internal.register_prefix(recordings_prefix, layer_name, on_duplicate))

    def download_segment(self, segment_id: str) -> Recording:
        """Download a segment from the dataset."""

        return self._internal.download_segment(segment_id)

    def filter_segments(self, segment_ids: str | Sequence[str] | datafusion.DataFrame) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Parameters
        ----------
        segment_ids
            A segment ID string, a list of segment ID strings, or a DataFusion DataFrame
            with a column named 'rerun_segment_id'. When passing a DataFrame,
            if there are additional columns, they will be ignored.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        Examples
        --------
        ```python
        # Filter to a single segment
        view = dataset.filter_segments("recording_0")

        # Filter to specific segments
        view = dataset.filter_segments(["recording_0", "recording_1"])

        # Filter using a DataFrame
        good_segments = segment_table.filter(col("success"))
        view = dataset.filter_segments(good_segments)

        # Read data from the filtered view
        df = view.reader(index="timeline")
        ```

        """

        import datafusion

        if isinstance(segment_ids, str):
            segment_ids = [segment_ids]
        elif isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(self, exprs: str | Sequence[str]) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Parameters
        ----------
        exprs : str | Sequence[str]
            Entity path expression or list of entity path expressions. Passing `[]` results in filtering out all
            contents.

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths.

        Examples
        --------
        ```python
        # Filter to a single entity path
        view = dataset.filter_contents("/points/**")

        # Filter to specific entity paths
        view = dataset.filter_contents(["/points/**"])

        # Exclude certain paths
        view = dataset.filter_contents(["/points/**", "-/text/**"])

        # Chain with segment filters
        view = dataset.filter_segments(["recording_0"]).filter_contents("/points/**")
        ```

        """

        if isinstance(exprs, str):
            exprs = [exprs]

        return DatasetView(self._internal.filter_contents(list(exprs)))

    def reader(
        self,
        index: str | None,
        *,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: dict[str, IndexValuesLike] | datafusion.DataFrame | IndexValuesLike | None = None,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this dataset.

        Returns a DataFusion DataFrame.

        Server side filters
        -------------------

        The returned DataFrame supports server side filtering for both `rerun_segment_id`
        and the index (timeline) column, which can greatly improve performance. For
        example, the following filters will effectively be handled by the Rerun server.

        ```python
        dataset.reader(index="real_time").filter(col("rerun_segment_id") == "aabbccddee")
        dataset.reader(index="real_time").filter(col("real_time") == "1234567890")
        dataset.reader(index="real_time").filter(
            (col("rerun_segment_id") == "aabbccddee") & (col("real_time") == "1234567890")
        )
        ```

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
            If provided, specifies the exact index values to sample per segment.
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

    def create_fts_search_index(
        self,
        *,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        time_index: IndexColumnSelector,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        """Create a full-text search index on the given column."""

        return self._internal.create_fts_search_index(
            column=column,
            time_index=time_index,
            store_position=store_position,
            base_tokenizer=base_tokenizer,
        )

    def create_vector_search_index(
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

        return self._internal.create_vector_search_index(
            column=column,
            time_index=time_index,
            target_partition_num_rows=target_partition_num_rows,
            num_sub_vectors=num_sub_vectors,
            distance_metric=distance_metric,
        )

    def list_search_indexes(self) -> list[IndexingResult]:
        """List all user-defined indexes in this dataset."""

        return self._internal.list_search_indexes()

    def delete_search_indexes(
        self,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> list[IndexConfig]:
        """Deletes all user-defined indexes for the specified column."""

        return self._internal.delete_search_indexes(column)

    def search_fts(
        self,
        query: str,
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
    ) -> datafusion.DataFrame:
        """Search the dataset using a full-text search query."""

        return self._internal.search_fts(query, column)

    def search_vector(
        self,
        query: Any,  # VectorLike
        column: str | ComponentColumnSelector | ComponentColumnDescriptor,
        top_k: int,
    ) -> datafusion.DataFrame:
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

    def get_index_ranges(self) -> datafusion.DataFrame:
        """Returns the range bounds of all indexes per segment."""
        view = self.filter_contents(["/**"])
        return view.get_index_ranges()


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

        The segment table contains metadata about each segment in the dataset,
        including segment IDs, layer names, storage URLs, and size information.

        Only segments matching this view's filters are included.

        Parameters
        ----------
        join_meta
            Optional metadata table or DataFrame to join with the segment table.
            If a `TableEntry` is provided, it will be converted to a DataFrame
            using `reader()`.
        join_key
            The column name to use for joining, defaults to "rerun_segment_id".
            Both the segment table and `join_meta` must contain this column.

        Returns
        -------
        datafusion.DataFrame
            The segment metadata table, optionally joined with `join_meta`.

        """
        segment_table_df = self.dataset.segment_table(join_meta, join_key)

        filtered_segment_ids = self._internal.filtered_segment_ids
        if filtered_segment_ids is not None:
            from datafusion import col, functions as F, literal

            segment_table_df = segment_table_df.filter(
                F.in_list(col("rerun_segment_id"), [literal(seg) for seg in filtered_segment_ids])
            )

        return segment_table_df

    def reader(
        self,
        index: str | None,
        *,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        using_index_values: dict[str, IndexValuesLike] | datafusion.DataFrame | IndexValuesLike | None = None,
        fill_latest_at: bool = False,
    ) -> datafusion.DataFrame:
        """
        Create a reader over this DatasetView.

        Returns a DataFusion DataFrame.

        Server side filters
        -------------------

        The returned DataFrame supports server side filtering for both `rerun_segment_id`
        and the index (timeline) column, which can greatly improve performance. For
        example, the following filters will effectively be handled by the Rerun server.

        ```python
        dataset.reader(index="real_time").filter(col("rerun_segment_id") == "aabbccddee")
        dataset.reader(index="real_time").filter(col("real_time") == "1234567890")
        dataset.reader(index="real_time").filter(
            (col("rerun_segment_id") == "aabbccddee") & (col("real_time") == "1234567890")
        )
        ```

        Parameters
        ----------
        index
            The index (timeline) to use for the view.
            Pass `None` to read only static data.
        include_semantically_empty_columns
            Whether to include columns that are semantically empty.
        include_tombstone_columns
            Whether to include tombstone columns.
        using_index_values
            If a dict is provided, keys are segment IDs and values are the index values
            to sample for that segment (per-segment semantics).
            If a DataFrame is provided, it must have 'rerun_segment_id' and index columns.
            Use with `fill_latest_at=True` to populate rows with the most recent data.
        fill_latest_at
            Whether to fill null values with the latest valid data.

        Returns
        -------
        A DataFusion DataFrame.

        """
        import logging

        import datafusion

        available_segments = set() if using_index_values is None else set(self._internal.segment_ids())

        index_values_dict = None
        match using_index_values:
            case None:
                pass

            case df if isinstance(df, datafusion.DataFrame):
                index_values_dict = self._dataframe_to_index_values_dict(df, index)

            case dict() as d:
                index_values_dict = d

            case _ as index_vals:
                # Scalar IndexValuesLike: apply the same indices to all segments
                index_values_dict = dict.fromkeys(available_segments, index_vals)

        if index_values_dict is not None:
            requested_segments = set(index_values_dict.keys())
            missing_segments = requested_segments - available_segments

            if missing_segments:
                logging.warning(
                    f"Index values for the following inexistent or filtered segments "
                    f"were ignored: {', '.join(sorted(missing_segments))}"
                )

            valid_segments = requested_segments - missing_segments
            view = self._internal.filter_segments([*valid_segments])
        else:
            view = self._internal

        return view.reader(
            index=index,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=index_values_dict,
        )

    def filter_segments(self, segment_ids: str | Sequence[str] | datafusion.DataFrame) -> DatasetView:
        """
        Return a new DatasetView filtered to the given segment IDs.

        Parameters
        ----------
        segment_ids
            A segment ID string, a list of segment ID strings, or a DataFusion DataFrame
            with a column named 'rerun_segment_id'. When passing a DataFrame,
            if there are additional columns, they will be ignored.

        Returns
        -------
        DatasetView
            A new view filtered to the given segments.

        Examples
        --------
        ```python
        # Filter to a single segment
        view = dataset.filter_segments("recording_0")

        # Filter to specific segments
        view = dataset.filter_segments(["recording_0", "recording_1"])

        # Filter using a DataFrame
        good_segments = segment_table.filter(col("success"))
        view = dataset.filter_segments(good_segments)

        # Read data from the filtered view
        df = view.reader(index="timeline")
        ```

        """

        import datafusion

        if isinstance(segment_ids, str):
            segment_ids = [segment_ids]
        elif isinstance(segment_ids, datafusion.DataFrame):
            segment_ids = segment_ids.select("rerun_segment_id").to_pydict()["rerun_segment_id"]

        return DatasetView(self._internal.filter_segments(list(segment_ids)))

    def filter_contents(self, exprs: str | Sequence[str]) -> DatasetView:
        """
        Return a new DatasetView filtered to the given entity paths.

        Entity path expressions support wildcards:
        - `"/points/**"` matches all entities under /points
        - `"-/text/**"` excludes all entities under /text

        Parameters
        ----------
        exprs : str | Sequence[str]
            Entity path expression or list of entity path expressions. Passing `[]` results in filtering out all
            contents.

        Returns
        -------
        DatasetView
            A new view filtered to the matching entity paths.

        Examples
        --------
        ```python
        # Filter to a single entity path
        view = dataset.filter_contents("/points/**")

        # Filter to specific entity paths
        view = dataset.filter_contents(["/points/**"])

        # Exclude certain paths
        view = dataset.filter_contents(["/points/**", "-/text/**"])

        # Chain with segment filters
        view = dataset.filter_segments(["recording_0"]).filter_contents("/points/**")
        ```

        """
        if isinstance(exprs, str):
            exprs = [exprs]

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

    def get_index_ranges(self) -> datafusion.DataFrame:
        """Returns the range bounds of all indexes per segment."""
        exprs = ["rerun_segment_id"]
        for index_col in self.schema().index_columns():
            exprs.append(f"{index_col.name}:start")
            exprs.append(f"{index_col.name}:end")

        return self.segment_table().select(*exprs)

    def _dataframe_to_index_values_dict(
        self, df: datafusion.DataFrame, index: str | None
    ) -> dict[str, IndexValuesLike]:
        """Convert a DataFrame with segment_id + index columns to a dict."""

        import datafusion as dfn

        if "rerun_segment_id" not in df.schema().names:
            raise ValueError("using_index_values DataFrame must have a 'rerun_segment_id' column")

        if index is None:
            raise ValueError("index must be provided when using_index_values is a DataFrame")

        if index not in df.schema().names:
            raise ValueError(f"using_index_values DataFrame must have an '{index}' column")

        table = pa.table(
            df.aggregate(
                ["rerun_segment_id"], [dfn.functions.array_agg(dfn.col(index), order_by=dfn.col(index)).alias(index)]
            )
        )

        # Group by segment_id
        segment_id_col = table.column("rerun_segment_id")
        index_col = table.column(index)

        return {segment_id_col[i].as_py(): index_col[i].values.to_numpy() for i in range(table.num_rows)}


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
        self._write(batches, named_params, TableInsertModeInternal.APPEND)

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
        self._write(batches, named_params, TableInsertModeInternal.OVERWRITE)

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
        self._write(batches, named_params, TableInsertModeInternal.REPLACE)

    def _write(
        self,
        batches: _BatchesType | None,
        named_params: dict[str, Any],
        insert_mode: TableInsertModeInternal,
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
        insert_mode: TableInsertModeInternal,
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
        insert_mode: TableInsertModeInternal,
    ) -> None:
        """Internal helper to write named parameters to the table."""
        batch = _python_objects_to_record_batch(self.arrow_schema(), named_params)
        if batch is not None:
            reader = RecordBatchReader.from_batches(batch.schema, [batch])
            self._internal.write_batches(reader, insert_mode=insert_mode)


def _python_objects_to_record_batch(schema: pa.Schema, named_params: dict[str, Any]) -> pa.RecordBatch:
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
                error = (
                    f"Columns have mismatched number of rows. "
                    f"Column '{name}' has {len(cast_value)} rows but expected {expected_len}."
                )

                if pa.types.is_list(field.type) or pa.types.is_large_list(field.type):
                    error += (
                        f" Hint: For single-row list-typed columns, wrap your list in another list: "
                        f"{name}=[[...]] instead of {name}=[...]"  # NOLINT
                    )

                raise ValueError(error)

    if expected_len is None or expected_len == 0:
        return

    columns = []
    for field in schema:
        if field.name in cast_params:
            columns.append(cast_params[field.name])
        else:
            columns.append(pa.array([None] * expected_len, type=field.type))

    return pa.RecordBatch.from_arrays(columns, schema=schema)
