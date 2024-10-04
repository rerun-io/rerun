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
    """The schema representing all columns in a [`Dataset`][]."""

    def control_columns(self) -> list[ControlColumnDescriptor]: ...
    def index_columns(self) -> list[IndexColumnDescriptor]: ...
    def component_columns(self) -> list[ComponentColumnDescriptor]: ...
    def column_for(self, entity_path: str, component: ComponentLike) -> Optional[ComponentColumnDescriptor]: ...

class RecordingView:
    """A view of a recording on a timeline, containing a specific set of entities and components."""

    def filter_range_sequence(self, start: int, end: int) -> RecordingView: ...
    def filter_range_seconds(self, start: float, end: float) -> RecordingView: ...
    def filter_range_nanos(self, start: int, end: int) -> RecordingView: ...
    def select(self, columns: Sequence[AnyColumn]) -> list[pa.RecordBatch]: ...

class Recording:
    """A single recording."""

    def schema(self) -> Schema: ...
    def view(self, timeline: str, contents: ViewContentsLike) -> RecordingView: ...

class RRDArchive:
    """An archive loaded from an RRD, typically containing 1 or more recordings or blueprints."""

    def num_recordings(self) -> int: ...
    def all_recordings(self) -> list[Recording]: ...

def load_recording(filename: str) -> Recording:
    """
    Load a single recording from an RRD.

    Will raise a `ValueError` if the file does not contain a single recording.

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
