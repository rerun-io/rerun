from __future__ import annotations

from collections import defaultdict
from typing import TYPE_CHECKING, Any

import pyarrow as pa

from ._baseclasses import ComponentColumn, ComponentDescriptor
from ._send_columns import TimeColumnLike, send_columns

if TYPE_CHECKING:
    from .recording_stream import RecordingStream

_SORBET_INDEX_NAME = b"rerun:index_name"
_SORBET_ENTITY_PATH = b"rerun:entity_path"
_SORBET_ARCHETYPE_NAME = b"rerun:archetype"
_SORBET_COMPONENT = b"rerun:component"
_SORBET_COMPONENT_TYPE = b"rerun:component_type"
_SORBET_IS_TABLE_INDEX = b"rerun:is_table_index"
_RERUN_KIND = b"rerun:kind"
_RERUN_KIND_CONTROL = b"control"
_RERUN_KIND_INDEX = b"index"


class _RawIndexColumn(TimeColumnLike):
    def __init__(self, metadata: dict[bytes, bytes], col: pa.Array) -> None:
        self.metadata = metadata
        self.col = col

    def timeline_name(self) -> str:
        name = self.metadata.get(_SORBET_INDEX_NAME, "unknown")
        if isinstance(name, bytes):
            name = name.decode("utf-8")
        return name

    def as_arrow_array(self) -> pa.Array:
        return self.col


class _RawComponentBatchLike(ComponentColumn):
    def __init__(self, metadata: dict[bytes, bytes], col: pa.Array) -> None:
        self.metadata = metadata
        self.col = col

    def component_descriptor(self) -> ComponentDescriptor:
        kwargs = {}
        if _SORBET_ARCHETYPE_NAME in self.metadata:
            kwargs["archetype"] = self.metadata[_SORBET_ARCHETYPE_NAME].decode("utf-8")
        if _SORBET_COMPONENT_TYPE in self.metadata:
            kwargs["component_type"] = self.metadata[_SORBET_COMPONENT_TYPE].decode("utf-8")
        if _SORBET_COMPONENT in self.metadata:
            kwargs["component"] = self.metadata[_SORBET_COMPONENT].decode("utf-8")

        if "component_type" not in kwargs:
            kwargs["component_type"] = "Unknown"

        return ComponentDescriptor(**kwargs)

    def as_arrow_array(self) -> pa.Array:
        return self.col


def send_record_batch(batch: pa.RecordBatch, recording: RecordingStream | None = None) -> None:
    """Coerce a single pyarrow `RecordBatch` to Rerun structure."""

    indexes = []
    data: defaultdict[str, list[Any]] = defaultdict(list)
    archetypes: defaultdict[str, set[Any]] = defaultdict(set)
    for col in batch.schema:
        metadata = col.metadata or {}
        if metadata.get(_RERUN_KIND) == _RERUN_KIND_CONTROL:
            continue
        if _SORBET_INDEX_NAME in metadata or metadata.get(_RERUN_KIND) == _RERUN_KIND_INDEX:
            if _SORBET_INDEX_NAME not in metadata:
                metadata[_SORBET_INDEX_NAME] = col.name
            indexes.append(_RawIndexColumn(metadata, batch.column(col.name)))
        else:
            entity_path = metadata.get(_SORBET_ENTITY_PATH, col.name.split(":")[0])
            if isinstance(entity_path, bytes):
                entity_path = entity_path.decode("utf-8")
            data[entity_path].append(_RawComponentBatchLike(metadata, batch.column(col.name)))
            if _SORBET_ARCHETYPE_NAME in metadata:
                archetypes[entity_path].add(metadata[_SORBET_ARCHETYPE_NAME].decode("utf-8"))

    for entity_path, columns in data.items():
        send_columns(
            entity_path,
            indexes,
            columns,
            # This is fine, send_columns will handle the conversion
            recording=recording,  # NOLINT
        )


# TODO(RR-3198): this should accept a `datafusion.DataFrame` as a soft dependency
def send_dataframe(df: pa.RecordBatchReader | pa.Table, recording: RecordingStream | None = None) -> None:
    """Coerce a pyarrow `RecordBatchReader` or `Table` to Rerun structure."""

    if isinstance(df, pa.Table):
        df = df.to_reader()

    for batch in df:
        send_record_batch(batch, recording)
