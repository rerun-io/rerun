from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Any, Generic, TypeVar

from rerun_bindings import DatasetEntryInternal, TableEntryInternal

if TYPE_CHECKING:
    from datetime import datetime

    import datafusion

    from . import (
        CatalogClient,
        DataframeQueryView,
        DataFusionTable,
        EntryId,
        EntryKind,
        IndexConfig,
        IndexingResult,
        Schema,
        Tasks,
        VectorDistanceMetric,
    )


if TYPE_CHECKING:
    from datetime import datetime

    import pyarrow as pa

    from rerun.dataframe import ComponentColumnDescriptor, ComponentColumnSelector, IndexColumnSelector, Recording


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

    def default_blueprint_partition_id(self) -> str | None:
        """The default blueprint partition ID for this dataset, if any."""

        return self._internal.default_blueprint_partition_id()

    def set_default_blueprint_partition_id(self, partition_id: str | None) -> None:
        """
        Set the default blueprint partition ID for this dataset.

        Pass `None` to clear the bluprint. This fails if the change cannot be made to the remote server.
        """

        return self._internal.set_default_blueprint_partition_id(partition_id)

    def schema(self) -> Schema:
        """Return the schema of the data contained in the dataset."""

        return self._internal.schema()

    def partition_ids(self) -> list[str]:
        """Returns a list of partitions IDs for the dataset."""

        return self._internal.partition_ids()

    def partition_table(self) -> DataFusionTable:
        """Return the partition table as a Datafusion table provider."""

        return self._internal.partition_table()

    def manifest(self) -> DataFusionTable:
        """Return the dataset manifest as a Datafusion table provider."""

        return self._internal.manifest()

    def partition_url(  # noqa: PLR0917
        self,
        partition_id: str,
        timeline: str | None = None,
        start: datetime | int | None = None,
        end: datetime | int | None = None,
    ) -> str:
        """
        Return the URL for the given partition.

        Parameters
        ----------
        partition_id: str
            The ID of the partition to get the URL for.

        timeline: str | None
            The name of the timeline to display.

        start: int | datetime | None
            The start time for the partition.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        end: int | datetime | None
            The end time for the partition.
            Integer for ticks, or datetime/nanoseconds for timestamps.

        Examples
        --------
        # With ticks
        >>> start_tick, end_time = 0, 10
        >>> dataset.partition_url("some_id", "log_tick", start_tick, end_time)

        # With timestamps
        >>> start_time, end_time = datetime.now() - timedelta(seconds=4), datetime.now()
        >>> dataset.partition_url("some_id", "real_time", start_time, end_time)

        Returns
        -------
        str
            The URL for the given partition.

        """

        return self._internal.partition_url(partition_id, timeline, start, end)

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
        partition_id: str
            The partition ID of the registered RRD.

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

    def download_partition(self, partition_id: str) -> Recording:
        """Download a partition from the dataset."""

        return self._internal.download_partition(partition_id)

    def dataframe_query_view(
        self,
        *,
        index: str | None,
        contents: Any,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
    ) -> DataframeQueryView:
        """
        Create a [`DataframeQueryView`][rerun.catalog.DataframeQueryView] of the recording according to a particular index and content specification.

        The only type of index currently supported is the name of a timeline, or `None` (see below
        for details).

        The view will only contain a single row for each unique value of the index
        that is associated with a component column that was included in the view.
        Component columns that are not included via the view contents will not
        impact the rows that make up the view. If the same entity / component pair
        was logged to a given index multiple times, only the most recent row will be
        included in the view, as determined by the `row_id` column. This will
        generally be the last value logged, as row_ids are guaranteed to be
        monotonically increasing when data is sent from a single process.

        If `None` is passed as the index, the view will contain only static columns (among those
        specified) and no index columns. It will also contain a single row per partition.

        Parameters
        ----------
        index : str | None
            The index to use for the view. This is typically a timeline name. Use `None` to query static data only.
        contents : ViewContentsLike
            The content specification for the view.

            This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
            specifying multiple content-expressions and a respective list of components to select within
            that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
        include_semantically_empty_columns : bool, optional
            Whether to include columns that are semantically empty, by default `False`.

            Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
        include_tombstone_columns : bool, optional
            Whether to include tombstone columns, by default `False`.

            Tombstone columns are components used to represent clears. However, even without the clear
            tombstone columns, the view will still apply the clear semantics when resolving row contents.

        Returns
        -------
        DataframeQueryView
            The view of the dataset.

        """

        return self._internal.dataframe_query_view(
            index=index,
            contents=contents,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
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
        column : AnyComponentColumn
            The component column to create the index on.
        time_index : IndexColumnSelector
            Which timeline this index will map to.
        target_partition_num_rows : int | None
            The target size (in number of rows) for each partition.
            The underlying indexer (lance) will pick a default when no value
            is specified - today this is 8192. It will also cap the
            maximum number of partitions independently of this setting - currently
            4096.
        num_sub_vectors : int
            The number of sub-vectors to use when building the index.
        distance_metric : VectorDistanceMetricLike
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


class TableEntry(Entry[TableEntryInternal]):
    """
    A table entry in the catalog.

    Note: this object acts as a table provider for DataFusion.
    """

    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

        return self._internal.__datafusion_table_provider__()

    def df(self) -> datafusion.DataFrame:
        """Registers the table with the DataFusion context and return a DataFrame."""

        return self._internal.df()

    def to_arrow_reader(self) -> pa.RecordBatchReader:
        """Convert this table to a [`pyarrow.RecordBatchReader`][]."""

        return self._internal.to_arrow_reader()

    @property
    def storage_url(self) -> str:
        """The table's storage URL."""

        return self._internal.storage_url
