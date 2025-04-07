from __future__ import annotations

import os
from collections.abc import Iterable, Iterator, Sequence
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Optional, Self

import pyarrow as pa

from .types import (
    AnyColumn,
    AnyComponentColumn,
    ComponentLike,
    IndexValuesLike,
    ViewContentsLike,
)

class IndexColumnDescriptor:
    """
    The descriptor of an index column.

    Index columns contain the index values for when the data was updated. They
    generally correspond to Rerun timelines.

    Column descriptors are used to describe the columns in a
    [`Schema`][rerun.dataframe.Schema]. They are read-only. To select an index
    column, use [`IndexColumnSelector`][rerun.dataframe.IndexColumnSelector].
    """

    @property
    def name(self) -> str:
        """
        The name of the index.

        This property is read-only.
        """

    @property
    def is_static(self) -> bool:
        """Part of generic ColumnDescriptor interface: always False for Index."""

class IndexColumnSelector:
    """
    A selector for an index column.

    Index columns contain the index values for when the data was updated. They
    generally correspond to Rerun timelines.
    """

    def __init__(self, index: str) -> None:
        """
        Create a new `IndexColumnSelector`.

        Parameters
        ----------
        index : str
            The name of the index to select. Usually the name of a timeline.

        """

    @property
    def name(self) -> str:
        """
        The name of the index.

        This property is read-only.
        """

class ComponentColumnDescriptor:
    """
    The descriptor of a component column.

    Component columns contain the data for a specific component of an entity.

    Column descriptors are used to describe the columns in a
    [`Schema`][rerun.dataframe.Schema]. They are read-only. To select a component
    column, use [`ComponentColumnSelector`][rerun.dataframe.ComponentColumnSelector].
    """

    @property
    def entity_path(self) -> str:
        """
        The entity path.

        This property is read-only.
        """

    @property
    def component_name(self) -> str:
        """
        The component name.

        This property is read-only.
        """

    @property
    def is_static(self) -> bool:
        """
        Whether the column is static.

        This property is read-only.
        """

class ComponentColumnSelector:
    """
    A selector for a component column.

    Component columns contain the data for a specific component of an entity.
    """

    def __init__(self, entity_path: str, component: ComponentLike) -> None:
        """
        Create a new `ComponentColumnSelector`.

        Parameters
        ----------
        entity_path : str
            The entity path to select.
        component : ComponentLike
            The component to select

        """
    @property
    def entity_path(self) -> str:
        """
        The entity path.

        This property is read-only.
        """

    @property
    def component_name(self) -> str:
        """
        The component name.

        This property is read-only.
        """

class VectorDistanceMetric(Enum):  # type: ignore[misc]
    """Which distance metric for use for vector index."""

    L2: VectorDistanceMetric
    COSINE: VectorDistanceMetric
    DOT: VectorDistanceMetric
    HAMMING: VectorDistanceMetric

class Schema:
    """
    The schema representing a set of available columns.

    Can be returned by [`Recording.schema()`][rerun.dataframe.Recording.schema] or
    [`RecordingView.schema()`][rerun.dataframe.RecordingView.schema].
    """

    def __iter__(self) -> Iterator[IndexColumnDescriptor | ComponentColumnDescriptor]:
        """Iterate over all the column descriptors in the schema."""

    def index_columns(self) -> list[IndexColumnDescriptor]:
        """Return a list of all the index columns in the schema."""

    def component_columns(self) -> list[ComponentColumnDescriptor]:
        """Return a list of all the component columns in the schema."""

    def column_for(self, entity_path: str, component: ComponentLike) -> Optional[ComponentColumnDescriptor]:
        """
        Look up the column descriptor for a specific entity path and component.

        Parameters
        ----------
        entity_path : str
            The entity path to look up.
        component : ComponentLike
            The component to look up.

        Returns
        -------
        Optional[ComponentColumnDescriptor]
            The column descriptor, if it exists.

        """

