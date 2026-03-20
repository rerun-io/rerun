from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pyarrow as pa

    from rerun_bindings import ChunkInternal


class Chunk:
    """A single chunk of data from a recording."""

    _internal: ChunkInternal

    def __init__(self, internal: ChunkInternal) -> None:
        self._internal = internal

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
