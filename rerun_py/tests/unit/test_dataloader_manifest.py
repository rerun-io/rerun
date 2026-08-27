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
import rerun.experimental.dataloader.manifest._manifest_build as manifest_build
import torch
from rerun.experimental.dataloader import (
    BlockShuffle,
    Field,
    Manifest,
    NoShuffle,
    NumericDecoder,
    RerunIterableDataset,
    RerunMapDataset,
    SampleShuffle,
    VideoFrameDecoder,
    _iterable_dataset as iterable_dataset,
    _map_dataset as map_dataset,
)
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import FetchedGroup, QueryPlan
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
    _too_far_back,
    schedule_samples,
)

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog._entry import DatasetEntry
    from rerun.experimental.dataloader._config import DataSource
    from rerun.experimental.dataloader._shuffle import ShuffleStrategy

# A tiny fake dataset in canonical (segment ascending, anchor ascending) order: 12 samples
# across two segments, with globally unique anchors so a sample is identified by its anchor.
SEGMENT_IDS = ["a"] * 8 + ["b"] * 4
ANCHORS = [*range(8), *range(100, 104)]
SEED = 3
FETCH_BLOCK_SIZE = 4

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
        fetch_block_size=FETCH_BLOCK_SIZE,
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
        fetch_block_size=FETCH_BLOCK_SIZE,
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
    mem = _build_manifest(BlockShuffle(buffer_size=FETCH_BLOCK_SIZE), num_ranks=2)
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
        sample_index=sample_index,
        scan=scan,
        required={"x"},
    )

    assert rows.segment_ids.to_pylist() == ["a", "a", "a", "b", "b", "b"]
    assert rows.anchors.tolist() == [2, 3, 4, 100, 101, 102]
    lo, hi = rows.field_ranges["x"]
    assert lo.tolist() == hi.tolist() == [2, 3, 4, 100, 101, 102]  # scalar field: range is just the anchor
    assert scan.real_by_entity["/e"] == {}  # each segment's scan data released as it was resolved


def test_windowed_video_validity_is_covered_by_its_prior_keyframe() -> None:
    sample_index = SampleIndex([
        SegmentMetadata(segment_id="a", index_start=2, index_end=3, num_samples=2),
    ])
    field = Field(
        "/video:VideoStream:sample",
        decode=VideoFrameDecoder(),
        window=(-1, 0),
    )
    scan = _ScanResult(
        keyframes={"video": {"a": np.array([0], dtype=np.int64)}},
        real_by_entity={},
    )

    rows = _resolve_rows(
        fields={"video": field},
        sample_index=sample_index,
        scan=scan,
        required={"video"},
    )

    assert rows.anchors.tolist() == [2, 3]
    lo, hi = rows.field_ranges["video"]
    assert lo.tolist() == [0, 0]
    assert hi.tolist() == [2, 3]


def test_manifest_video_scan_never_fetches_frame_timestamps(monkeypatch: pytest.MonkeyPatch) -> None:
    sample_index = SampleIndex([
        SegmentMetadata(segment_id="a", index_start=2, index_end=3, num_samples=2),
    ])
    video = Field(
        "/video:VideoStream:sample",
        decode=VideoFrameDecoder(),
        window=(-1, 0),
        max_staleness=2,
    )
    state = Field("/state:Scalars:scalars", decode=NumericDecoder())
    keyframes = {"video": {"a": np.array([0], dtype=np.int64)}}
    fetched_entities: list[str] = []

    monkeypatch.setattr(manifest_build, "_fetch_prior_keyframes", lambda **_: keyframes)

    def fetch_entity_index_values(*, entity: str, **_: object) -> np.ndarray:
        fetched_entities.append(entity)
        return np.array([2, 3], dtype=np.int64)

    monkeypatch.setattr(manifest_build, "_fetch_entity_index_values", fetch_entity_index_values)

    scan = manifest_build._scan(
        view=cast("DatasetEntry", SimpleNamespace()),
        index="frame",
        fields={"video": video, "state": state},
        sample_index=sample_index,
        segment_maxes=[(sample_index.segments[0], 3)],
        required={"video", "state"},
        max_workers=1,
    )

    assert fetched_entities == ["/state"]
    assert scan.keyframes == keyframes
    assert scan.real_by_entity["/state"]["a"].tolist() == [2, 3]


