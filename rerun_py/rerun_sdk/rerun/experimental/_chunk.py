from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from .._send_dataframe import AUTO_INDEX, _AutoIndex

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator, Sequence
    from typing import TypeAlias

    import datafusion  # Soft dependency, ok for type checking

    from rerun import ComponentColumn
    from rerun._baseclasses import ComponentDescriptor
    from rerun._send_columns import TimeColumnLike
    from rerun_bindings import ChunkInternal

    from ._lens import Lens
    from ._selector import Selector

    # Single-schema dataframe sources accepted by `Chunk.from_dataframe`.
    # `datafusion` is an optional dependency.
    DataframeLike: TypeAlias = "pa.Table | pa.RecordBatch | pa.RecordBatchReader | datafusion.DataFrame"


def _resolve_index(index: str | list[str] | None | _AutoIndex) -> tuple[str, list[str]]:
    """Map the Python `index` argument to the binding's `(index_mode, index_columns)` arguments."""

    match index:
        case _AutoIndex():
            return "auto", []
        case None:
            return "static", []
        case str():
            return "columns", [index]
        case _:
            return "columns", list(index)


def _as_record_batch_reader(dataframe: DataframeLike) -> pa.RecordBatchReader:
    """Normalize a single-schema dataframe to a [`pa.RecordBatchReader`][pyarrow.RecordBatchReader]."""

    match dataframe:
        case pa.RecordBatchReader():
            return dataframe
        case pa.Table():
            return dataframe.to_reader()
        case pa.RecordBatch():
            return pa.RecordBatchReader.from_batches(dataframe.schema, [dataframe])
        # Anything implementing the Arrow C stream interface — e.g. a `datafusion.DataFrame` — can
        # be streamed lazily. The protocol guarantees a single schema for the whole stream, so we
        # don't need to import (or even name) datafusion here.
        case _ if hasattr(dataframe, "__arrow_c_stream__"):
            return pa.RecordBatchReader.from_stream(dataframe)
        case _:
            raise TypeError(
                "Expected a pyarrow Table, pyarrow RecordBatch, pyarrow RecordBatchReader, or an "
                f"Arrow-C-stream object (e.g. a datafusion DataFrame), got {type(dataframe).__name__}",
            )


