"""
Tests that a `RerunIterableDataset` built from a `Manifest` replays a live (manifest-less) run exactly.

Both paths are driven from one small in-memory dataset with the catalog I/O stubbed out, so the
only thing under test is that the manifest's frozen order and decode reproduce what the live path
does for the same strategy, seed, and topology.
"""

from __future__ import annotations

import pickle
from types import SimpleNamespace
from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa
import pytest
import torch
from rerun.experimental.dataloader import (
    BlockShuffle,
    Field,
    Manifest,
    NoShuffle,
    NumericDecoder,
    RerunIterableDataset,
    SampleShuffle,
    _iterable_dataset as iterable_dataset,
)
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import Target
from rerun.experimental.dataloader.manifest._manifest import (
    MANIFEST_FORMAT_VERSION,
    ManifestMeta,
    _metadata_to_arrow,
)
from rerun.experimental.dataloader.manifest._manifest_build import (
    _compact_index,
    _resolve_rows,
    _ResolvedRows,
    _sample_table,
    _ScanResult,
    schedule_samples,
)

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.experimental.dataloader._config import DataSource
    from rerun.experimental.dataloader._shuffle import ShuffleStrategy

# A tiny fake dataset in canonical (segment ascending, anchor ascending) order: 12 samples
# across two segments, with globally unique anchors so a sample is identified by its anchor.
SEGMENT_IDS = ["a"] * 8 + ["b"] * 4
ANCHORS = [*range(8), *range(100, 104)]
SEED = 3
FETCH_SIZE = 4

_SOURCE = cast(
    "DataSource",
    SimpleNamespace(
        dataset=SimpleNamespace(catalog=SimpleNamespace(url="rerun+http://fake"), name="ds", id="id"),
    ),
)
_FIELDS = {"x": Field("/e:Scalars:scalars", decode=NumericDecoder())}


def _buffer_size(strategy: ShuffleStrategy) -> int | None:
    buffer = strategy.emission_buffer()
    return buffer.buffer_size if buffer is not None else None


def _min_fill(strategy: ShuffleStrategy) -> int | None:
    buffer = strategy.emission_buffer()
    return buffer.min_fill if buffer is not None else None


def _build_manifest(strategy: ShuffleStrategy, *, num_ranks: int = 1) -> Manifest:
    """Build a `num_ranks` / single-worker manifest for the fake dataset via the real scheduler."""
    anchors = np.array(ANCHORS, dtype=np.int64)
    rows = _ResolvedRows(
        segment_ids=pa.array(SEGMENT_IDS, type=pa.string()),
        anchors=anchors,
        field_ranges={"x": (anchors, anchors)},
    )
    table = schedule_samples(
        _sample_table(rows, ["x"]),
        strategy=strategy,
        fetch_size=FETCH_SIZE,
        num_ranks=num_ranks,
        num_workers_per_rank=1,
        seed=SEED,
    )
    meta = ManifestMeta(
        format_version=MANIFEST_FORMAT_VERSION,
        dataset_name="ds",
        dataset_id="id",
        index_name="t",
        ns_per_sample=None,
        ns_dtype=None,
        recipe={},
        required_fields=["x"],
        fetch_size=FETCH_SIZE,
        buffer_size=_buffer_size(strategy),
        min_fill=_min_fill(strategy),
        num_ranks=num_ranks,
        num_workers_per_rank=1,
        seed=SEED,
        shuffle_strategy=strategy.RECIPE_TAG,
    )
    return Manifest._from_arrow(table.replace_schema_metadata(_metadata_to_arrow(meta)))


def test_parquet_backed_manifest_reads_shards_lazily(tmp_path: Path) -> None:
    """A parquet-backed manifest reads only per-`(rank, worker)` shards, never the whole table into RAM."""
    mem = _build_manifest(BlockShuffle(buffer_size=FETCH_SIZE), num_ranks=2)
    path = tmp_path / "manifest.parquet"
    mem.write_parquet(path)

    lazy = Manifest.from_parquet(path)
    assert lazy._table is None  # not materialized on load

    # Header (row count, topology) comes from the parquet footer without loading rows.
    assert lazy.num_rows == mem.num_rows
    assert lazy.metadata.num_ranks == mem.metadata.num_ranks == 2
    assert lazy._table is None

    # Each shard reproduces the in-memory path exactly, still without materializing the whole table.
    for rank in range(2):
        mem_groups, mem_emit = mem.worker_plan(rank, 0)
        lazy_groups, lazy_emit = lazy.worker_plan(rank, 0)
        assert np.array_equal(mem_emit, lazy_emit)
        assert [g.to_pydict() for g in mem_groups] == [g.to_pydict() for g in lazy_groups]
    assert lazy._table is None

    # Pickling (as when shipping to a `DataLoader` worker) carries the path, not the rows.
    restored = pickle.loads(pickle.dumps(lazy))
    assert restored._table is None
    restored_groups, _ = restored.worker_plan(1, 0)
    assert [g.to_pydict() for g in restored_groups] == [g.to_pydict() for g in mem.worker_plan(1, 0)[0]]