class RecordingView:
    """
    A view of a recording restricted to a given index, containing a specific set of entities and components.

    See [`Recording.view(…)`][rerun.dataframe.Recording.view] for details on how to create a `RecordingView`.

    Note: `RecordingView` APIs never mutate the underlying view. Instead, they
    always return new views with the requested modifications applied.

    The view will only contain a single row for each unique value of the index
    that is associated with a component column that was included in the view.
    Component columns that are not included via the view contents will not
    impact the rows that make up the view. If the same entity / component pair
    was logged to a given index multiple times, only the most recent row will be
    included in the view, as determined by the `row_id` column. This will
    generally be the last value logged, as row_ids are guaranteed to be
    monotonically increasing when data is sent from a single process.
    """

    def schema(self) -> Schema:
        """
        The schema describing all the columns available in the view.

        This schema will only contain the columns that are included in the view via
        the view contents.
        """

    def filter_range_sequence(self, start: int, end: int) -> RecordingView:
        """
        Filter the view to only include data between the given index sequence numbers.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a sequential index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_range_secs(self, start: float, end: float) -> RecordingView:
        """
        Filter the view to only include data between the given index values expressed as seconds.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a temporal index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_range_nanos(self, start: int, end: int) -> RecordingView:
        """
        Filter the view to only include data between the given index values expressed as nanoseconds.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a temporal index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_index_values(self, values: IndexValuesLike) -> RecordingView:
        """
        Filter the view to only include data at the provided index values.

        The index values returned will be the intersection between the provided values and the
        original index values.

        This requires index values to be a precise match. Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.

        Parameters
        ----------
        values : IndexValuesLike
            The index values to filter by.

        Returns
        -------
        RecordingView
            A new view containing only the data at the specified index values.

            The original view will not be modified.

        """

    def filter_is_not_null(self, column: AnyComponentColumn) -> RecordingView:
        """
        Filter the view to only include rows where the given component column is not null.

        This corresponds to rows for index values where this component was provided to Rerun explicitly
        via `.log()` or `.send_columns()`.

        Parameters
        ----------
        column : AnyComponentColumn
            The component column to filter by.

        Returns
        -------
        RecordingView
            A new view containing only the data where the specified component column is not null.

            The original view will not be modified.

        """

    def using_index_values(self, values: IndexValuesLike) -> RecordingView:
        """
        Replace the index in the view with the provided values.

        The output view will always have the same number of rows as the provided values, even if
        those rows are empty. Use with [`.fill_latest_at()`][rerun.dataframe.RecordingView.fill_latest_at]
        to populate these rows with the most recent data.

        This requires index values to be a precise match. Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.

        Parameters
        ----------
        values : IndexValuesLike
            The index values to use.

        Returns
        -------
        RecordingView
            A new view containing the provided index values.

            The original view will not be modified.

        """

    def fill_latest_at(self) -> RecordingView:
        """
        Populate any null values in a row with the latest valid data according to the index.

        Returns
        -------
        RecordingView
            A new view with the null values filled in.

            The original view will not be modified.

        """

    def select(self, *args: AnyColumn, columns: Optional[Sequence[AnyColumn]] = None) -> pa.RecordBatchReader:
        """
        Select the columns from the view.

        If no columns are provided, all available columns will be included in
        the output.

        The selected columns do not change the rows that are included in the
        view. The rows are determined by the index values and the components
        that were included in the view contents, or can be overridden with
        [`.using_index_values()`][rerun.dataframe.RecordingView.using_index_values].

        If a column was not provided with data for a given row, it will be
        `null` in the output.

        The output is a [`pyarrow.RecordBatchReader`][] that can be used to read
        out the data.

        Parameters
        ----------
        *args : AnyColumn
            The columns to select.
        columns : Optional[Sequence[AnyColumn]], optional
            Alternatively the columns to select can be provided as a sequence.

        Returns
        -------
        pa.RecordBatchReader
            A reader that can be used to read out the selected data.

        """

    def select_static(self, *args: AnyColumn, columns: Optional[Sequence[AnyColumn]] = None) -> pa.RecordBatchReader:
        """
        Select only the static columns from the view.

        Because static data has no associated index values it does not cause a
        row to be generated in the output. If your view only contains static data
        this method allows you to select it without needing to provide index values.

        This method will always return a single row.

        Any non-static columns that are included in the selection will generate a warning
        and produce empty columns.


        Parameters
        ----------
        *args : AnyColumn
            The columns to select.
        columns : Optional[Sequence[AnyColumn]], optional
            Alternatively the columns to select can be provided as a sequence.

        Returns
        -------
        pa.RecordBatchReader
            A reader that can be used to read out the selected data.

        """

class Recording:
    """
    A single Rerun recording.

    This can be loaded from an RRD file using [`load_recording()`][rerun.dataframe.load_recording].

    A recording is a collection of data that was logged to Rerun. This data is organized
    as a column for each index (timeline) and each entity/component pair that was logged.

    You can examine the [`.schema()`][rerun.dataframe.Recording.schema] of the recording to see
    what data is available, or create a [`RecordingView`][rerun.dataframe.RecordingView] to
    to retrieve the data.
    """

    def schema(self) -> Schema:
        """The schema describing all the columns available in the recording."""

    def view(
        self,
        *,
        index: str,
        contents: ViewContentsLike,
        include_semantically_empty_columns: bool = False,
        include_indicator_columns: bool = False,
        include_tombstone_columns: bool = False,
    ) -> RecordingView:
        """
        Create a [`RecordingView`][rerun.dataframe.RecordingView] of the recording according to a particular index and content specification.

        The only type of index currently supported is the name of a timeline.

        The view will only contain a single row for each unique value of the index
        that is associated with a component column that was included in the view.
        Component columns that are not included via the view contents will not
        impact the rows that make up the view. If the same entity / component pair
        was logged to a given index multiple times, only the most recent row will be
        included in the view, as determined by the `row_id` column. This will
        generally be the last value logged, as row_ids are guaranteed to be
        monotonically increasing when data is sent from a single process.

        Parameters
        ----------
        index : str
            The index to use for the view. This is typically a timeline name.
        contents : ViewContentsLike
            The content specification for the view.

            This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
            specifying multiple content-expressions and a respective list of components to select within
            that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
        include_semantically_empty_columns : bool, optional
            Whether to include columns that are semantically empty, by default `False`.

            Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
        include_indicator_columns : bool, optional
            Whether to include indicator columns, by default `False`.

            Indicator columns are components used to represent the presence of an archetype within an entity.
        include_tombstone_columns : bool, optional
            Whether to include tombstone columns, by default `False`.

            Tombstone columns are components used to represent clears. However, even without the clear
            tombstone columns, the view will still apply the clear semantics when resolving row contents.

        Returns
        -------
        RecordingView
            The view of the recording.

        Examples
        --------
        All the data in the recording on the timeline "my_index":
        ```python
        recording.view(index="my_index", contents="/**")
        ```

        Just the Position3D components in the "points" entity:
        ```python
        recording.view(index="my_index", contents={"points": "Position3D"})
        ```

        """

    def recording_id(self) -> str:
        """The recording ID of the recording."""

    def application_id(self) -> str:
        """The application ID of the recording."""

class RRDArchive:
    """
    An archive loaded from an RRD.

    RRD archives may include 1 or more recordings or blueprints.
    """

    def num_recordings(self) -> int:
        """The number of recordings in the archive."""
    def all_recordings(self) -> list[Recording]:
        """All the recordings in the archive."""

def load_recording(path_to_rrd: str | os.PathLike[str]) -> Recording:
    """
    Load a single recording from an RRD file.

    Will raise a `ValueError` if the file does not contain exactly one recording.

    Parameters
    ----------
    path_to_rrd : str | os.PathLike[str]
        The path to the file to load.

    Returns
    -------
    Recording
        The loaded recording.

    """

def load_archive(path_to_rrd: str | os.PathLike[str]) -> RRDArchive:
    """
    Load a rerun archive from an RRD file.

    Parameters
    ----------
    path_to_rrd : str | os.PathLike[str]
        The path to the file to load.

    Returns
    -------
    RRDArchive
        The loaded archive.

    """

# AI generated stubs for `PyRecordingStream` related class and functions
# TODO(#9187): this will be entirely replaced with `RecordingStream` is itself written in Rust

class PyRecordingStream:
    def is_forked_child(self) -> bool:
        """
        Determine if this stream is operating in the context of a forked child process.

        This means the stream was created in the parent process. It now exists in the child
        process by way of fork, but it is effectively a zombie since its batcher and sink
        threads would not have been copied.

        Calling operations such as flush or set_sink will result in an error.
        """

class PyMemorySinkStorage:
    def concat_as_bytes(self, concat: Optional[PyMemorySinkStorage] = None) -> bytes:
        """
        Concatenate the contents of the [`MemorySinkStorage`] as bytes.

        Note: This will do a blocking flush before returning!
        """
    def num_msgs(self) -> int:
        """
        Count the number of pending messages in the [`MemorySinkStorage`].

        This will do a blocking flush before returning!
        """
    def drain_as_bytes(self) -> bytes:
        """
        Drain all messages logged to the [`MemorySinkStorage`] and return as bytes.

        This will do a blocking flush before returning!
        """

class PyBinarySinkStorage:
    def read(self, *, flush: bool = True) -> bytes:
        """
        Read the bytes from the binary sink.

        If `flush` is `true`, the sink will be flushed before reading.
        """
    def flush(self) -> None:
        """Flush the binary sink manually."""

#
# init
#

def new_recording(
    application_id: str,
    recording_id: Optional[str] = None,
    make_default: bool = True,
    make_thread_default: bool = True,
    default_enabled: bool = True,
    send_properties: bool = True,
) -> PyRecordingStream:
    """Create a new recording stream."""

def new_blueprint(
    application_id: str,
    make_default: bool = True,
    make_thread_default: bool = True,
    default_enabled: bool = True,
) -> PyRecordingStream:
    """Create a new blueprint stream."""

def shutdown() -> None:
    """Shutdown the Rerun SDK."""

def cleanup_if_forked_child() -> None:
    """Cleans up internal state if this is the child of a forked process."""

def spawn(
    port: int = 9876,
    memory_limit: str = ...,
    hide_welcome_screen: bool = False,
    detach_process: bool = True,
    executable_name: str = ...,
    executable_path: Optional[str] = None,
    extra_args: list[str] = ...,
    extra_env: list[tuple[str, str]] = ...,
) -> None:
    """Spawn a new viewer."""

#
# recordings
#

def get_application_id(recording: Optional[PyRecordingStream] = None) -> Optional[str]:
    """Get the current recording stream's application ID."""

