"""Experimental parquet loader with configurable column grouping and archetype mapping."""

from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import ParquetLoaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path


class ParquetLoader:
    """Load chunks from a Parquet file."""

    _internal: ParquetLoaderInternal

    def __init__(
        self,
        path: str | Path,
        *,
        entity_path_prefix: str | None = None,
        column_grouping: str = "prefix",
        delimiter: str = "_",
        static_columns: list[str] | None = None,
        index_columns: list[tuple[str, str] | tuple[str, str, str]] | None = None,
        pos_suffixes: Sequence[Sequence[str]] | None = None,
        quat_suffixes: Sequence[Sequence[str]] | None = None,
        scalar_suffixes: Sequence[tuple[Sequence[str], Sequence[str]]] | None = None,
    ) -> None:
        """
        Load a parquet file with configurable column grouping, archetype mapping, and static columns.

        Parameters
        ----------
        path:
            Path to the ``.parquet`` file.
        entity_path_prefix:
            Optional prefix for all entity paths (e.g. ``"/world"``).
        column_grouping:
            How to group columns into chunks. ``"prefix"`` splits column names
            on `delimiter` and groups by the first segment. ``"individual"``
            gives each column its own chunk.
        delimiter:
            Character used to split column names when ``column_grouping="prefix"``.
        static_columns:
            Column names whose values are constant across all rows. These are
            emitted once as timeless/static data. An error is raised if a
            listed column contains varying values.
        index_columns:
            List of columns to use as timeline indices. Each entry is a tuple:
            ``(name, type)`` or ``(name, type, unit)``.

            The ``type`` specifies the timeline kind:

            - ``"timestamp"``: time since epoch
            - ``"duration"``: elapsed time
            - ``"sequence"``: ordinal integer index

            The ``unit`` describes what the raw integer values in the column
            represent (not a desired output unit). Rerun stores all timestamps
            in nanoseconds internally, so values are scaled accordingly.
            Supported: ``"ns"`` (default), ``"us"``, ``"ms"``, ``"s"``.
            Ignored for ``"sequence"`` type.

            Example::

                index_columns=[
                    ("timestamp_ms", "timestamp", "ms"),  # column values are milliseconds
                    ("frame_id", "sequence"),
                ]

            When omitted, a synthetic ``row_index`` sequence timeline is
            generated automatically (one entry per row).
        pos_suffixes:
            One or more suffix groups that identify translation columns.
            Each group is an ordered list of suffixes
            (default: ``[["_pos_x", "_pos_y", "_pos_z"]]``).
            Columns matching a group's suffixes with a common prefix are
            combined into a ``Translation3D`` component.
        quat_suffixes:
            One or more suffix groups that identify quaternion columns.
            Each group is an ordered list of suffixes
            (default: ``[["_quat_x", "_quat_y", "_quat_z", "_quat_w"]]``).
            Columns matching a group's suffixes with a common prefix are
            combined into a ``RotationQuat`` component.
        scalar_suffixes:
            One or more ``(suffixes, names)`` pairs. Each pair groups columns
            whose names end with the given suffixes into a multi-instance
            ``Scalars`` component with a static ``Name`` component for the
            series labels.

            Example -- to plot ``*_x``, ``*_y``, ``*_z`` columns as named
            scalar series::

                scalar_suffixes=[
                    (["_x", "_y", "_z"], ["x", "y", "z"]),
                ]

            This converts ``prefix_foo_x``, ``prefix_foo_y``, ``prefix_foo_z``
            into entity ``/prefix/foo`` with three named scalar series.

        """
        # Normalize index_columns: pad 2-tuples to 3-tuples with None for the unit
        normalized_index = (
            [(t[0], t[1], t[2] if len(t) > 2 else None) for t in index_columns] if index_columns is not None else None
        )
        self._internal = ParquetLoaderInternal(
            str(path),
            entity_path_prefix=entity_path_prefix,
            column_grouping=column_grouping,
            delimiter=delimiter,
            static_columns=static_columns,
            index_columns=normalized_index,
            pos_suffixes=[list(g) for g in pos_suffixes] if pos_suffixes is not None else None,
            quat_suffixes=[list(g) for g in quat_suffixes] if quat_suffixes is not None else None,
            scalar_suffixes=([(list(s), list(n)) for s, n in scalar_suffixes] if scalar_suffixes is not None else None),
        )

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the Parquet file."""
        return LazyChunkStream(self._internal.stream())

    @property
    def path(self) -> Path:
        """The file path of the Parquet file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"ParquetLoader({self._internal.path})"
