from __future__ import annotations

from collections import defaultdict
from typing import TYPE_CHECKING, Any

import pyarrow as pa

from ._baseclasses import ComponentColumn, ComponentDescriptor
from ._send_columns import TimeColumnLike, send_columns

if TYPE_CHECKING:
    from .recording_stream import RecordingStream

SORBET_INDEX_NAME = b"rerun:index_name"
SORBET_ENTITY_PATH = b"rerun:entity_path"
SORBET_ARCHETYPE_NAME = b"rerun:archetype"
SORBET_COMPONENT = b"rerun:component"
SORBET_COMPONENT_TYPE = b"rerun:component_type"
SORBET_IS_TABLE_INDEX = b"rerun:is_table_index"
RERUN_KIND = b"rerun:kind"
RERUN_KIND_CONTROL = b"control"
RERUN_KIND_INDEX = b"index"


class _RawIndexColumn(TimeColumnLike):
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


class _RawComponentBatchLike(ComponentColumn):
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


def send_record_batch(batch: pa.RecordBatch, recording: RecordingStream | None = None) -> None:
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
            indexes.append(_RawIndexColumn(metadata, batch.column(col.name)))
        else:
            entity_path = metadata.get(SORBET_ENTITY_PATH, col.name.split(":")[0])
            if isinstance(entity_path, bytes):
                entity_path = entity_path.decode("utf-8")
            data[entity_path].append(_RawComponentBatchLike(metadata, batch.column(col.name)))
            if SORBET_ARCHETYPE_NAME in metadata:
                archetypes[entity_path].add(metadata[SORBET_ARCHETYPE_NAME].decode("utf-8"))

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


def view_contents_for_archetypes(
    schema: Schema,
    archetypes: str | list[str],
    *,
    entity_path: str | None = None,
) -> ViewContentsLike:
    """
    Create a ViewContentsLike filter for specific archetypes.

    This helper function makes it easy to filter views to specific archetypes
    using the schema exploration methods.

    Parameters
    ----------
    schema : Schema
        The schema to query for columns.
    archetypes : str | list[str]
        Archetype name or list of archetype names (fully-qualified,
        e.g., "rerun.archetypes.Points3D").
    entity_path : str | None
        Optional entity path filter. If provided, only columns at this
        entity path will be included.

    Returns
    -------
    ViewContentsLike
        A contents specification that can be passed to `Recording.view()`
        or `DatasetEntry.dataframe_query_view()`.

    Examples
    --------
    Filter a recording to only Points3D data:

    ```python
    import rerun as rr

    recording = rr.dataframe.load_recording("recording.rrd")
    schema = recording.schema()

    # Create view with only Points3D archetype
    contents = rr.dataframe.view_contents_for_archetypes(
        schema,
        "rerun.archetypes.Points3D"
    )
    view = recording.view(index="frame", contents=contents)
    ```

    Filter to multiple archetypes:

    ```python
    contents = rr.dataframe.view_contents_for_archetypes(
        schema,
        ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
    )
    view = recording.view(index="frame", contents=contents)
    ```

    Filter to a specific archetype at a specific entity:

    ```python
    contents = rr.dataframe.view_contents_for_archetypes(
        schema,
        "rerun.archetypes.Points3D",
        entity_path="/world/points"
    )
    view = recording.view(index="frame", contents=contents)
    ```

    """
    if isinstance(archetypes, str):
        archetypes = [archetypes]

    all_components = []
    for archetype in archetypes:
        columns = schema.columns_for(
            archetype=archetype,
            entity_path=entity_path,
        )
        # Extract just the component part (e.g., "Points3D:positions")
        # not the full column name (e.g., "/world/points:Points3D:positions")
        all_components.extend([col.component for col in columns])

    # Return as dict with wildcard path and specific component names
    # This tells the view to include these components from any matching entity
    return {"/**": all_components}


def view_contents_for_component_types(
    schema: Schema,
    component_types: str | list[str],
    *,
    entity_path: str | None = None,
) -> ViewContentsLike:
    """
    Create a ViewContentsLike filter for specific component types.

    This helper function makes it easy to filter views to specific component types
    using the schema exploration methods.

    Parameters
    ----------
    schema : Schema
        The schema to query for columns.
    component_types : str | list[str]
        Component type name or list of component type names (fully-qualified,
        e.g., "rerun.components.Position3D").
    entity_path : str | None
        Optional entity path filter. If provided, only columns at this
        entity path will be included.

    Returns
    -------
    ViewContentsLike
        A contents specification that can be passed to `Recording.view()`
        or `DatasetEntry.dataframe_query_view()`.

    Examples
    --------
    Filter a recording to only Position3D components:

    ```python
    import rerun as rr

    recording = rr.dataframe.load_recording("recording.rrd")
    schema = recording.schema()

    # Create view with only Position3D components
    contents = rr.dataframe.view_contents_for_component_types(
        schema,
        "rerun.components.Position3D"
    )
    view = recording.view(index="frame", contents=contents)
    ```

    Filter to multiple component types:

    ```python
    contents = rr.dataframe.view_contents_for_component_types(
        schema,
        ["rerun.components.Position3D", "rerun.components.Color"]
    )
    view = recording.view(index="frame", contents=contents)
    ```

    Filter to a specific component type at a specific entity:

    ```python
    contents = rr.dataframe.view_contents_for_component_types(
        schema,
        "rerun.components.Position3D",
        entity_path="/world/points"
    )
    view = recording.view(index="frame", contents=contents)
    ```

    """
    if isinstance(component_types, str):
        component_types = [component_types]

    all_components = []
    for component_type in component_types:
        columns = schema.columns_for(
            component_type=component_type,
            entity_path=entity_path,
        )
        # Extract just the component part (e.g., "Points3D:positions")
        # not the full column name (e.g., "/world/points:Points3D:positions")
        all_components.extend([col.component for col in columns])

    # Return as dict with wildcard path and specific component names
    # This tells the view to include these components from any matching entity
    return {"/**": all_components}
>>>>>>> 8c388a3028f (Follow-up 2: Add view contents helper functions for filtering):rerun_py/rerun_sdk/rerun/dataframe.py
