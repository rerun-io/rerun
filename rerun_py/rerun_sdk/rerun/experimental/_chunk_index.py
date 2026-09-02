from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import datafusion
    import pyarrow as pa


# TODO(RR-5531): this class is currently private (not re-exported from `rerun.experimental`. To be cleaned up and made public
# when it stabilizes. Also, it should be implemented per the Python-wrapper/rust-internal pattern.
class ChunkIndex:
    """A store's chunk index (its raw RRD manifest), one row per chunk."""

    def __init__(self, store_id: str, batch: pa.RecordBatch) -> None:
        self._store_id = store_id
        self._batch = batch

    def df(self, ctx: datafusion.SessionContext | None = None) -> datafusion.DataFrame:
        """
        The chunk index as a DataFusion DataFrame, verbatim.

        Parameters
        ----------
        ctx
            DataFusion `SessionContext` to register the data into. When `None`, uses
            `datafusion.SessionContext.global_ctx()` — the process-wide default.

        """
        import datafusion
        import pyarrow as pa

        if ctx is None:
            ctx = datafusion.SessionContext.global_ctx()
        return ctx.from_arrow(pa.Table.from_batches([self._batch]))

    def to_arrow(self) -> pa.RecordBatch:
        """The raw chunk index, verbatim, for pandas/polars users."""
        return self._batch

    @property
    def store_id(self) -> str:
        """The id of the store this index describes."""
        return self._store_id

    @property
    def num_columns(self) -> int:
        """
        The number of columns of the chunk index itself.

        Recordings whose chunk index exceeds the catalog server's column limit fail registration.
        """
        return int(self._batch.num_columns)

    def __len__(self) -> int:
        """The number of chunks."""
        return int(self._batch.num_rows)

    def __repr__(self) -> str:
        return f"ChunkIndex({len(self)} chunks, {self.num_columns} columns, store_id={self._store_id!r})"
