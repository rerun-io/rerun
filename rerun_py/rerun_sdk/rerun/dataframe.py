from __future__ import annotations

from collections import defaultdict
from typing import TYPE_CHECKING, Any, Optional

import pyarrow as pa
from rerun_bindings import (
    ComponentColumnDescriptor as ComponentColumnDescriptor,
    ComponentColumnSelector as ComponentColumnSelector,
    IndexColumnDescriptor as IndexColumnDescriptor,
    IndexColumnSelector as IndexColumnSelector,
    Recording as Recording,
    RecordingView as RecordingView,
    RRDArchive as RRDArchive,
    Schema as Schema,
    load_archive as load_archive,
    load_recording as load_recording,
)
from rerun_bindings.types import (
    AnyColumn as AnyColumn,
    AnyComponentColumn as AnyComponentColumn,
    ViewContentsLike as ViewContentsLike,
)

from ._baseclasses import ComponentColumn, ComponentDescriptor
from ._send_columns import TimeColumnLike, send_columns

if TYPE_CHECKING:
    from .recording_stream import RecordingStream

SORBET_INDEX_NAME = b"rerun:index_name"
SORBET_ENTITY_PATH = b"rerun:entity_path"
SORBET_ARCHETYPE_NAME = b"rerun:archetype"
SORBET_COMPONENT = b"rerun:component"
SORBET_COMPONENT_TYPE = b"rerun:component_type"
RERUN_KIND = b"rerun:kind"
RERUN_KIND_CONTROL = b"control"
RERUN_KIND_INDEX = b"index"


class RawIndexColumn(TimeColumnLike):
    def __init__(self, metadata: dict[bytes, bytes], col: pa.Array) -> None:
        self.metadata = metadata
        self.col = col

    def timeline_name(self) -> str:
        name = self.metadata.get(SORBET_INDEX_NAME, "unknown")
        if isinstance(name, bytes):
            name = name.decode("utf-8")
        return name

    def as_arrow_array(self) -> pa.Array:
        return self.col


class RawComponentBatchLike(ComponentColumn):
    def __init__(self, metadata: dict[bytes, bytes], col: pa.Array) -> None:
        self.metadata = metadata
        self.col = col

    def component_descriptor(self) -> ComponentDescriptor:
        kwargs = {}
        if SORBET_ARCHETYPE_NAME in self.metadata:
            kwargs["archetype"] = self.metadata[SORBET_ARCHETYPE_NAME].decode("utf-8")
        if SORBET_COMPONENT_TYPE in self.metadata:
            kwargs["component_type"] = self.metadata[SORBET_COMPONENT_TYPE].decode("utf-8")
        if SORBET_COMPONENT in self.metadata:
            kwargs["component"] = self.metadata[SORBET_COMPONENT].decode("utf-8")

        if "component_type" not in kwargs:
            kwargs["component_type"] = "Unknown"

        return ComponentDescriptor(**kwargs)

    def as_arrow_array(self) -> pa.Array:
        return self.col


def send_record_batch(batch: pa.RecordBatch, rec: Optional[RecordingStream] = None) -> None:
    """Coerce a single pyarrow `RecordBatch` to Rerun structure."""

    indexes = []
    data: defaultdict[str, list[Any]] = defaultdict(list)
    archetypes: defaultdict[str, set[Any]] = defaultdict(set)
    for col in batch.schema:
        metadata = col.metadata or {}
        if metadata.get(RERUN_KIND) == RERUN_KIND_CONTROL:
            continue
        if SORBET_INDEX_NAME in metadata or metadata.get(RERUN_KIND) == RERUN_KIND_INDEX:
            if SORBET_INDEX_NAME not in metadata:
                metadata[SORBET_INDEX_NAME] = col.name
            indexes.append(RawIndexColumn(metadata, batch.column(col.name)))
        else:
            entity_path = metadata.get(SORBET_ENTITY_PATH, col.name.split(":")[0])
            if isinstance(entity_path, bytes):
                entity_path = entity_path.decode("utf-8")
            data[entity_path].append(RawComponentBatchLike(metadata, batch.column(col.name)))
            if SORBET_ARCHETYPE_NAME in metadata:
                archetypes[entity_path].add(metadata[SORBET_ARCHETYPE_NAME].decode("utf-8"))

    for entity_path, columns in data.items():
        send_columns(
            entity_path,
            indexes,
            columns,
            # This is fine, send_columns will handle the conversion
            recording=rec,  # NOLINT
        )


def send_dataframe(df: pa.RecordBatchReader | pa.Table, rec: Optional[RecordingStream] = None) -> None:
    """Coerce a pyarrow `RecordBatchReader` or `Table` to Rerun structure."""
    if isinstance(df, pa.Table):
        df = df.to_reader()

    for batch in df:
        send_record_batch(batch, rec)