def test_resolve_rows_walks_segments_and_drops_invalid() -> None:
    """`_resolve_rows` keeps only samples with a real row at/before them, segment by segment, freeing scan data."""
    sample_index = SampleIndex([
        SegmentMetadata(segment_id="a", index_start=0, index_end=4, num_samples=5),
        SegmentMetadata(segment_id="b", index_start=100, index_end=102, num_samples=3),
    ])
    # Segment "a" has no real row at index 0 or 1, so those two samples are dropped.
    scan = _ScanResult(
        keyframes={},
        real_by_entity={"/e": {"a": np.array([2, 3, 4]), "b": np.array([100, 101, 102])}},
    )

    rows = _resolve_rows(
        fields=_FIELDS,
        decoders={"x": _FIELDS["x"].decode},
        sample_index=sample_index,
        scan=scan,
        required={"x"},
    )

    assert rows.segment_ids.to_pylist() == ["a", "a", "a", "b", "b", "b"]
    assert rows.anchors.tolist() == [2, 3, 4, 100, 101, 102]
    lo, hi = rows.field_ranges["x"]
    assert lo.tolist() == hi.tolist() == [2, 3, 4, 100, 101, 102]  # scalar field: range is just the anchor
    assert scan.real_by_entity["/e"] == {}  # each segment's scan data released as it was resolved


def _patch_rank(monkeypatch: pytest.MonkeyPatch, rank: int, world_size: int) -> None:
    """Make both paths believe they run as `rank` of a `world_size` DDP job."""
    monkeypatch.setattr(torch.distributed, "is_available", lambda: True)
    monkeypatch.setattr(torch.distributed, "is_initialized", lambda: True)
    monkeypatch.setattr(torch.distributed, "get_rank", lambda: rank)
    monkeypatch.setattr(torch.distributed, "get_world_size", lambda: world_size)


def _build_live(strategy: ShuffleStrategy) -> RerunIterableDataset:
    """A live (manifest-less) dataset over the same fake dataset, strategy, seed, and topology."""
    live = RerunIterableDataset(
        _SOURCE,
        "t",
        _FIELDS,
        fetch_size=FETCH_SIZE,
        shuffle_strategy=strategy,
    )
    live.set_epoch(SEED)
    return live


def _stub_catalog(monkeypatch: pytest.MonkeyPatch, seg_tables: dict[str, dict[str, pa.Table]]) -> None:
    """Skip the server: the live index is the compact one, and both paths fetch `seg_tables` in place."""
    monkeypatch.setattr(iterable_dataset.SampleIndex, "build", lambda *_a, **_k: _compact_index(SEGMENT_IDS))
    monkeypatch.setattr(
        iterable_dataset._WorkerConnection,
        "ensure",
        lambda self: (None, {k: f.decode for k, f in self._fields.items()}),
    )
    # Manifest path: `targets_from_rows` already built the targets, so just hand back the tables.
    monkeypatch.setattr(iterable_dataset, "_fetch_targets", lambda targets, **_: (targets, seg_tables))

    # Live path: turn each fetch chunk of global sample indices into targets tagged by anchor.
    def fake_fetch_arrow(indices: np.ndarray, **_: object) -> tuple[list[Target], dict[str, dict[str, pa.Table]]]:
        targets = [
            Target(
                segment=SegmentMetadata(segment_id=SEGMENT_IDS[int(g)], index_start=0, index_end=0, num_samples=0),
                index_value=int(ANCHORS[int(g)]),
                anchors={},
            )
            for g in indices
        ]
        return targets, seg_tables

    monkeypatch.setattr(iterable_dataset, "_fetch_arrow", fake_fetch_arrow)


# Only `BlockShuffle` can carry an emission buffer; the other strategies emit in fetch order.
_STRATEGIES = [
    pytest.param(NoShuffle(), id="no-shuffle"),
    pytest.param(SampleShuffle(), id="uniform"),
    pytest.param(BlockShuffle(), id="block"),
    pytest.param(BlockShuffle(buffer_size=6), id="block+buffer"),
    pytest.param(BlockShuffle(buffer_size=6, min_fill=1), id="block+buffer+min_fill"),
]