def get_recording_id(recording: Optional[PyRecordingStream] = None) -> Optional[str]:
    """Get the current recording stream's recording ID."""

def get_data_recording(recording: Optional[PyRecordingStream] = None) -> Optional[PyRecordingStream]:
    """Returns the currently active data recording in the global scope, if any; fallbacks to the specified recording otherwise, if any."""

def get_global_data_recording() -> Optional[PyRecordingStream]:
    """Returns the currently active data recording in the global scope, if any."""

def set_global_data_recording(recording: Optional[PyRecordingStream] = None) -> Optional[PyRecordingStream]:
    """
    Replaces the currently active recording in the global scope with the specified one.

    Returns the previous one, if any.
    """

def get_thread_local_data_recording() -> Optional[PyRecordingStream]:
    """Returns the currently active data recording in the thread-local scope, if any."""

def set_thread_local_data_recording(recording: Optional[PyRecordingStream] = None) -> Optional[PyRecordingStream]:
    """
    Replaces the currently active recording in the thread-local scope with the specified one.

    Returns the previous one, if any.
    """

def get_blueprint_recording(overrides: Optional[PyRecordingStream] = None) -> Optional[PyRecordingStream]:
    """Returns the currently active blueprint recording in the global scope, if any; fallbacks to the specified recording otherwise, if any."""

