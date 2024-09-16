from typing import Optional, Sequence, Union

import pyarrow as pa
from rerun._baseclasses import ComponentMixin

AnyColumn = Union[
    ControlColumnDescriptor, TimeColumnDescriptor, ComponentColumnDescriptor, DictionaryComponentColumnDescriptor
]

class ControlColumnDescriptor:
    """A control-level column such as `RowId`."""

class TimeColumnDescriptor:
    """A column containing the time values for when the component data was updated."""

class ComponentColumnDescriptor:
    """A column containing the component data."""

    def as_dict(self) -> DictionaryComponentColumnDescriptor: ...

class DictionaryComponentColumnDescriptor:
    """A dictionary-encoded column containing the component data."""

class Schema:
    """The schema representing all columns in a [`Dataset`][]."""

    def control_columns(self) -> list[ControlColumnDescriptor]: ...
    def time_columns(self) -> list[TimeColumnDescriptor]: ...
    def component_columns(self) -> list[ComponentColumnDescriptor]: ...
    def column_for(
        self, entity_path: str, component: str | type[ComponentMixin]
    ) -> Optional[ComponentColumnDescriptor]: ...

class Dataset:
    """A single dataset from an RRD, representing a Recording or a Blueprint."""

    def schema(self) -> Schema: ...
    def range_query(
        self,
        entity_path_expr: str,
        pov: ComponentColumnDescriptor,
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