def test_video_staleness_is_conservatively_bounded_by_prior_keyframes() -> None:
    sample_index = SampleIndex([
        SegmentMetadata(segment_id="a", index_start=2, index_end=3, num_samples=2),
    ])
    field = Field(
        "/video:VideoStream:sample",
        decode=VideoFrameDecoder(),
        window=(-1, 0),
        max_staleness=2,
    )
    scan = _ScanResult(
        keyframes={"video": {"a": np.array([0], dtype=np.int64)}},
        real_by_entity={},
    )

    rows = _resolve_rows(
        fields={"video": field},
        sample_index=sample_index,
        scan=scan,
        required={"video"},
    )

    assert rows.anchors.tolist() == [2]
    lo, hi = rows.field_ranges["video"]
    assert lo.tolist() == [0]
    assert hi.tolist() == [2]


def test_temporal_max_staleness_is_expressed_in_seconds() -> None:
    sample_index = SampleIndex([], ns_dtype="datetime64[ns]")
    field = Field("/e:Scalars:scalars", decode=NumericDecoder(), max_staleness=0.5)
    real = np.array([1_000_000_000], dtype=np.int64)

    assert not _too_far_back(real, 1_500_000_000, field=field, sample_index=sample_index)
    assert _too_far_back(real, 1_500_000_001, field=field, sample_index=sample_index)


def test_integer_max_staleness_must_be_integral() -> None:
    sample_index = SampleIndex([])
    field = Field("/e:Scalars:scalars", decode=NumericDecoder(), max_staleness=0.5)

    with pytest.raises(ValueError, match="integral max_staleness"):
        _too_far_back(np.array([0], dtype=np.int64), 1, field=field, sample_index=sample_index)


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
        fetch_block_size=FETCH_BLOCK_SIZE,
        shuffle_strategy=strategy,
    )
    live.set_epoch(SEED)
    return live


def _stub_catalog(monkeypatch: pytest.MonkeyPatch, fetched_groups: list[FetchedGroup]) -> None:
    """Skip the server: the live index is the compact one, and both paths reuse `fetched_groups`."""
    monkeypatch.setattr(iterable_dataset.SampleIndex, "build", lambda *_a, **_k: _compact_index(SEGMENT_IDS))
    monkeypatch.setattr(
        iterable_dataset._WorkerConnection,
        "ensure",
        lambda self: (None, {k: f.decode for k, f in self._fields.items()}),
    )
    monkeypatch.setattr(
        iterable_dataset,
        "_locate_samples",
        lambda indices, **_: [
            (
                SegmentMetadata(segment_id=SEGMENT_IDS[int(i)], index_start=0, index_end=0, num_samples=0),
                int(ANCHORS[int(i)]),
            )
            for i in indices
        ],
    )

    def fetch_queries(plans: list[QueryPlan], **_: object) -> list[FetchedGroup]:
        if not fetched_groups:
            return []
        return [
            FetchedGroup(fields=group.fields, fetch_requests=plan.fetch_requests, table=group.table)
            for group, plan in zip(fetched_groups, plans, strict=True)
        ]

    monkeypatch.setattr(iterable_dataset, "_fetch_queries_parallel", fetch_queries)


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
    monkeypatch.setattr(iterable_dataset, "_resolve_decode_requests_in_block", lambda indexed, **_: indexed)
    monkeypatch.setattr(
        iterable_dataset,
        "_decode_iter",
        lambda *, prepared, **_: ({"anchor": int(t.index_value), "x": torch.ones(1)} for t in prepared.targets),
    )
    _stub_catalog(monkeypatch, fetched_groups=[])

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
    monkeypatch.setattr(iterable_dataset, "_resolve_decode_requests_in_block", lambda indexed, **_: indexed)
    monkeypatch.setattr(
        iterable_dataset,
        "_decode_iter",
        lambda *, prepared, **_: ({"anchor": int(t.index_value), "x": torch.ones(1)} for t in prepared.targets),
    )
    _stub_catalog(monkeypatch, fetched_groups=[])

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


