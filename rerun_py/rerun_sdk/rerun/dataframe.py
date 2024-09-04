from __future__ import annotations
from typing import Optional, Type

from rerun import bindings
from rerun._baseclasses import ComponentMixin


class Schema:
    """The schema representing all columns in a [`Dataset`][]."""

    def __init__(self, storage: bindings.PySchema) -> None:
        self.storage = storage

    def control_columns(self) -> list[bindings.PyControlColumn]:
        return self.storage.control_columns()

    def time_columns(self) -> list[bindings.PyTimeColumn]:
        return self.storage.time_columns()

    def component_columns(self) -> list[bindings.PyComponentColumn]:
        return self.storage.component_columns()

    def column_for(self, entity_path: str, component: str | Type[ComponentMixin]) -> Optional[bindings.PyColumn]:
        if not isinstance(component, str):
            component = component._BATCH_TYPE._ARROW_TYPE._TYPE_NAME

        for col in self.component_columns():
            if col.matches(entity_path, component):
                return col


class Dataset:
    """A single dataset from an RRD, representing a Recording or a Blueprint."""

    def __init__(self, storage: bindings.PyChunkStore) -> None:
        self.storage = storage

    def schema(self) -> bindings.PySchema:
        """The schema of the dataset."""
        return Schema(self.storage.schema())

    def range_query(self, entity_path_expr: str, pov: bindings.PyControlColumn) -> list[pa.RecordBatch]:
        """Execute a range query on the dataset."""
        return self.storage.range_query(entity_path_expr, pov)


class Archive:
    """An archive containing all the data stores in an RRD file."""

    def __init__(self, storage: bindings.PyRRDArchive) -> None:
        self.storage = storage

    def num_recordings(self) -> int:
        """The number of recordings in the archive."""
        return self.storage.num_recordings()

    def all_recordings(self) -> list[Dataset]:
        """The recordings in the archive."""
        return [Dataset(r) for r in self.storage.all_recordings()]


def load_recording(filename: str) -> Dataset:
    """
    Load a rerun data file from disk.

    :param filename: The path to the file to load.
    :return: A dictionary of stores in the file.
    """
    archive = load_archive(filename)

    if archive.num_recordings() != 1:
        raise ValueError(f"Expected exactly one recording in the archive, got {archive.num_recordings()}")

    recordings = archive.all_recordings()

    return Dataset(recordings[0])


def load_archive(filename: str) -> Archive:
    """
    Load a rerun archive file from disk.

    :param filename: The path to the file to load.
    :return: A dictionary of stores in the file.
    """
    stores = bindings.load_rrd(filename)

    return Archive(stores)
