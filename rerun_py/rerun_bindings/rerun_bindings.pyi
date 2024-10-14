import os
from typing import Optional, Sequence

import pyarrow as pa

from .types import AnyColumn, ComponentLike, IndexValuesLike, ViewContentsLike

class IndexColumnDescriptor:
    """A column containing the index values for when the component data was updated."""

class IndexColumnSelector:
    """A selector for an index column."""

    def __init__(self, index: str): ...

class ComponentColumnDescriptor:
    """A column containing the component data."""

    def with_dictionary_encoding(self) -> ComponentColumnDescriptor: ...

class ComponentColumnSelector:
    """A selector for a component column."""

    def __init__(self, entity_path: str, component: ComponentLike): ...
    def with_dictionary_encoding(self) -> ComponentColumnSelector: ...

class Schema:
    """The schema representing all columns in a [`Recording`][]."""

    def index_columns(self) -> list[IndexColumnDescriptor]: ...
    def component_columns(self) -> list[ComponentColumnDescriptor]: ...
    def column_for(self, entity_path: str, component: ComponentLike) -> Optional[ComponentColumnDescriptor]: ...

class RecordingView:
    """
    A view of a recording restricted to a given index, containing a specific set of entities and components.

    Can only be created by calling `view(...)` on a `Recording`.

    The only type of index currently supported is the name of a timeline.

    The view will only contain a single row for each unique value of the index. If the same entity / component pair
    was logged to a given index multiple times, only the most recent row will be included in the view, as determined
    by the `row_id` column. This will generally be the last value logged, as row_ids are guaranteed to be monotonically
    increasing when data is sent from a single process.
    """

    def filter_range_sequence(self, start: int, end: int) -> RecordingView:
        """Filter the view to only include data between the given index sequence numbers."""
        ...

    def filter_range_seconds(self, start: float, end: float) -> RecordingView:
        """Filter the view to only include data between the given index time values."""
        ...

    def filter_range_nanos(self, start: int, end: int) -> RecordingView:
        """Filter the view to only include data between the given index time values."""
        ...

    def filter_index_values(self, values: IndexValuesLike) -> RecordingView:
        """
        Filter the view to only include data at the given index values.

        The index values returned will be the intersection between the provided values and the
        original index values.

        This requires index values to be a precise match.  Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.
        """
        ...

    def filter_is_not_null(self, column: AnyColumn) -> RecordingView:
        """
        Filter the view to only include rows where the given column is not null.

        This corresponds to rows for index values where this component was provided to Rerun explicitly
        via `.log()` or `.send_columns()`.
        """
        ...

    def using_index_values(self, values: IndexValuesLike) -> RecordingView:
        """
        Replace the index in the view with the provided values.

        The output view will always have the same number of rows as the provided values, even if
        those rows are empty.  Use with `.fill_latest_at()` to populate these rows with the most
        recent data.

        This requires index values to be a precise match.  Index values in Rerun are
        represented as i64 sequence counts or nanoseconds. This API does not expose an interface
        in floating point seconds, as the numerical conversion would risk false mismatches.
        """
        ...

    def fill_latest_at(self) -> RecordingView:
        """Populate any null values in a row with the latest valid data on the timeline."""
        ...

    def select(self, *args: AnyColumn, columns: Optional[Sequence[AnyColumn]] = None) -> pa.RecordBatchReader: ...

class Recording:
    """A single recording."""

    def schema(self) -> Schema: ...
    def view(self, *, index: str, contents: ViewContentsLike) -> RecordingView: ...
    def recording_id(self) -> str: ...
    def application_id(self) -> str: ...

class RRDArchive:
    """An archive loaded from an RRD, typically containing 1 or more recordings or blueprints."""

    def num_recordings(self) -> int: ...
    def all_recordings(self) -> list[Recording]: ...

def load_recording(path_to_rrd: str | os.PathLike) -> Recording:
    """
    Load a single recording from an RRD.

    Will raise a `ValueError` if the file does not contain exactly one recording.

    Parameters
    ----------
    path_to_rrd : str
        The path to the file to load.

    """
    ...

def load_archive(path_to_rrd: str | os.PathLike) -> RRDArchive:
    """
    Load a rerun archive file from disk.

    Parameters
    ----------
    path_to_rrd : str
        The path to the file to load.

    """
    ...
