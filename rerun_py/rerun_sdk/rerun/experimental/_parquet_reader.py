"""Experimental parquet reader with configurable column grouping."""

from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import ParquetReaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from pathlib import Path


class ParquetReader:
    """
    Read chunks from a Parquet file.

    The reader turns raw parquet columns into grouped, time-indexed
    [`Chunk`][rerun.experimental.Chunk]s of struct/scalar components. To map those
    struct fields into Rerun archetypes (translation, rotation, scalars, …), apply
    lenses to the resulting `.stream()` — see
    [`DeriveLens`][rerun.experimental.DeriveLens]:

    Example
    -------
    ```python
    from rerun.experimental import ParquetReader, DeriveLens

    store = (
        ParquetReader(path, index_columns=[("frame_index", "sequence")])
        .stream()
        .lenses(
            [
                DeriveLens("data", output_entity="/pose")
                .to_translation("pos_x", "pos_y", "pos_z")
                .to_quaternion("quat_x", "quat_y", "quat_z", "quat_w")
            ],
            content="/transform",
        )
        .collect()
    )
    ```

    """

    _internal: ParquetReaderInternal

    def __init__(
        self,
        path: str | Path,
        *,
        entity_path_prefix: str | None = None,
        column_grouping: str = "prefix",
        delimiter: str = "_",
        prefixes: list[str] | None = None,
        use_structs: bool = True,
        static_columns: list[str] | None = None,
        index_columns: list[tuple[str, str] | tuple[str, str, str]] | None = None,
    ) -> None:
        """
        Load a parquet file with configurable column grouping.

        Parameters
        ----------
        path:
            Path to the `.parquet` file.
        entity_path_prefix:
            Optional prefix for all entity paths (e.g. `"/world"`).
        column_grouping:
            How to group columns into chunks. `"prefix"` splits column names
            on `delimiter` and groups by the first segment. `"individual"`
            gives each column its own chunk. `"explicit_prefixes"` groups
            columns by the explicit prefix strings in `prefixes`.
        delimiter:
            Character used to split column names when `column_grouping="prefix"`.
        prefixes:
            Explicit prefix strings for grouping columns. Required when
            `column_grouping="explicit_prefixes"`. Columns starting with a
            prefix are grouped together; the prefix is stripped from the
            component name. Prefixes are tried longest-first to avoid
            ambiguity.
        use_structs:
            When `True` (default) and `column_grouping="prefix"` or
            `"explicit_prefixes"`, columns sharing a prefix are packed into
            a single Arrow `Struct` component. When `False`, each column
            becomes a separate component (the pre-struct layout). Ignored
            when `column_grouping="individual"`.
        static_columns:
            Column names whose values are constant across all rows. These are
            emitted once as timeless/static data. An error is raised if a
            listed column contains varying values.
        index_columns:
            List of columns to use as timeline indices. Each entry is a tuple:
            `(name, type)` or `(name, type, unit)`.

            The `type` specifies the timeline kind:

            - `"timestamp"`: time since epoch
            - `"duration"`: elapsed time
            - `"sequence"`: ordinal integer index

            The `unit` describes what the raw integer values in the column
            represent (not a desired output unit). Rerun stores all timestamps
            in nanoseconds internally, so values are scaled accordingly.
            Supported: `"ns"` (default), `"us"`, `"ms"`, `"s"`.
            Ignored for `"sequence"` type.

            When omitted, a synthetic `row_index` sequence timeline is
            generated automatically (one entry per row).

        """
        # Normalize index_columns: pad 2-tuples to 3-tuples with None for the unit
        normalized_index = (
            [(t[0], t[1], t[2] if len(t) > 2 else None) for t in index_columns] if index_columns is not None else None
        )

        self._internal = ParquetReaderInternal(
            str(path),
            entity_path_prefix=entity_path_prefix,
            column_grouping=column_grouping,
            delimiter=delimiter,
            prefixes=prefixes,
            use_structs=use_structs,
            static_columns=static_columns,
            index_columns=normalized_index,
        )

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the Parquet file."""
        return LazyChunkStream(self._internal.stream())

    @property
    def path(self) -> Path:
        """The file path of the Parquet file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"ParquetReader({self._internal.path})"