def get_global_blueprint_recording() -> Optional[PyRecordingStream]:
    """Returns the currently active blueprint recording in the global scope, if any."""

def set_global_blueprint_recording(recording: Optional[PyRecordingStream] = None) -> Optional[PyRecordingStream]:
    """
    Replaces the currently active recording in the global scope with the specified one.

    Returns the previous one, if any.
    """

def get_thread_local_blueprint_recording() -> Optional[PyRecordingStream]:
    """Returns the currently active blueprint recording in the thread-local scope, if any."""

def set_thread_local_blueprint_recording(
    recording: Optional[PyRecordingStream] = None,
) -> Optional[PyRecordingStream]:
    """
    Replaces the currently active recording in the thread-local scope with the specified one.

    Returns the previous one, if any.
    """

#
# sinks
#

def is_enabled(recording: Optional[PyRecordingStream] = None) -> bool:
    """Whether the recording stream enabled."""

def binary_stream(recording: Optional[PyRecordingStream] = None) -> Optional[PyBinarySinkStorage]:
    """Create a new binary stream sink, and return the associated binary stream."""

def connect_grpc(
    url: Optional[str],
    flush_timeout_sec: Optional[float] = ...,
    default_blueprint: Optional[PyMemorySinkStorage] = None,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Connect the recording stream to a remote Rerun Viewer on the given HTTP(S) URL."""

def connect_grpc_blueprint(
    url: Optional[str],
    make_active: bool,
    make_default: bool,
    blueprint_stream: PyRecordingStream,
) -> None:
    """Special binding for directly sending a blueprint stream to a connection."""

def save(
    path: str,
    default_blueprint: Optional[PyMemorySinkStorage] = None,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Save the recording stream to a file."""

def save_blueprint(path: str, blueprint_stream: PyRecordingStream) -> None:
    """Special binding for directly savings a blueprint stream to a file."""

def stdout(
    default_blueprint: Optional[PyMemorySinkStorage] = None,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Save to stdout."""

def memory_recording(recording: Optional[PyRecordingStream] = None) -> Optional[PyMemorySinkStorage]:
    """Create an in-memory rrd file."""

def set_callback_sink(
    callback: Callable[[bytes], Any],
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Set callback sink."""

def serve_grpc(
    grpc_port: Optional[int],
    server_memory_limit: str,
    default_blueprint: Optional[PyMemorySinkStorage] = None,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Spawn a gRPC server which an SDK or Viewer can connect to."""

def serve_web(
    open_browser: bool,
    web_port: Optional[int],
    grpc_port: Optional[int],
    server_memory_limit: str,
    default_blueprint: Optional[PyMemorySinkStorage] = None,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Serve a web-viewer."""

def disconnect(recording: Optional[PyRecordingStream] = None) -> None:
    """
    Disconnect from remote server (if any).

    Subsequent log messages will be buffered and either sent on the next call to `connect`,
    or shown with `show`.
    """

def flush(blocking: bool, recording: Optional[PyRecordingStream] = None) -> None:
    """Block until outstanding data has been flushed to the sink."""

#
# time
#

def set_time_sequence(
    timeline: str,
    sequence: int,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Set the current time for this thread as an integer sequence."""

def set_time_duration_nanos(
    timeline: str,
    nanos: int,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Set the current duration for this thread in nanoseconds."""

def set_time_timestamp_nanos_since_epoch(
    timeline: str,
    nanos: int,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Set the current time for this thread in nanoseconds."""

def send_recording_name(
    name: str,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Send the name of the recording."""

def send_recording_start_time_nanos(
    nanos: int,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Send the start time of the recording."""

def disable_timeline(
    timeline: str,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Clear time information for the specified timeline on this thread."""

def reset_time(recording: Optional[PyRecordingStream] = None) -> None:
    """Clear all timeline information on this thread."""

#
# log any
#

def log_arrow_msg(
    entity_path: str,
    components: dict[Any, Any],
    static_: bool,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Log an arrow message."""

def send_arrow_chunk(
    entity_path: str,
    timelines: dict[Any, Any],
    components: dict[Any, Any],
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """
    Directly send an arrow chunk to the recording stream.

    Params
    ------
    entity_path: `str`
        The entity path to log the chunk to.
    timelines: `Dict[str, arrow::Int64Array]`
        A dictionary mapping timeline names to their values.
    components: `Dict[str, arrow::ListArray]`
        A dictionary mapping component names to their values.
    """

def log_file_from_path(
    file_path: str | os.PathLike[str],
    entity_path_prefix: Optional[str] = None,
    static_: bool = False,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Log a file by path."""

def log_file_from_contents(
    file_path: str | os.PathLike[str],
    file_contents: bytes,
    entity_path_prefix: Optional[str] = None,
    static_: bool = False,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Log a file by contents."""

def send_blueprint(
    blueprint: PyMemorySinkStorage,
    make_active: bool = False,
    make_default: bool = True,
    recording: Optional[PyRecordingStream] = None,
) -> None:
    """Send a blueprint to the given recording stream."""

def send_recording(rrd: Recording, recording: Optional[PyRecordingStream] = None) -> None:
    """
    Send all chunks from a [`PyRecording`] to the given recording stream.

    .. warning::
        ⚠️ This API is experimental and may change or be removed in future versions! ⚠️
    """

#
# misc
#

def version() -> str:
    """Return a verbose version string."""

def get_app_url() -> str:
    """
    Get an url to an instance of the web-viewer.

    This may point to app.rerun.io or localhost depending on
    whether [`start_web_viewer_server()`] was called.
    """

def start_web_viewer_server(port: int) -> None:
    """Start a web server to host the run web-assets."""

def escape_entity_path_part(part: str) -> str:
    """Escape an entity path."""

def new_entity_path(parts: list[str]) -> str:
    """Create an entity path."""

def new_property_entity_path(parts: list[str]) -> str:
    """Create a property entity path."""

def asset_video_read_frame_timestamps_nanos(
    video_bytes_arrow_array: Any, media_type: Optional[str] = None
) -> list[int]:
    """
    Reads the timestamps of all frames in a video asset.

    Implementation note:
    On the Python side we start out with a pyarrow array of bytes. Converting it to
    Python `bytes` can be done with `to_pybytes` but this requires copying the data.
    So instead, we pass the arrow array directly.
    """

#####################################################################################################################
## CATALOG                                                                                                         ##
#####################################################################################################################

class EntryId:
    """A unique identifier for an entry in the catalog."""

    def __str__(self) -> str:
        """Return str(self)."""

class EntryKind:
    """The kinds of entries that can be stored in the catalog."""

    DATASET: EntryKind
    DATASET_VIEW: EntryKind
    TABLE: EntryKind
    TABLE_VIEW: EntryKind

    def __str__(self, /) -> str:
        """Return str(self)."""

class Entry:
    """An entry in the catalog."""

    @property
    def id(self) -> EntryId:
        """The entry's id."""

    @property
    def name(self) -> str:
        """The entry's name."""

    @property
    def catalog(self) -> CatalogClient:
        """The catalog client that this entry belongs to."""

    @property
    def kind(self) -> EntryKind:
        """The entry's kind."""

    @property
    def created_at(self) -> datetime:
        """The entry's creation date and time."""

    @property
    def updated_at(self) -> datetime:
        """The entry's last updated date and time."""

    def delete(self) -> None:
        """Delete this entry from the catalog."""

class Dataset(Entry):
    @property
    def manifest_url(self) -> str:
        """Return the dataset manifest URL."""

    def arrow_schema(self) -> pa.Schema:
        """Return the Arrow schema of the data contained in the dataset."""

    def partition_table(self) -> DataFusionTable:
        """Return the partition table as a Datafusion table provider."""

    def partition_url(self, partition_id: str) -> str:
        """Return the URL for the given partition."""

    def register(self, recording_uri: str) -> None:
        """Register a RRD URI to the dataset."""

    def download_partition(self, partition_id: str) -> Recording:
        """Download a partition from the dataset."""

    def dataframe_query_view(
        self,
        *,
        index: str,
        contents: Any,
        include_semantically_empty_columns: bool = False,
        include_indicator_columns: bool = False,
        include_tombstone_columns: bool = False,
    ) -> DataframeQueryView:
        """Create a view to run a dataframe query on the dataset."""

    def create_fts_index(
        self,
        *,
        column: ComponentColumnSelector,
        time_index: IndexColumnSelector,
        store_position: bool = False,
        base_tokenizer: str = "simple",
    ) -> None:
        """Create a full-text search index on the given column."""

    def create_vector_index(
        self,
        *,
        column: ComponentColumnSelector,
        time_index: IndexColumnSelector,
        num_partitions: int = 5,
        num_sub_vectors: int = 16,
        distance_metric: VectorDistanceMetric | str = ...,
    ) -> None:
        """Create a vector index on the given column."""

    def search_fts(
        self,
        query: str,
        column: ComponentColumnSelector,
    ) -> DataFusionTable:
        """Search the dataset using a full-text search query."""

    def search_vector(
        self,
        query: Any,  # VectorLike
        column: ComponentColumnSelector,
        top_k: int,
    ) -> DataFusionTable:
        """Search the dataset using a vector search query."""

class Table(Entry):
    """
    A table entry in the catalog.

    Note: this object acts as a table provider for DataFusion.
    """

    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

    def df(self) -> Any:
        """Registers the table with the DataFusion context and return a DataFrame."""

class DataframeQueryView:
    def filter_partition_id(self, partition_id: str, *args: Iterable[str]) -> Self:
        """Filter by one or more partition ids. All partition ids are included if not specified."""

    def filter_range_sequence(self, start: int, end: int) -> Self:
        """
        Filter the view to only include data between the given index sequence numbers.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a sequential index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_range_secs(self, start: float, end: float) -> Self:
        """
        Filter the view to only include data between the given index values expressed as seconds.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a temporal index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_range_nanos(self, start: int, end: int) -> Self:
        """
        Filter the view to only include data between the given index values expressed as nanoseconds.

        This range is inclusive and will contain both the value at the start and the value at the end.

        The view must be of a temporal index type to use this method.

        Parameters
        ----------
        start : int
            The inclusive start of the range.
        end : int
            The inclusive end of the range.

        Returns
        -------
        RecordingView
            A new view containing only the data within the specified range.

            The original view will not be modified.

        """

    def filter_index_values(self, values: IndexValuesLike) -> Self:
        """
        Filter the view to only include data at the provided index values.

        The index values returned will be the intersection between the provided values and the
        original index values.

        This requires index values to be a precise match. Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.

        Parameters
        ----------
        values : IndexValuesLike
            The index values to filter by.

        Returns
        -------
        RecordingView
            A new view containing only the data at the specified index values.

            The original view will not be modified.

        """

    def filter_is_not_null(self, column: AnyComponentColumn) -> Self:
        """
        Filter the view to only include rows where the given component column is not null.

        This corresponds to rows for index values where this component was provided to Rerun explicitly
        via `.log()` or `.send_columns()`.

        Parameters
        ----------
        column : AnyComponentColumn
            The component column to filter by.

        Returns
        -------
        RecordingView
            A new view containing only the data where the specified component column is not null.

            The original view will not be modified.

        """

    def using_index_values(self, values: IndexValuesLike) -> Self:
        """
        Replace the index in the view with the provided values.

        The output view will always have the same number of rows as the provided values, even if
        those rows are empty. Use with [`.fill_latest_at()`][rerun.dataframe.RecordingView.fill_latest_at]
        to populate these rows with the most recent data.

        This requires index values to be a precise match. Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.

        Parameters
        ----------
        values : IndexValuesLike
            The index values to use.

        Returns
        -------
        RecordingView
            A new view containing the provided index values.

            The original view will not be modified.

        """

    def fill_latest_at(self) -> Self:
        """
        Populate any null values in a row with the latest valid data according to the index.

        Returns
        -------
        RecordingView
            A new view with the null values filled in.

            The original view will not be modified.

        """

    def df(self) -> Any:
        """Register this view to the global DataFusion context and return a DataFrame."""

class CatalogClient:
    """Client for a remote Rerun catalog server."""

    def entries(self) -> list[Entry]:
        """Get a list of all entries in the catalog."""

    def get_dataset(self, name_or_id: str | EntryId) -> Dataset:
        """Get a dataset by name or id."""

    def create_dataset(self, name: str) -> Dataset:
        """Create a new dataset with the provided name."""

    def get_table(self, name_or_id: str | EntryId) -> Table:
        """
        Get a table by name or id.

        Note: the entry table is named `__entries`.
        """

    def entries_table(self) -> Table:
        """Get the entries table."""

    @property
    def ctx(self) -> Any:
        """The DataFusion context (if available)."""

class DataFusionTable:
    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

    def df(self) -> Any:
        """Register this view to the global DataFusion context and return a DataFrame."""

    @property
    def name(self) -> str:
        """Name of this table."""
