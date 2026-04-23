from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence

    import pyarrow as pa

    from rerun import ComponentColumn
    from rerun._baseclasses import ComponentDescriptor
    from rerun._send_columns import TimeColumnLike
    from rerun_bindings import ChunkInternal

    from ._lens import Lens
    from ._selector import Selector


class Chunk:
    """A single chunk of data from a recording."""

    _internal: ChunkInternal

    def __init__(self, internal: ChunkInternal) -> None:
        self._internal = internal

    @classmethod
    def from_record_batch(cls, record_batch: pa.RecordBatch) -> Chunk:
        """
        Create a Chunk from a PyArrow RecordBatch with Rerun schema metadata.

        The RecordBatch must have Rerun metadata in its schema, as produced by
        `to_record_batch`. This enables round-tripping through PyArrow
        transforms. The original chunk ID and row IDs are preserved.

        Parameters
        ----------
        record_batch:
            A PyArrow RecordBatch with Rerun schema metadata.

        Raises
        ------
        ValueError
            If the RecordBatch lacks required Rerun schema metadata.

        """
        from rerun_bindings import ChunkInternal

        return cls(ChunkInternal.from_record_batch(record_batch))

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

    def apply_selector(
        self,
        source: ComponentDescriptor | str,
        selector: Selector | str,
    ) -> Chunk:
        """
        Apply a selector to a single component, returning a new chunk with the component transformed.

        All other columns (timelines, other components) are preserved unchanged.
        The source component's existing descriptor is preserved.

        Parameters
        ----------
        source:
            A [`ComponentDescriptor`][] or component identifier string for the
            input column to transform.
        selector:
            A [`Selector`][] or selector query string to apply to the component.

        Returns
        -------
        A new [`Chunk`][] with the component transformed.

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

    def apply_lenses(self, lenses: Sequence[Lens] | Lens) -> list[Chunk]:
        """
        Apply one or more lenses to this chunk, returning transformed chunks.

        Each lens matches by input component. Columns not consumed by
        any matching lens are forwarded unchanged as a separate chunk.
        A single lens with multiple [`LensOutput`][] groups may produce
        multiple output chunks (e.g., with different target entities).

        If no lens matches the chunk (including when an empty list of
        lenses is passed), the original chunk is returned unchanged.

        Parameters
        ----------
        lenses:
            Zero or more [`Lens`][] objects to apply.

        Returns
        -------
        A list of [`Chunk`][] objects. Contains the original chunk if no
        lens matched, or one or more transformed chunks (optionally
        preceded by a chunk with the untouched forwarded columns)
        otherwise.

        Raises
        ------
        ValueError
            If a lens produces a partial result (e.g., a selector fails
            to evaluate on the input data, or a lens produces no output
            columns).

        """
        from ._lens import Lens as LensType

        if isinstance(lenses, LensType):
            lenses = [lenses]
        return [Chunk(internal) for internal in self._internal.apply_lenses([lens._internal for lens in lenses])]

    def format(self, *, width: int = 240, redact: bool = False) -> str:
        """
        Format this chunk as a human-readable table string.

        Parameters
        ----------
        width:
            Fixed width for the table. Default: 240.
        redact:
            If True, redact non-deterministic values (RowIds, ChunkIds, etc.)
            for stable snapshot testing. Default: False.

        """
        return self._internal.format(width=width, redact=redact)

    def __repr__(self) -> str:
        return repr(self._internal)

    def __str__(self) -> str:
        return self._internal.format()

    def __len__(self) -> int:
        return len(self._internal)