def _stub_map_catalog(monkeypatch: pytest.MonkeyPatch, fetched_groups: list[FetchedGroup]) -> None:
    """Skip the server for the map path: hand back `fetched_groups` in place of the resolved fetch."""
    monkeypatch.setattr(
        map_dataset._WorkerConnection,
        "ensure",
        lambda self: (None, {k: f.decode for k, f in self._fields.items()}),
    )

    def fetch_queries(plans: list[QueryPlan], **_: object) -> list[FetchedGroup]:
        if not fetched_groups:
            return []
        return [
            FetchedGroup(fields=group.fields, fetch_requests=plan.fetch_requests, table=group.table)
            for group, plan in zip(fetched_groups, plans, strict=True)
        ]

    monkeypatch.setattr(map_dataset, "_fetch_queries_parallel", fetch_queries)


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
@pytest.mark.parametrize("strategy", _STRATEGIES)
def test_map_manifest_yields_full_sample_set(monkeypatch: pytest.MonkeyPatch, strategy: ShuffleStrategy) -> None:
    """A map dataset over a manifest exposes every validated sample once, whatever strategy built it (order is the sampler's job, not the manifest's)."""
    monkeypatch.setattr(map_dataset, "_resolve_decode_requests_in_block", lambda indexed, **_: indexed)
    monkeypatch.setattr(
        map_dataset,
        "_decode_iter",
        lambda *, prepared, **_: ({"anchor": int(t.index_value)} for t in prepared.targets),
    )
    _stub_map_catalog(monkeypatch, fetched_groups=[])

    dataset = RerunMapDataset.from_manifest(_build_manifest(strategy), _SOURCE, _FIELDS)
    assert len(dataset) == len(ANCHORS)

    anchors = [cast("int", s["anchor"]) for s in dataset.__getitems__(list(range(len(dataset))))]
    assert sorted(anchors) == sorted(ANCHORS)  # every validated sample, exactly once


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_map_manifest_decodes_frozen_ranges(monkeypatch: pytest.MonkeyPatch) -> None:
    """`__getitems__` decodes each requested manifest row from its frozen range (real decode, catalog stubbed)."""
    # One table for the whole read group, as the server returns it: grouped by segment.
    by_segment: dict[str, list[int]] = {}
    for sid, anchor in zip(SEGMENT_IDS, ANCHORS, strict=True):
        by_segment.setdefault(sid, []).append(int(anchor))
    segment_ids = [sid for sid, anchors in by_segment.items() for _ in anchors]
    anchors_flat = [anchor for anchors in by_segment.values() for anchor in sorted(anchors)]
    group_table = pa.table({
        "t": pa.array(anchors_flat, pa.int64()),
        "rerun_segment_id": pa.array(segment_ids, pa.string()),
        "x": pa.array([[anchor * 1.5] for anchor in anchors_flat], pa.list_(pa.float32())),
    })
    _stub_map_catalog(monkeypatch, fetched_groups=[FetchedGroup(fields=_FIELDS, fetch_requests={}, table=group_table)])

    dataset = RerunMapDataset.from_manifest(_build_manifest(NoShuffle()), _SOURCE, _FIELDS)
    samples = dataset.__getitems__(list(range(len(dataset))))

    values = sorted(cast("torch.Tensor", s["x"]).item() for s in samples)
    assert values == pytest.approx(sorted(a * 1.5 for a in ANCHORS))  # each row decoded from its own anchor


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_manifest_and_live_decode_identical_content(monkeypatch: pytest.MonkeyPatch) -> None:
    # Real decode this time: one float32 scalar per sample, value derived from the anchor.
    # One table for the whole read group, as the server returns it: grouped by segment.
    by_segment: dict[str, list[int]] = {}
    for sid, anchor in zip(SEGMENT_IDS, ANCHORS, strict=True):
        by_segment.setdefault(sid, []).append(int(anchor))
    segment_ids = [sid for sid, anchors in by_segment.items() for _ in anchors]
    anchors_flat = [anchor for anchors in by_segment.values() for anchor in sorted(anchors)]
    group_table = pa.table({
        "t": pa.array(anchors_flat, pa.int64()),
        "rerun_segment_id": pa.array(segment_ids, pa.string()),
        "x": pa.array([[anchor * 1.5] for anchor in anchors_flat], pa.list_(pa.float32())),
    })
    _stub_catalog(monkeypatch, fetched_groups=[FetchedGroup(fields=_FIELDS, fetch_requests={}, table=group_table)])

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