@pytest.mark.filterwarnings("ignore::RuntimeWarning")  # the fork-safety warning is irrelevant here
@pytest.mark.parametrize("strategy", _STRATEGIES)
def test_manifest_replays_live_order_exactly(monkeypatch: pytest.MonkeyPatch, strategy: ShuffleStrategy) -> None:
    # Decode is irrelevant here: tag each sample with its anchor so we compare emission order only.
    monkeypatch.setattr(
        iterable_dataset, "_decode_iter", lambda *, targets, **_: ({"anchor": int(t.index_value)} for t in targets)
    )
    _stub_catalog(monkeypatch, seg_tables={"x": {}})

    manifest_order = [
        cast("int", s["anchor"])
        for s in RerunIterableDataset.from_manifest(_build_manifest(strategy), _SOURCE, _FIELDS)
    ]
    live_order = [cast("int", s["anchor"]) for s in _build_live(strategy)]

    assert manifest_order == live_order  # the contract: a manifest replays the live order sample-for-sample
    assert sorted(live_order) == sorted(ANCHORS)  # every sample exactly once

    reordered = strategy.RECIPE_TAG != "none" or strategy.emission_buffer() is not None
    assert (live_order != ANCHORS) == reordered  # shuffling (or a buffer) actually permutes; NoShuffle does not


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_manifest_replays_live_order_across_ranks(monkeypatch: pytest.MonkeyPatch) -> None:
    # A buffered block shuffle across 2 DDP ranks: this is the case that fails if the live
    # buffer seed drops `rank` (its emission order would no longer match the frozen manifest).
    strategy, world_size = BlockShuffle(buffer_size=6), 2
    manifest = _build_manifest(strategy, num_ranks=world_size)
    monkeypatch.setattr(
        iterable_dataset, "_decode_iter", lambda *, targets, **_: ({"anchor": int(t.index_value)} for t in targets)
    )
    _stub_catalog(monkeypatch, seg_tables={"x": {}})

    all_live: list[int] = []
    for rank in range(world_size):
        with monkeypatch.context() as m:
            _patch_rank(m, rank, world_size)
            manifest_order = [
                cast("int", s["anchor"]) for s in RerunIterableDataset.from_manifest(manifest, _SOURCE, _FIELDS)
            ]
            live_order = [cast("int", s["anchor"]) for s in _build_live(strategy)]
        assert manifest_order == live_order, f"rank {rank} replay diverged from the live run"
        all_live += live_order

    assert sorted(all_live) == sorted(ANCHORS)  # the ranks partition the dataset: no sample dropped or duplicated


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_manifest_and_live_decode_identical_content(monkeypatch: pytest.MonkeyPatch) -> None:
    # Real decode this time: one float32 scalar per sample, value derived from the anchor.
    by_segment: dict[str, dict[str, list[object]]] = {}
    for sid, anchor in zip(SEGMENT_IDS, ANCHORS, strict=True):
        cols = by_segment.setdefault(sid, {"t": [], "rerun_segment_id": [], "x": []})
        cols["t"].append(anchor)
        cols["rerun_segment_id"].append(sid)
        cols["x"].append(anchor * 1.5)
    seg_tables = {
        "x": {
            sid: pa.table({
                "t": pa.array(cols["t"], pa.int64()),
                "rerun_segment_id": pa.array(cols["rerun_segment_id"], pa.string()),
                "x": pa.array(cols["x"], pa.float32()),
            })
            for sid, cols in by_segment.items()
        }
    }
    _stub_catalog(monkeypatch, seg_tables=seg_tables)

    strategy = NoShuffle()
    manifest_samples = list(RerunIterableDataset.from_manifest(_build_manifest(strategy), _SOURCE, _FIELDS))
    live_samples = list(_build_live(strategy))

    by_anchor = {int(cast("torch.Tensor", s["x"]).item() / 1.5): s for s in manifest_samples}
    assert len(by_anchor) == len(ANCHORS)  # every sample decoded, once

    for live in live_samples:
        live_x = cast("torch.Tensor", live["x"])
        anchor = int(live_x.item() / 1.5)
        manifest = by_anchor[anchor]
        manifest_x = cast("torch.Tensor", manifest["x"])
        assert live.keys() == manifest.keys() == {"x"}  # same schema
        assert live_x.dtype == manifest_x.dtype == torch.float32  # same precision, preserved from Arrow
        assert live_x.shape == manifest_x.shape == (1,)  # same shape
        assert torch.equal(live_x, manifest_x)  # same values
        assert live_x.item() == pytest.approx(anchor * 1.5)
