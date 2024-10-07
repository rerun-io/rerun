from typing import Optional, Sequence

import pyarrow as pa

from .types import AnyColumn, ComponentLike, ViewContentsLike

class ControlColumnDescriptor:
    """A control-level column such as `RowId`."""

class ControlColumnSelector:
    """A selector for a control column."""

    @staticmethod
    def row_id() -> ControlColumnSelector: ...

class IndexColumnDescriptor:
    """A column containing the index values for when the component data was updated."""

class IndexColumnSelector:
    """A selector for an index column."""

    def __init__(self, timeline: str): ...

class ComponentColumnDescriptor:
    """A column containing the component data."""

    def with_dictionary_encoding(self) -> ComponentColumnDescriptor: ...

class ComponentColumnSelector:
    """A selector for a component column."""

    def __new__(cls, entity_path: str, component_type: ComponentLike): ...
    def with_dictionary_encoding(self) -> ComponentColumnSelector: ...

class Schema:
    """The schema representing all columns in a [`Recording`][]."""

    def control_columns(self) -> list[ControlColumnDescriptor]: ...
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

    def select(self, *args: AnyColumn, columns: Optional[Sequence[AnyColumn]] = None) -> pa.RecordBatchReader: ...

class Recording:
    """A single recording."""

    def schema(self) -> Schema: ...
    def view(self, index: str, contents: ViewContentsLike) -> RecordingView: ...

class RRDArchive:
    """An archive loaded from an RRD, typically containing 1 or more recordings or blueprints."""

    def num_recordings(self) -> int: ...
    def all_recordings(self) -> list[Recording]: ...

def load_recording(filename: str) -> Recording:
    """
    Load a single recording from an RRD.

    Will raise a `ValueError` if the file does not contain exactly one recording.

    Parameters
    ----------
    filename : str
        The path to the file to load.

    """
    ...

def load_archive(filename: str) -> RRDArchive:
    """
    Load a rerun archive file from disk.

    Parameters
    ----------
    filename : str
        The path to the file to load.

    """
    ...
