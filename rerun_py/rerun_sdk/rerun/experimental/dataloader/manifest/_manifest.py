"""Sampling-manifest schema conventions, writer, and reader."""

# A manifest freezes one epoch's blockwise sampling into a single manifest file.
# Each row is one sample, stored in fetch order and carrying its shard
# (`rank` / `worker`), its co-fetch / co-decode block (`fetch_group`), its position
# in the emission order (`emit_rank`), and what to read (`segment_id` / `anchor` /
# per-field `struct<lo, hi>`). Reading is pure replay: a shard iterates rows in
# fetch order (feeding the incremental decoder in-order) while emitting them in
# `emit_rank` order — no RNG, the order is data.

from __future__ import annotations

import dataclasses
import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from .._shuffle import BlockShuffle, ShuffleStrategy

if TYPE_CHECKING:
    import os

    from .._config import DataSource, Field
    from .._sample_index import FixedRateSampling

MANIFEST_FORMAT_VERSION = "1"

COL_RANK = "rank"
COL_WORKER = "worker"
COL_FETCH_GROUP = "fetch_group"
COL_EMIT_RANK = "emit_rank"
COL_SEGMENT_ID = "segment_id"
COL_ANCHOR = "anchor"

# Per-field columns are a `struct<lo, hi>`: the inclusive index range to read and decode for
# that field at each sample (the field's window, or a video field's `[keyframe, anchor]` GOP).
RANGE_LO = "lo"
RANGE_HI = "hi"

# Guards `Manifest.__init__` so a manifest can only be created through its factories.
_CONSTRUCTOR_KEY = object()


@dataclass(frozen=True)
class ManifestMeta:
    """Decoded manifest metadata header."""

    format_version: str
    dataset_name: str
    dataset_id: str
    index_name: str
    ns_per_sample: int | None
    ns_dtype: str | None
    recipe: dict[str, Any]
    required_fields: list[str]
    fetch_size: int
    buffer_size: int | None
    min_fill: int | None
    num_ranks: int
    num_workers_per_rank: int
    seed: int
    shuffle_strategy: str


def _metadata_from_schema(schema: pa.Schema) -> ManifestMeta:
    metadata_bytes = schema.metadata
    m = {k.decode(): v.decode() for k, v in (metadata_bytes or {}).items()}

    def _opt_int(key: str) -> int | None:
        v = m.get(key, "")
        return int(v) if v not in ("", "None") else None

    return ManifestMeta(
        format_version=m.get("manifest_format_version", ""),
        dataset_name=m.get("dataset_name", ""),
        dataset_id=m.get("dataset_id", ""),
        index_name=m.get("index_name", ""),
        ns_per_sample=_opt_int("ns_per_sample"),
        ns_dtype=(m.get("ns_dtype") if m.get("ns_dtype", "None") != "None" else None),
        recipe=json.loads(m.get("recipe", "{}")),
        required_fields=json.loads(m.get("required_fields", "[]")),
        fetch_size=int(m.get("fetch_size", "0")),
        buffer_size=_opt_int("buffer_size"),
        min_fill=_opt_int("min_fill"),
        num_ranks=int(m.get("num_ranks", "1")),
        num_workers_per_rank=int(m.get("num_workers_per_rank", "1")),
        seed=int(m.get("seed", "0")),
        shuffle_strategy=m.get("shuffle_strategy", ""),
    )


def _metadata_to_arrow(meta: ManifestMeta) -> dict[str, str]:
    """Serialize a metadata header to the schema-metadata string dict (inverse of `_metadata_from_schema`)."""
    raw: dict[str, Any] = {
        "manifest_format_version": meta.format_version,
        "dataset_name": meta.dataset_name,
        "dataset_id": meta.dataset_id,
        "index_name": meta.index_name,
        "ns_per_sample": meta.ns_per_sample,
        "ns_dtype": meta.ns_dtype,
        "recipe": json.dumps(meta.recipe),
        "required_fields": json.dumps(meta.required_fields),
        "fetch_size": meta.fetch_size,
        "buffer_size": meta.buffer_size,
        "min_fill": meta.min_fill,
        "num_ranks": meta.num_ranks,
        "num_workers_per_rank": meta.num_workers_per_rank,
        "seed": meta.seed,
        "shuffle_strategy": meta.shuffle_strategy,
    }
    return {k: str(v) for k, v in raw.items()}