class Chunk:
    """A single chunk of data from a recording."""

    _internal: ChunkInternal

    def __init__(self, internal: ChunkInternal) -> None:
        self._internal = internal

    @classmethod
    def from_record_batch(
        cls,
        record_batch: pa.RecordBatch,
        *,
        index: str | list[str] | None | _AutoIndex = AUTO_INDEX,
        entity_path: str | None = None,
    ) -> list[Chunk]:
        """
        Interpret an Arrow [`RecordBatch`][pyarrow.RecordBatch] as Rerun chunk data.

        Each column of the batch is classified as a row-id column, index (timeline) column,
        or a component column. Component columns are then grouped per entity path, and
        one chunk per entity path is emitted.

        The `rerun:*` arrow metadata, if it exists, drives the kind of each input column,
        as well as the entity/archetype/component type for component columns.

        If present, the row id column and chunk id metadata indicate that the batch represents
        a fully identified chunk, e.g. as produced by [`Chunk.to_record_batch`][rerun.experimental.Chunk.to_record_batch].
        Both the row ids and chunk id are preserved under the following conditions:
        - both are present in the input batch
        - `index` is omitted
        - `entity_path` is omitted

        If any of these conditions are not met, it means that either the batch is not fully
        identified, or that the chunk data is reinterpreted (e.g. entity path rewriting).
        In that case, fresh row ids and chunk id are generated and used instead of the input
        ones.

        Parameters
        ----------
        record_batch:
            The Arrow record batch to interpret.
            Component columns may be either lists (one component batch per row) or plain arrays
            (wrapped as single-element lists automatically).
        index:
            Determines which columns are index (timeline) columns. Each promoted column's
            time type is taken from its Arrow datatype: `int64` → sequence, `timestamp(ns)`
            → timestamp, `duration(ns)` → duration.

            - Omitted (the default): derive the index columns from the batch's Rerun metadata.
              The batch is treated as temporal if it carries index metadata. A batch with no index
              metadata is ambiguous and raises an error — unless it is an already-identified chunk
              (it carries a row-id column and a chunk id), which round-trips as-is and may therefore
              be static. Pass `index=None` to force a static interpretation.
            - A column name, or list of column names: treat exactly these columns as timelines.
              The remaining (non-row-id) columns become components.
            - `None`: produce static chunks (no timeline). Any index metadata or promoted index
              column is then a contradiction and is rejected.

            !!! note
                Static chunks with multiple rows are legitimate in some cases, but only the last
                row is visible from typical latest-at queries. An info-level message is emitted
                when this happens — except for an already-identified chunk that is preserved as-is
                (see above), which is passed through without this check.
        entity_path:
            Default entity path for component columns that do not otherwise specify one.
            Resolution order per component column is: its `rerun:entity_path` metadata, then the
            batch-level `rerun:entity_path` metadata, then the column-name convention (see
            *Notes*), then this argument, then the root entity (`/`).

        Returns
        -------
        One chunk per distinct entity path described by the batch, in first-seen column order.

        Raises
        ------
        ValueError
            In any of the following cases:

            - `index` was omitted and the batch carries no index metadata
              (an ambiguous raw batch). Pass `index=<column>` for temporal data or `index=None`
              for static data.
            - `index=None` was given but the batch also carries index metadata or names an
              index column (contradiction).
            - `index` names a column that is not present in the batch.
            - The batch contains no component columns (there is nothing to log).
            - A column promoted to an index contains null values. Time columns must be dense;
              static data is expressed with `index=None`, not with null times.
            - An index column has an Arrow datatype that is not a supported time type.
            - The batch is a fully-identified chunk (it carries both a row-id column and a chunk id)
              but resolves to more than one entity path. An identified chunk is preserved as a single
              chunk; drop the chunk-id metadata and/or the row-id column to reinterpret it into one
              chunk per entity (with freshly-minted ids).

        Notes
        -----
        **Column-name convention.** When a component column carries no `rerun:entity_path` /
        `rerun:component` metadata, its entity path is read from the column name: if the name starts
        with `/` and contains a `:`, the first part of the column name is interpreted as the entity
        path and the rest as the component identifier. Example: `/point:Points3D:positions` and
        `/metadata:foo`.

        Limitations/Future work
        -----------------------
        A batch that mixes static and temporal rows — aka where some index values are `null` — are
        rejected. Handling this case requires row-splitting and generating a mix of temporal and
        static chunks.

        Recording-property columns (named `property:…`, mapping to the `/__properties` entity) are
        not recognized by the column-name convention and are not mapped back to that entity.

        """
        # NOTE: Agent, keep this in sync with `Chunk.from_record_batch`.
        from rerun_bindings import ChunkInternal

        index_mode, index_columns = _resolve_index(index)
        return [
            cls(internal)
            for internal in ChunkInternal.from_record_batch(record_batch, index_mode, index_columns, entity_path)
        ]

    @classmethod
    def from_dataframe(
        cls,
        dataframe: DataframeLike,
        *,
        index: str | list[str] | None | _AutoIndex = AUTO_INDEX,
        entity_path: str | None = None,
    ) -> Iterator[Chunk]:
        """
        Lazily turn an Arrow-backed dataframe into chunks.

        Accepts a [`Table`][pyarrow.Table], a [`RecordBatch`][pyarrow.RecordBatch], a
        [`RecordBatchReader`][pyarrow.RecordBatchReader], or any object implementing the Arrow C
        stream interface (`__arrow_c_stream__`) — most notably a `datafusion.DataFrame` (an optional
        dependency).

        Yields each chunk of
        [`Chunk.from_record_batch`][rerun.experimental.Chunk.from_record_batch] applied to every
        record batch in turn. See that method for the `index` and `entity_path` semantics.


        Raises
        ------
        TypeError
            If `dataframe` is not a pyarrow `Table`, a pyarrow `RecordBatch`, a pyarrow
            `RecordBatchReader`, or an Arrow-C-stream object (such as a `datafusion.DataFrame`).
        ValueError
            See [`Chunk.from_record_batch`][rerun.experimental.Chunk.from_record_batch].

        """

        # Note: by returning a generator instead of _being_ a generator, we ensure that this line is executed at call
        # time and not deferred to the first `next()`
        reader = _as_record_batch_reader(dataframe)

        def chunks() -> Iterator[Chunk]:
            for batch in reader:
                yield from cls.from_record_batch(batch, index=index, entity_path=entity_path)

        return chunks()

    @classmethod
    def from_columns(
        cls,
        entity_path: str,
        indexes: Iterable[TimeColumnLike],
        columns: Iterable[ComponentColumn],
    ) -> Chunk:
        """
        Create a Chunk from columns, mirroring the [`rerun.send_columns`][] API.

        A fresh chunk ID and sequential row IDs are auto-generated.

        Parameters
        ----------
        entity_path:
            The entity path for this chunk (e.g., "/camera/image").
        indexes:
            The time columns for this chunk. Each `TimeColumnLike`
            provides a timeline name and a PyArrow array of timestamps.
            You typically use `TimeColumn` here.
            Pass an empty iterable for static data.
        columns:
            The component columns for this chunk. Each
            `ComponentColumn` provides a component descriptor
            and a PyArrow array of component data.

        Raises
        ------
        ValueError
            If timeline and component column lengths don't match.

        Example
        -------
        ```python
        chunk = Chunk.from_columns(
            "/robots/arm",
            indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],
            columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
        )
        ```

        """
        from rerun._send_columns import build_column_args
        from rerun_bindings import ChunkInternal

        timelines_args, columns_args = build_column_args(indexes, columns)

        return cls(ChunkInternal.from_columns(entity_path, timelines_args, columns_args))

    @property
    def id(self) -> str:
        """The unique ID of this chunk."""
        return self._internal.id

    @property
    def entity_path(self) -> str:
        """The entity path this chunk belongs to."""
        return self._internal.entity_path

    @property
    def num_rows(self) -> int:
        """The number of rows in this chunk."""
        return self._internal.num_rows

    @property
    def num_columns(self) -> int:
        """The number of columns in this chunk."""
        return self._internal.num_columns

    @property
    def is_static(self) -> bool:
        """Whether the chunk contains only static data (no timelines)."""
        return self._internal.is_static

    @property
    def is_empty(self) -> bool:
        """Whether the chunk has zero rows."""
        return self._internal.is_empty

    @property
    def timeline_names(self) -> list[str]:
        """The names of all timelines in this chunk."""
        return self._internal.timeline_names

    def to_record_batch(self) -> pa.RecordBatch:
        """Convert this chunk to an Arrow RecordBatch."""
        return self._internal.to_record_batch()

    def with_entity_path(self, entity_path: str) -> Chunk:
        """
        Return a copy of this chunk with a new entity path.

        A fresh chunk ID is generated to avoid aliasing the original chunk in downstream
        caches and indices. Row IDs, timelines, and components are preserved as-is.

        Parameters
        ----------
        entity_path:
            The new entity path for the returned chunk (e.g. `"/left/camera/image"`).

        """
        return Chunk(self._internal.with_entity_path(entity_path))

    def apply_selector(
        self,
        source: ComponentDescriptor | str,
        selector: Selector | str,
    ) -> Chunk:
        """
        Apply a selector to a single component, returning a new chunk with the component transformed.

        All other columns (timelines, other components) are preserved unchanged.
        The source component's existing descriptor is preserved.

        For better performance, prefer [`MutateLens`][rerun.experimental.MutateLens]
        with [`apply_lenses`][rerun.experimental.Chunk.apply_lenses]
        which processes multiple transformations in a single pass.

        Parameters
        ----------
        source:
            A `ComponentDescriptor` or component identifier string for the
            input column to transform.
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to apply to the component.

        Returns
        -------
        A new [`Chunk`][rerun.experimental.Chunk] with the component transformed.

        Raises
        ------
        ValueError
            If the source component is not found in the chunk or the selector
            fails to evaluate.

        """
        from rerun._baseclasses import ComponentDescriptor as CD

        from ._selector import Selector as SelectorType

        source_str = source.component if isinstance(source, CD) else source

        if isinstance(selector, str):
            selector = SelectorType(selector)

        return Chunk(self._internal.apply_selector(source_str, selector._internal))

    def apply_lenses(
        self,
        lenses: Sequence[Lens] | Lens,
    ) -> list[Chunk]:
        """
        Apply one or more lenses to this chunk, returning transformed chunks.

        Each lens matches by input component. Columns not consumed by
        any matching lens are forwarded unchanged as a separate chunk.

        If no lens matches the chunk (including when an empty list of
        lenses is passed), the original chunk is returned unchanged.

        Parameters
        ----------
        lenses:
            One or more [`Lens`][rerun.experimental.Lens] objects.

        Returns
        -------
        A list of [`Chunk`][] objects.

        """
        from ._lens import Lens

        if isinstance(lenses, Lens):
            lenses = [lenses]
        return [Chunk(internal) for internal in self._internal.apply_lenses([lens._internal for lens in lenses])]

    def format(self, *, width: int = 240, redact: bool = False, trim_metadata_keys: bool = True) -> str:
        """
        Format this chunk as a human-readable table string.

        Parameters
        ----------
        width:
            Fixed width for the table. Default: 240.
        redact:
            If True, redact non-deterministic values (RowIds, ChunkIds, etc.)
            for stable snapshot testing. Default: False.
        trim_metadata_keys:
            If True, trim the `rerun:` / `sorbet:` prefix from metadata keys.
            Default: True.

        """
        return self._internal.format(width=width, redact=redact, trim_metadata_keys=trim_metadata_keys)

    def __repr__(self) -> str:
        return repr(self._internal)

    def __str__(self) -> str:
        return self.format()

    def __len__(self) -> int:
        return len(self._internal)
