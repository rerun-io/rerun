from typing import Optional, Sequence

import pyarrow as pa

from .types import AnyColumn, ComponentLike

class ControlColumnDescriptor:
    """A control-level column such as `RowId`."""

class ControlColumnSelector:
    """A selector for a control column."""

    @staticmethod
    def row_id() -> ControlColumnSelector: ...

class TimeColumnDescriptor:
    """A column containing the time values for when the component data was updated."""

class TimeColumnSelector:
    """A selector for a time column."""

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
    def time_columns(self) -> list[TimeColumnDescriptor]: ...
    def component_columns(self) -> list[ComponentColumnDescriptor]: ...
    def column_for(self, entity_path: str, component: ComponentLike) -> Optional[ComponentColumnDescriptor]: ...

class TimeRange:
    """A time range with a start and end time."""

    @staticmethod
    def everything() -> TimeRange: ...
    @staticmethod
    def seconds(start: float, end: float) -> TimeRange: ...
    @staticmethod
    def nanos(start: int, end: int) -> TimeRange: ...
    @staticmethod
    def sequence(start: int, end: int) -> TimeRange: ...

class Dataset:
    """A single dataset from an RRD, representing a Recording or a Blueprint."""

    def schema(self) -> Schema: ...
    def query(
        self,
        entity_path_expr: str,
        timeline: str,
        time_range: TimeRange,
        query_columns: Sequence[AnyColumn],
    ) -> list[pa.RecordBatch]: ...

class RRDArchive:
    """An archive loaded from an RRD, typically containing 1 or more recordings or blueprints."""

    def num_recordings(self) -> int: ...
    def all_recordings(self) -> list[Dataset]: ...

def load_recording(filename: str) -> Dataset:
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