class Manifest:
    """
    A description of the exact sampling order of data for one epoch of a training run. This serves the purpose of making training runs reproducible and resumable.

    A manifest is created by generating one from a source with [`Manifest.generate`][rerun.experimental.dataloader.Manifest.generate],
    or by loading an existing one with [`Manifest.from_parquet`][rerun.experimental.dataloader.Manifest.from_parquet].
    Persist it with [`Manifest.write_parquet`][rerun.experimental.dataloader.Manifest.write_parquet]; a manifest is always
    backed by a parquet file on the read path so a `DataLoader` worker never holds the whole table in RAM.

    !!! note
        This API is provisional and will be improved, expect the surface to change.
    """

    def __init__(
        self,
        table: pa.Table | None,
        metadata: ManifestMeta,
        *,
        path: str | os.PathLike[str] | None = None,
        _key: object = None,
    ) -> None:
        if _key is not _CONSTRUCTOR_KEY:
            raise TypeError("Create a Manifest via Manifest.generate() or Manifest.from_parquet().")
        # `_table` is the loaded manifest, or `None` when parquet-backed and not yet loaded —
        # the read path then loads only each worker's shard (see `worker_assignments`).
        self._table = table
        self._meta = metadata
        self._path = path

    @classmethod
    def _from_arrow(cls, table: pa.Table) -> Manifest:
        """Wrap an in-memory table built by `generate` / `shuffle`; there is no public in-memory constructor."""
        return cls(table, _metadata_from_schema(table.schema), _key=_CONSTRUCTOR_KEY)

    @classmethod
    def from_parquet(cls, path: str | os.PathLike[str]) -> Manifest:
        """
        Load a manifest from a parquet file.

        Rows are read lazily, per `(rank, worker)` shard, so a `DataLoader` worker
        never loads the whole manifest into RAM.
        """
        meta = _metadata_from_schema(pq.read_schema(path))
        return cls(None, meta, path=path, _key=_CONSTRUCTOR_KEY)

    def _ensure_table(self) -> pa.Table:
        """The whole manifest, read from the backing parquet file on first use."""
        if self._table is None:
            assert self._path is not None
            self._table = pq.read_table(self._path)
        return self._table

    def __getstate__(self) -> dict[str, Any]:
        """Ship only the path (not the loaded rows) to `DataLoader` workers when parquet-backed."""
        state = self.__dict__.copy()
        if self._path is not None:
            state["_table"] = None  # each worker re-reads its own shard; don't ship the whole table
        return state

    def write_parquet(self, path: str | os.PathLike[str], *, row_group_size: int = 1 << 16) -> None:
        """Write the manifest to a zstd-compressed parquet file; the header rides in the schema metadata."""
        pq.write_table(self._ensure_table(), path, compression="zstd", row_group_size=row_group_size)

    @property
    def metadata(self) -> ManifestMeta:
        """Decoded metadata header."""
        return self._meta

    @property
    def num_rows(self) -> int:
        """Total number of samples in the manifest."""
        if self._table is None:
            assert self._path is not None
            return int(pq.read_metadata(self._path).num_rows)  # from the footer, without loading rows
        return int(self._table.num_rows)

    @classmethod
    def generate(
        cls,
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: FixedRateSampling | None = None,
        fetch_size: int = 128,
        num_ranks: int = 1,
        num_workers_per_rank: int = 1,
        required_fields: list[str] | None = None,
        scan_max_workers: int | None = None,
    ) -> Manifest:
        """
        Generate an **unshuffled** manifest for one epoch by scanning the source.

        Scans the source, drops invalid samples, and unrolls one epoch in natural
        order (segment by segment, along the timeline) into `fetch_group` /
        `emit_rank` columns. To shuffle, call
        [`shuffle`][rerun.experimental.dataloader.Manifest.shuffle] on the result —
        it re-orders cheaply without re-scanning. Persist with
        [`write_parquet`][rerun.experimental.dataloader.Manifest.write_parquet].

        See [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset] for
        `source` / `index` / `fields` / `timeline_sampling`. `fetch_size` is the co-fetch /
        co-decode block size and `num_ranks` / `num_workers_per_rank` freeze the `(rank, worker)`
        assignment. `required_fields` are the field keys that must resolve to real data for a
        sample to be kept. `scan_max_workers` caps the concurrent scan queries against the
        server (defaults to `8`); raise it to speed up scanning a large dataset.
        """
        # Deferred import: `_manifest_build` imports this module's schema constants.
        from ._manifest_build import build_manifest_table

        table = build_manifest_table(
            source,
            index,
            fields,
            timeline_sampling=timeline_sampling,
            fetch_size=fetch_size,
            num_ranks=num_ranks,
            num_workers_per_rank=num_workers_per_rank,
            required_fields=required_fields,
            scan_max_workers=scan_max_workers,
        )
        return cls._from_arrow(table)

    def shuffle(
        self,
        strategy: ShuffleStrategy | None = None,
        *,
        seed: int = 0,
    ) -> Manifest:
        """
        Shuffle a scanned manifest into a new epoch order, without re-scanning the source.

        Keeps the manifest's validated sample set and per-field decode ranges (the
        expensive scan result) and only recomputes the `fetch_group` / `emit_rank`
        schedule under a new `strategy` and `seed`. `fetch_size` and the
        `(num_ranks, num_workers_per_rank)` topology are inherited from this manifest.
        Returns a new manifest; this one is unchanged.

        Scan once with [`generate`][rerun.experimental.dataloader.Manifest.generate],
        then call this per epoch (bumping `seed`) to get fresh orders for free.

        Parameters
        ----------
        strategy
            The [`ShuffleStrategy`][rerun.experimental.dataloader.ShuffleStrategy]
            to apply, e.g. [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] (the
            default), [`SampleShuffle`][rerun.experimental.dataloader.SampleShuffle], or
            [`NoShuffle`][rerun.experimental.dataloader.NoShuffle]. It fixes both the
            fetch order and the emission buffer, so passing the same strategy object
            here and to [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset]
            guarantees a replay matches the live run.
        seed
            Seed for the block shuffle and the emission buffer.

        """
        # Deferred import: `_manifest_build` imports this module's schema constants.
        from ._manifest_build import schedule_samples

        strategy = strategy if strategy is not None else BlockShuffle()
        meta = self._meta
        # Re-scheduling needs every sample, so this materializes a parquet-backed manifest.
        # `schedule_samples` drops the old schedule columns and regenerates them.
        table = schedule_samples(
            self._ensure_table(),
            strategy=strategy,
            fetch_size=meta.fetch_size,
            num_ranks=meta.num_ranks,
            num_workers_per_rank=meta.num_workers_per_rank,
            seed=seed,
        )
        buffer = strategy.emission_buffer()
        new_meta = dataclasses.replace(
            meta,
            shuffle_strategy=strategy.RECIPE_TAG,
            buffer_size=buffer.buffer_size if buffer is not None else None,
            min_fill=buffer.min_fill if buffer is not None else None,
            seed=seed,
        )
        return type(self)._from_arrow(table.replace_schema_metadata(_metadata_to_arrow(new_meta)))

    def to_arrow(self) -> pa.Table:
        """The manifest as an Arrow table (loading it from the backing parquet file if needed)."""
        return self._ensure_table()

    def validate_topology(self, num_ranks: int, num_workers_per_rank: int) -> None:
        """Raise if the run's topology differs from the one the manifest was frozen for."""
        meta = self._meta
        want = (meta.num_ranks, meta.num_workers_per_rank)
        if (num_ranks, num_workers_per_rank) != want:
            raise ValueError(
                f"Manifest was built for {want[0]}x{want[1]} (ranks x workers_per_rank), "
                f"but the run has {num_ranks}x{num_workers_per_rank}."
            )

    def worker_assignments(self, rank: int, worker: int) -> pa.Table:
        """
        Rows assigned to one `(rank, worker)`, in fetch order.

        For a parquet-backed manifest this reads only this shard from the file
        (predicate pushdown), not the whole manifest.
        """
        if self._table is None:
            assert self._path is not None
            return pq.read_table(self._path, filters=[(COL_RANK, "=", rank), (COL_WORKER, "=", worker)])
        mask = pc.and_(pc.equal(self._table[COL_RANK], rank), pc.equal(self._table[COL_WORKER], worker))
        return self._table.filter(mask)

    def worker_plan(self, rank: int, worker: int) -> tuple[list[pa.Table], np.ndarray]:
        """
        Return this `(rank, worker)`'s `fetch_group`s (co-fetch / co-decode units, in fetch order) and emission pull order.

        Reading a shard can hit disk, so the fetch groups and the emission order are
        both derived from a single read.
        """
        rows = self.worker_assignments(rank, worker)
        if rows.num_rows == 0:
            return [], np.empty(0, dtype=np.int64)

        # Split the shard into one table per `fetch_group` (a run of consecutive rows sharing an id).
        fetch_group = rows.column(COL_FETCH_GROUP).to_numpy()
        cuts = np.flatnonzero(np.diff(fetch_group)) + 1  # row indices where the id changes
        starts = [0, *cuts.tolist()]
        ends = [*cuts.tolist(), rows.num_rows]
        chunks = [rows.slice(start, end - start) for start, end in zip(starts, ends, strict=True)]

        emit_order = np.argsort(rows.column(COL_EMIT_RANK).to_numpy())
        return chunks, emit_order
