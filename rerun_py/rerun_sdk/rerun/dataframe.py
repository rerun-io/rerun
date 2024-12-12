from __future__ import annotations

from collections import defaultdict
from typing import Optional

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
    ComponentLike as ComponentLike,
    ViewContentsLike as ViewContentsLike,
)

from ._baseclasses import ComponentColumn, ComponentDescriptor
from ._log import IndicatorComponentBatch
from ._send_columns import TimeColumnLike, send_columns
from .recording_stream import RecordingStream

SORBET_INDEX_NAME = b"sorbet.index_name"
SORBET_ENTITY_PATH = b"sorbet.path"
SORBET_ARCHETYPE_NAME = b"sorbet.semantic_family"
SORBET_ARCHETYPE_FIELD = b"sorbet.logical_type"
SORBET_COMPONENT_NAME = b"sorbet.semantic_type"


class RawIndexColumn(TimeColumnLike):
    def __init__(self, metadata: dict, col: pa.Array):
        self.metadata = metadata
        self.col = col

    def timeline_name(self) -> str:
        return self.metadata[SORBET_INDEX_NAME].decode("utf-8")

    def as_arrow_array(self) -> pa.Array:
        return self.col


class RawComponentBatchLike(ComponentColumn):
    def __init__(self, metadata: dict, col: pa.Array):
        self.metadata = metadata
        self.col = col

    def component_descriptor(self) -> ComponentDescriptor:
        kwargs = {}
        if SORBET_ARCHETYPE_NAME in self.metadata:
            kwargs["archetype_name"] = "rerun.archetypes" + self.metadata[SORBET_ARCHETYPE_NAME].decode("utf-8")
        if SORBET_COMPONENT_NAME in self.metadata:
            kwargs["component_name"] = "rerun.components." + self.metadata[SORBET_COMPONENT_NAME].decode("utf-8")
        if SORBET_ARCHETYPE_FIELD in self.metadata:
            kwargs["archetype_field_name"] = self.metadata[SORBET_ARCHETYPE_FIELD].decode("utf-8")

        if "component_name" not in kwargs:
            kwargs["component_name"] = "Unknown"

        return ComponentDescriptor(**kwargs)

    def as_arrow_array(self) -> pa.Array:
        return self.col


def send_record_batch(batch: pa.RecordBatch, rec: Optional[RecordingStream] = None):
    """Coerce a single pyarrow `RecordBatch` to Rerun structure."""

    indexes = []
    data = defaultdict(list)
    archetypes = defaultdict(set)
    for col in batch.schema:
        metadata = col.metadata or {}
        if SORBET_INDEX_NAME in metadata:
            indexes.append(RawIndexColumn(metadata, batch.column(col.name)))
        else:
            entity_path = metadata.get(SORBET_ENTITY_PATH, col.name.split(":")[0])
            if isinstance(entity_path, bytes):
                entity_path = entity_path.decode("utf-8")
            data[entity_path].append(RawComponentBatchLike(metadata, batch.column(col.name)))
            if SORBET_ARCHETYPE_NAME in metadata:
                archetypes[entity_path].add(metadata[SORBET_ARCHETYPE_NAME].decode("utf-8"))
    for entity_path, archetypes in archetypes.items():
        for archetype in archetypes:
            data[entity_path].append(IndicatorComponentBatch("rerun.archetypes." + archetype))

    for entity_path, columns in data.items():
        send_columns(
            entity_path,
            indexes,
            columns,
            recording=rec,
        )


def send_dataframe(df: pa.RecordBatchReader | pa.Table, rec: Optional[RecordingStream] = None):
    """Coerce a pyarrow `RecordBatchReader` or `Table` to Rerun structure."""
    if isinstance(df, pa.Table):
        df = df.to_reader()

    for batch in df:
        send_record_batch(batch, rec)
