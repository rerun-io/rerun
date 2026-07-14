from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path

    import datafusion

    from rerun.catalog import ContentFilter, IndexValuesLike, Schema
    from rerun_bindings import ChunkStoreInternal

    from ._chunk import Chunk
    from ._lazy_chunk_stream import LazyChunkStream


class ChunkStore:
    """
    A fully-materialized, in-memory chunk store.

    Build one from chunks via
    [`ChunkStore.from_chunks`][rerun.experimental.ChunkStore.from_chunks], or
    fully materialize an [`IndexedReader`][rerun.experimental.IndexedReader]
    via `reader.stream().collect()`.
    For lazy, on-demand chunk loading, see [`LazyStore`][rerun.experimental.LazyStore].

    Use `stream()` to process chunks through the lazy pipeline, or
    `write_rrd()` to persist to disk.
    """

    _internal: ChunkStoreInternal

    def __init__(self, internal: ChunkStoreInternal) -> None:
        self._internal = internal

    @staticmethod
    def from_chunks(chunks: Sequence[Chunk]) -> ChunkStore:
        """Build a ChunkStore from a sequence of chunks."""
        from rerun_bindings import ChunkStoreInternal

        internals = [c._internal for c in chunks]
        return ChunkStore(ChunkStoreInternal.from_chunks(internals))

    def schema(self) -> Schema:
        """The schema describing all columns in this store."""
        from rerun.catalog import Schema

        return Schema(self._internal.schema())

    def summary(self) -> str:
        """
        Compact, deterministic summary of every chunk in the store.

        Each line describes one chunk:

            {entity_path}  rows={n}  static={True|False}  timelines=[…]  cols=[…]

        Useful for snapshot testing.
        """
        return self._internal.summary()

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in this store."""
        from ._lazy_chunk_stream import LazyChunkStream

        return LazyChunkStream(self._internal.stream())

    def reader(
        self,
        index: str | None,
        *,
        contents: ContentFilter | str | list[str] | None = None,
        include_semantically_empty_columns: bool = False,
        include_tombstone_columns: bool = False,
        fill_latest_at: bool = False,
        using_index_values: IndexValuesLike | None = None,
        ctx: datafusion.SessionContext | None = None,
    ) -> datafusion.DataFrame:
        """
        Build a DataFusion DataFrame over this store.

        The returned DataFrame is data-equivalent to the result of round-tripping
        the same chunks through `write_rrd → rr.server.Server → dataset.reader()`,
        modulo the `rerun_segment_id` column (absent here because a single
        `ChunkStore` has no segment concept).

        Parameters
        ----------
        index
            The index (timeline) column to use, or `None` for the static-only view.
        contents
            Entity-path filter. A `ContentFilter` built with the fluent API, a single
            entity-path expression, a list of expressions, or `None` for everything.
            An empty list returns no rows.
        include_semantically_empty_columns
            Whether to include columns that are semantically empty.
        include_tombstone_columns
            Whether to include tombstone columns.
        fill_latest_at
            Whether to fill null values with the latest valid data.
        using_index_values
            Index values at which to **resample** data.

            When specified, this argument changes the way rows are returned. Instead
            of returning the rows that exist in the data, one row is returned per
            `index_value` you provide. If the segment has no row at that index value,
            nulls are returned — or the latest prior value if fill_latest_at=True`
            (which is typically what you want for resampling).

            Don't use this argument for plain index slicing — use a DataFusion filter
            on the index column instead. For example:

            ```python
            from datafusion import col, lit

            # All rows in a time window.
            store.reader(index="real_time").filter(
                (col("real_time") >= lit(t0)) & (col("real_time") <= lit(t1))
            )
            ```
        ctx
            DataFusion `SessionContext` to register the table into. When `None`,
            uses `datafusion.SessionContext.global_ctx()` — the process-wide
            default.
            Pass an explicit `ctx` for isolation or a custom `SessionConfig`.

        """
        import datafusion

        from rerun.catalog._content_filter import ContentFilter

        contents_list: list[str] | None
        match contents:
            case ContentFilter():
                contents_list = contents.to_exprs()
            case str():
                contents_list = [contents]
            case None:
                contents_list = None
            case _:
                contents_list = list(contents)

        table = self._internal.reader(
            index=index,
            contents=contents_list,
            include_semantically_empty_columns=include_semantically_empty_columns,
            include_tombstone_columns=include_tombstone_columns,
            fill_latest_at=fill_latest_at,
            using_index_values=using_index_values,
        )
        if ctx is None:
            # TODO(RR-4795): we should use a SDK-provided context (with pre-populated UDF, etc.) instead of global_ctx
            ctx = datafusion.SessionContext.global_ctx()
        return ctx.read_table(table)

    def write_rrd(
        self,
        path: str | Path,
        *,
        application_id: str,
        recording_id: str,
    ) -> None:
        """
        Write all chunks to an RRD file.

        The caller must provide application_id and recording_id explicitly.
        """
        self.stream().write_rrd(
            path,
            application_id=application_id,
            recording_id=recording_id,
        )

    def __len__(self) -> int:
        """Return the number of chunks in this store."""
        return self._internal.num_chunks()

    def __repr__(self) -> str:
        return f"ChunkStore({len(self)} chunks)"
