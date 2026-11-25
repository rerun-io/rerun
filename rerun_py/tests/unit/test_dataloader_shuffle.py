"""Tests for `rerun.experimental.dataloader._shuffle`."""

from __future__ import annotations

import tracemalloc
from dataclasses import dataclass
from itertools import pairwise
from typing import TYPE_CHECKING, ClassVar, Literal

import numpy as np
import pytest
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._shuffle import (
    BlockShuffle,
    NoShuffle,
    SampleShuffle,
    ShuffleBuffer,
    ShuffleStrategy,
    _MaterializedWorkOrder,
)

if TYPE_CHECKING:
    from collections.abc import Generator

    import numpy.typing as npt


def _sample_index(*num_samples: int) -> SampleIndex:
    segments = [
        SegmentMetadata(segment_id=f"seg{i}", index_start=0, index_end=n - 1, num_samples=n)
        for i, n in enumerate(num_samples)
    ]
    return SampleIndex(segments)


def _blocks(indices: np.ndarray, block_bounds: np.ndarray) -> list[np.ndarray]:
    return [
        indices[start:end] for start, end in zip(np.concatenate([[0], block_bounds[:-1]]), block_bounds, strict=True)
    ]


@dataclass(frozen=True)
class GroupShuffle(ShuffleStrategy):
    """Yield temporally spread groups from each segment in random group order."""

    RECIPE_TAG: ClassVar[str] = "group"

    group_size: int
    groups_per_episode: int | None = None
    spacing: Literal["random", "even"] = "random"
    num_shards: int = 1

    def epoch_order(
        self,
        sample_index: SampleIndex,
        *,
        fetch_block_size: int,
        seed: int,
    ) -> tuple[npt.NDArray[np.int64], npt.NDArray[np.int64]]:
        del fetch_block_size
        rng = np.random.default_rng(seed=seed)

        groups: list[npt.NDArray[np.int64]] = []
        for start, end in pairwise(sample_index.segment_offsets):
            parts = np.array_split(np.arange(start, end, dtype=np.int64), self.group_size)
            fit = len(parts[-1])
            num_groups = fit if self.groups_per_episode is None else min(self.groups_per_episode, fit)
            if num_groups == 0:
                continue
            if self.spacing == "even":
                offsets = rng.choice(fit, size=num_groups, replace=False)
                picks = np.stack([part[offsets] for part in parts])
            else:
                picks = np.stack([rng.choice(part, size=num_groups, replace=False) for part in parts])
            groups.extend(picks.T)

        order = rng.permutation(len(groups))
        order = order[: len(order) - len(order) % self.num_shards]
        if len(order) == 0:
            return np.empty(0, dtype=np.int64), np.empty(0, dtype=np.int64)
        indices = np.concatenate([groups[i] for i in order])
        block_bounds = np.arange(self.group_size, len(indices) + 1, self.group_size, dtype=np.int64)
        return indices, block_bounds


def _materialized_block_order_reference(
    sample_index: SampleIndex,
    *,
    block_size: int,
    shuffle: bool,
    seed: int,
) -> tuple[np.ndarray, np.ndarray]:
    """Materialized reference that pins the established block ordering."""
    offsets = sample_index.segment_offsets
    total = int(offsets[-1])
    if total == 0:
        return np.empty(0, dtype=np.int64), np.empty(0, dtype=np.int64)

    block_ids = np.empty(total, dtype=np.int64)
    num_blocks = 0
    for start, end in pairwise(offsets):
        block_ids[start:end] = num_blocks + np.arange(end - start, dtype=np.int64) // block_size
        num_blocks += (end - start + block_size - 1) // block_size

    if shuffle:
        emitted_blocks = np.random.default_rng(seed=seed).permutation(num_blocks)
    else:
        emitted_blocks = np.arange(num_blocks)

    block_position = np.empty(num_blocks, dtype=np.int64)
    block_position[emitted_blocks] = np.arange(num_blocks)
    indices = np.argsort(block_position[block_ids], kind="stable").astype(np.int64)
    block_bounds = np.cumsum(np.bincount(block_ids, minlength=num_blocks)[emitted_blocks])
    return indices, block_bounds


def _materialized_epoch_order_reference(
    strategy: SampleShuffle | BlockShuffle | NoShuffle,
    sample_index: SampleIndex,
    *,
    fetch_block_size: int,
    seed: int,
) -> tuple[np.ndarray, np.ndarray]:
    if isinstance(strategy, BlockShuffle):
        return _materialized_block_order_reference(
            sample_index,
            block_size=fetch_block_size,
            shuffle=True,
            seed=seed,
        )
    if isinstance(strategy, NoShuffle):
        return _materialized_block_order_reference(
            sample_index,
            block_size=fetch_block_size,
            shuffle=False,
            seed=seed,
        )
    return strategy.epoch_order(sample_index, fetch_block_size=fetch_block_size, seed=seed)


def _materialized_contiguous_shard_reference(
    indices: np.ndarray,
    block_bounds: np.ndarray,
    *,
    rank: int,
    world_size: int,
) -> tuple[np.ndarray, np.ndarray]:
    """Materialized reference that pins nested rank and worker sharding."""
    per_shard = len(indices) // world_size
    start = rank * per_shard
    end = start + per_shard if rank < world_size - 1 else len(indices)
    inner_bounds = block_bounds[(block_bounds > start) & (block_bounds < end)] - start
    return indices[start:end], np.append(inner_bounds, end - start)


def _materialized_fetch_blocks_reference(
    indices: np.ndarray,
    block_bounds: np.ndarray,
    *,
    fetch_block_size: int,
) -> list[np.ndarray]:
    """Materialized reference that pins fetch packing and block splitting."""
    fetch_blocks: list[np.ndarray] = []
    fetch_start = 0
    packed_end = 0
    for raw_bound in block_bounds:
        bound = int(raw_bound)
        if bound - fetch_start <= fetch_block_size:
            packed_end = bound
            continue
        if packed_end > fetch_start:
            fetch_blocks.append(indices[fetch_start:packed_end])
            fetch_start = packed_end
        while bound - fetch_start > fetch_block_size:
            fetch_blocks.append(indices[fetch_start : fetch_start + fetch_block_size])
            fetch_start += fetch_block_size
        packed_end = bound
    if fetch_start < len(indices):
        fetch_blocks.append(indices[fetch_start:])
    return fetch_blocks


def _assert_blocks_segment_local(
    sample_index: SampleIndex,
    indices: np.ndarray,
    block_bounds: np.ndarray,
    *,
    contiguous: bool = True,
) -> None:
    """
    Every block must lie within one segment.

    With `contiguous`, the block must also cover a gap-free span of global
    indices; shard pieces of a cut block only stay within the block's span.
    """
    offsets = sample_index.segment_offsets
    for block in _blocks(indices, block_bounds):
        low, high = block.min(), block.max()
        if contiguous:
            assert high - low == len(block) - 1, "block indices must be contiguous"
        segment = np.searchsorted(offsets[1:], low, side="right")
        assert high < offsets[segment + 1], "block must not cross a segment boundary"


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(buffer_size=6), NoShuffle()])
def test_epoch_order_is_a_permutation(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> None:
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = strategy.epoch_order(sample_index, fetch_block_size=16, seed=0)
    assert np.array_equal(np.sort(indices), np.arange(sample_index.total_samples))
    assert block_bounds[-1] == sample_index.total_samples
    _assert_blocks_segment_local(sample_index, indices, block_bounds)


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(buffer_size=6), NoShuffle()])
def test_epoch_order_empty(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> None:
    sample_index = _sample_index()
    indices, block_bounds = strategy.epoch_order(sample_index, fetch_block_size=16, seed=0)
    work_order = strategy._work_order(sample_index, fetch_block_size=16, seed=0).compact()
    assert len(indices) == 0
    assert len(block_bounds) == 0
    assert work_order.num_samples == 0
    assert list(work_order.fetch_blocks(fetch_block_size=16)) == []


def test_no_shuffle_is_natural_order() -> None:
    sample_index = _sample_index(10, 5)
    indices, _ = NoShuffle().epoch_order(sample_index, fetch_block_size=4, seed=3)
    assert np.array_equal(indices, np.arange(15))


def test_sample_shuffle_seed_determinism() -> None:
    sample_index = _sample_index(64, 64)
    order_a, _ = SampleShuffle().epoch_order(sample_index, fetch_block_size=16, seed=1)
    order_b, _ = SampleShuffle().epoch_order(sample_index, fetch_block_size=16, seed=1)
    order_c, _ = SampleShuffle().epoch_order(sample_index, fetch_block_size=16, seed=2)
    assert np.array_equal(order_a, order_b)
    assert not np.array_equal(order_a, order_c)


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), NoShuffle()])
@pytest.mark.parametrize("seed", [0, 1, 4, 123])
@pytest.mark.parametrize("sizes", [(100, 33, 1, 50), (7,), (16, 16), (3, 0, 40)])
def test_work_order_matches_materialized_reference(
    strategy: SampleShuffle | BlockShuffle | NoShuffle,
    seed: int,
    sizes: tuple[int, ...],
) -> None:
    sample_index = _sample_index(*sizes)
    expected_indices, expected_bounds = _materialized_epoch_order_reference(
        strategy,
        sample_index,
        fetch_block_size=16,
        seed=seed,
    )

    epoch_indices, epoch_bounds = strategy.epoch_order(sample_index, fetch_block_size=16, seed=seed)
    assert np.array_equal(epoch_indices, expected_indices)
    assert np.array_equal(epoch_bounds, expected_bounds)

    work_order = strategy._work_order(sample_index, fetch_block_size=16, seed=seed)
    actual_indices, actual_bounds = work_order.materialize()
    assert np.array_equal(actual_indices, expected_indices)
    assert np.array_equal(actual_bounds, expected_bounds)
    assert [block.tolist() for block in work_order.fetch_blocks(fetch_block_size=16)] == [
        block.tolist()
        for block in _materialized_fetch_blocks_reference(
            expected_indices,
            expected_bounds,
            fetch_block_size=16,
        )
    ]


def test_block_shuffle_order_depends_on_seed() -> None:
    sample_index = _sample_index(100, 33, 1, 50)
    order_a, _ = BlockShuffle()._work_order(sample_index, fetch_block_size=16, seed=1).materialize()
    order_b, _ = BlockShuffle()._work_order(sample_index, fetch_block_size=16, seed=1).materialize()
    order_c, _ = BlockShuffle()._work_order(sample_index, fetch_block_size=16, seed=2).materialize()

    assert np.array_equal(order_a, order_b)
    assert not np.array_equal(order_a, order_c)


def test_custom_group_shuffle_preserves_groups_through_nested_sharding() -> None:
    sample_index = _sample_index(12, 20, 3)
    strategy = GroupShuffle(group_size=4, groups_per_episode=2, num_shards=4)
    indices, block_bounds = strategy.epoch_order(sample_index, fetch_block_size=6, seed=7)
    expected_groups = _blocks(indices, block_bounds)

    assert len(indices) < sample_index.total_samples
    assert len(expected_groups) == 4
    assert all(len(group) == strategy.group_size for group in expected_groups)
    assert all(np.all(np.diff(group) > 0) for group in expected_groups)
    _assert_blocks_segment_local(sample_index, indices, block_bounds, contiguous=False)

    work_order = strategy._work_order(sample_index, fetch_block_size=6, seed=7)
    materialized_indices, materialized_bounds = work_order.materialize()
    assert np.array_equal(materialized_indices, indices)
    assert np.array_equal(materialized_bounds, block_bounds)
    assert [block.tolist() for block in work_order.fetch_blocks(fetch_block_size=6)] == [
        group.tolist() for group in expected_groups
    ]

    shard_blocks = []
    for rank in range(2):
        rank_order = work_order.shard(rank=rank, world_size=2)
        for worker in range(2):
            worker_order = rank_order.shard(rank=worker, world_size=2).compact()
            blocks = list(worker_order.fetch_blocks(fetch_block_size=6))
            assert len(blocks) == 1
            assert len(blocks[0]) == strategy.group_size
            shard_blocks.extend(blocks)

    assert np.array_equal(np.concatenate(shard_blocks), indices)


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), NoShuffle()])
@pytest.mark.parametrize(
    ("sizes", "ddp_world_size", "num_workers"),
    [
        ((100, 33, 1, 50), 3, 2),
        ((100, 33, 1, 50), 1, 5),
        ((5,), 4, 3),
        ((40, 3), 2, 7),
    ],
)
def test_per_shard_fetch_blocks_match_materialized_reference(
    strategy: SampleShuffle | BlockShuffle | NoShuffle,
    sizes: tuple[int, ...],
    ddp_world_size: int,
    num_workers: int,
) -> None:
    sample_index = _sample_index(*sizes)
    fetch_block_size = 16
    expected_indices, expected_bounds = _materialized_epoch_order_reference(
        strategy,
        sample_index,
        fetch_block_size=fetch_block_size,
        seed=4,
    )
    work_order = strategy._work_order(sample_index, fetch_block_size=fetch_block_size, seed=4)

    for rank in range(ddp_world_size):
        if ddp_world_size > 1:
            rank_order = work_order.shard(rank=rank, world_size=ddp_world_size)
            rank_reference = _materialized_contiguous_shard_reference(
                expected_indices,
                expected_bounds,
                rank=rank,
                world_size=ddp_world_size,
            )
        else:
            rank_order = work_order
            rank_reference = expected_indices, expected_bounds

        for worker in range(num_workers):
            worker_order = rank_order.shard(rank=worker, world_size=num_workers).compact()
            worker_reference = _materialized_contiguous_shard_reference(
                *rank_reference,
                rank=worker,
                world_size=num_workers,
            )
            expected_blocks = _materialized_fetch_blocks_reference(
                *worker_reference,
                fetch_block_size=fetch_block_size,
            )
            actual_blocks = list(worker_order.fetch_blocks(fetch_block_size=fetch_block_size))

            assert [block.tolist() for block in actual_blocks] == [block.tolist() for block in expected_blocks]
            assert worker_order.num_samples == len(worker_reference[0])


@pytest.mark.parametrize("strategy", [BlockShuffle(), NoShuffle()])
def test_work_order_does_not_allocate_sample_sized_arrays(strategy: BlockShuffle | NoShuffle) -> None:
    # Four million int64 sample indices need about 32 MiB; descriptor-only orders stay well below 4 MiB.
    sample_index = _sample_index(1_500_000, 1_000_003, 1_500_000)

    tracemalloc.start()
    try:
        work_order = strategy._work_order(sample_index, fetch_block_size=1024, seed=3)
        work_order = work_order.shard(rank=1, world_size=2).shard(rank=0, world_size=3).compact()
        first_block = next(iter(work_order.fetch_blocks(fetch_block_size=1024)))
        _, peak_bytes = tracemalloc.get_traced_memory()
    finally:
        tracemalloc.stop()

    assert len(first_block) <= 1024
    assert peak_bytes < 4 * 1024 * 1024, f"Peak traced allocation was {peak_bytes} bytes"


def test_compacting_shards_that_share_one_block_clips_each_shard() -> None:
    sample_index = _sample_index(10)
    work_order = BlockShuffle()._work_order(sample_index, fetch_block_size=128, seed=0)

    shards = [work_order.shard(rank=rank, world_size=4).compact() for rank in range(4)]
    actual = np.concatenate([np.concatenate(list(shard.fetch_blocks(fetch_block_size=128))) for shard in shards])

    assert np.array_equal(actual, np.arange(10))


def test_block_shuffle_blocks_are_fetch_block_sized() -> None:
    # There is no separate block-size knob: a block is always one fetch wide, so BlockShuffle cuts
    # the sample space into exactly the same blocks as the natural-order strategy, only reordered.
    sample_index = _sample_index(100, 33, 1, 50)
    _, block_bounds = BlockShuffle().epoch_order(sample_index, fetch_block_size=16, seed=0)
    _, natural_bounds = NoShuffle().epoch_order(sample_index, fetch_block_size=16, seed=0)
    block_sizes = np.diff(np.concatenate([[0], block_bounds]))
    natural_sizes = np.diff(np.concatenate([[0], natural_bounds]))
    assert sorted(block_sizes.tolist()) == sorted(natural_sizes.tolist())
    assert block_sizes.max() == 16  # a full block is exactly one fetch wide


def test_block_shuffle_keeps_natural_order_within_blocks() -> None:
    # Within-block order must stay natural: reordering samples inside a block
    # would defeat decoder caching across consecutive samples.
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = BlockShuffle().epoch_order(sample_index, fetch_block_size=7, seed=0)
    assert not np.array_equal(indices, np.arange(sample_index.total_samples))
    for block in _blocks(indices, block_bounds):
        assert np.array_equal(block, np.arange(block[0], block[0] + len(block)))


def test_fetch_blocks_respect_block_bounds() -> None:
    # A materialized order must split a block wider than a fetch and greedily pack whole small blocks.
    # A 20-wide block followed by blocks of 2, 2, and 6 exercises both paths.
    indices = np.arange(30, dtype=np.int64)
    block_bounds = np.array([20, 22, 24, 30], dtype=np.int64)

    work_order = _MaterializedWorkOrder(indices, block_bounds, 0, len(indices))
    fetch_blocks = list(work_order.fetch_blocks(fetch_block_size=16))

    assert all(len(fetch_block) <= 16 for fetch_block in fetch_blocks)
    assert np.array_equal(np.concatenate(fetch_blocks), indices)
    # Each fetch block stays within a small number of contiguous spans: no fetch block
    # mixes a split-block tail with the head of an unrelated block.
    bound_set = {int(b) for b in block_bounds}
    position = 0
    for fetch_block in fetch_blocks:
        position += len(fetch_block)
        if len(fetch_block) < 16:
            assert position in bound_set, "short fetch blocks may only end at a block boundary"


def test_shuffle_buffer_emits_each_item_once() -> None:
    buffer = ShuffleBuffer(8)
    items = list(range(100))
    out = list(buffer.shuffle((i for i in items), rng=np.random.default_rng(0)))
    assert sorted(out) == items
    assert out != items


def test_shuffle_buffer_determinism() -> None:
    buffer = ShuffleBuffer(8)
    out_a = list(buffer.shuffle((i for i in range(50)), rng=np.random.default_rng(1)))
    out_b = list(buffer.shuffle((i for i in range(50)), rng=np.random.default_rng(1)))
    out_c = list(buffer.shuffle((i for i in range(50)), rng=np.random.default_rng(2)))
    assert out_a == out_b
    assert out_a != out_c


def test_shuffle_buffer_holds_at_most_buffer_size() -> None:
    buffer_size = 4
    buffer = ShuffleBuffer(buffer_size)
    consumed = 0

    def source() -> Generator[int, None, None]:
        nonlocal consumed
        for i in range(20):
            consumed = i + 1
            yield i

    shuffled = buffer.shuffle(source(), rng=np.random.default_rng(0))
    emitted = [next(shuffled) for _ in range(5)]
    # At most buffer_size items are held beyond what was emitted, and the
    # source is consumed lazily, not exhausted up front.
    assert consumed <= buffer_size + len(emitted)
    assert consumed < 20
    shuffled.close()


def test_shuffle_buffer_emits_before_full() -> None:
    buffer = ShuffleBuffer(64)
    consumed = 0

    def source() -> Generator[int, None, None]:
        nonlocal consumed
        for i in range(1000):
            consumed = i + 1
            yield i

    shuffled = buffer.shuffle(source(), rng=np.random.default_rng(0))
    next(shuffled)
    # The first item is emitted once min_fill (half the buffer) is reached.
    assert consumed == 32
    shuffled.close()


def test_shuffle_buffer_input_shorter_than_buffer() -> None:
    buffer = ShuffleBuffer(64)
    out = list(buffer.shuffle((i for i in range(5)), rng=np.random.default_rng(0)))
    assert sorted(out) == list(range(5))


def test_shuffle_buffer_closes_source() -> None:
    closed = False

    def source() -> Generator[int, None, None]:
        nonlocal closed
        try:
            yield from range(100)
        finally:
            closed = True

    shuffled = ShuffleBuffer(8).shuffle(source(), rng=np.random.default_rng(0))
    next(shuffled)
    shuffled.close()
    assert closed


def test_shuffle_buffer_rejects_invalid_size() -> None:
    with pytest.raises(ValueError, match="buffer_size"):
        ShuffleBuffer(1)
    with pytest.raises(ValueError, match="min_fill"):
        ShuffleBuffer(8, min_fill=0)
    with pytest.raises(ValueError, match="min_fill"):
        ShuffleBuffer(8, min_fill=9)


# --- Fetch locality: the property that actually drives throughput -------------------------------
#
# A server fetch is cheap when it reads one contiguous, segment-local span (decoders stay warm and
# stored data is read once); it is expensive when it scatters across segments (cold decode heads,
# random reads). `NoShuffle` and `BlockShuffle` block at `fetch_block_size`, so every fetch is one
# contiguous span; `SampleShuffle` scatters every sample. There is no separate block-size knob to
# get this wrong. These tests hand-check that guarantee on the two-segment dataset below so a
# refactor that reintroduces a partial-locality mode — or routes a config to the wrong strategy —
# fails here instead of only showing up as an unexplained throughput delta.
#
# Two segments: "a" is global 0..7, "b" is global 8..11; with fetch_block_size == 4 every result is small
# enough to read by eye.

_LOC_SEG_A, _LOC_SEG_B = 8, 4
_LOC_TOTAL = _LOC_SEG_A + _LOC_SEG_B
_LOC_FETCH = 4
_LOC_SEED = 3
_SEGMENT_OF = np.array([0] * _LOC_SEG_A + [1] * _LOC_SEG_B)


def _loc_fetch_blocks(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> list[list[int]]:
    """The server fetches a strategy produces for one epoch of the two-segment dataset."""
    sample_index = _sample_index(_LOC_SEG_A, _LOC_SEG_B)
    work_order = strategy._work_order(sample_index, fetch_block_size=_LOC_FETCH, seed=_LOC_SEED)
    return [block.tolist() for block in work_order.fetch_blocks(fetch_block_size=_LOC_FETCH)]


def _contiguous_fetches(fetch_blocks: list[list[int]]) -> int:
    """How many fetches are a single ascending run of consecutive indices (one contiguous read)."""
    return sum(len(c) <= 1 or bool(np.all(np.diff(c) == 1)) for c in fetch_blocks)


def _covers_every_sample(fetch_blocks: list[list[int]]) -> bool:
    return sorted(i for c in fetch_blocks for i in c) == list(range(_LOC_TOTAL))


def _no_fetch_crosses_a_segment(fetch_blocks: list[list[int]]) -> bool:
    return all(len(set(_SEGMENT_OF[c].tolist())) == 1 for c in fetch_blocks)


def test_no_shuffle_fetches_are_all_contiguous() -> None:
    fetch_blocks = _loc_fetch_blocks(NoShuffle())
    assert fetch_blocks == [[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11]]
    assert _contiguous_fetches(fetch_blocks) == len(fetch_blocks)


def test_block_shuffle_has_same_locality_as_no_shuffle() -> None:
    # Blocks are one fetch wide, so every fetch is still one contiguous, segment-local span —
    # identical read locality to NoShuffle, only the block *order* is permuted. This is why `none`
    # and `block` benchmark within noise of each other.
    fetch_blocks = _loc_fetch_blocks(BlockShuffle())
    assert _contiguous_fetches(fetch_blocks) == len(fetch_blocks)
    assert _covers_every_sample(fetch_blocks)
    assert _no_fetch_crosses_a_segment(fetch_blocks)
    assert fetch_blocks != _loc_fetch_blocks(NoShuffle())  # order really is permuted


def test_block_shuffle_locality_cannot_be_downgraded() -> None:
    # The footgun behind the original `block` regression was a block size smaller than the fetch,
    # which silently turned each contiguous fetch into a scattered one. That knob no longer exists,
    # so `block` locality can't be downgraded by configuration — only `buffer_size` / `min_fill`
    # remain, and both are emission-time, not fetch-time.
    assert not hasattr(BlockShuffle(), "block_size")
    assert "block_size" not in BlockShuffle.__dataclass_fields__


def test_sample_shuffle_scatters_and_ignores_fetch_block_size() -> None:
    # Per-sample shuffle has no block structure: it scatters every sample (zero locality), and its
    # emission order depends on neither a block size (there is none) nor `fetch_block_size` — fetching
    # only batches the network I/O.
    assert _contiguous_fetches(_loc_fetch_blocks(SampleShuffle())) == 0
    assert _covers_every_sample(_loc_fetch_blocks(SampleShuffle()))
    sample_index = _sample_index(_LOC_SEG_A, _LOC_SEG_B)
    order_small, _ = SampleShuffle().epoch_order(sample_index, fetch_block_size=2, seed=_LOC_SEED)
    order_large, _ = SampleShuffle().epoch_order(sample_index, fetch_block_size=999, seed=_LOC_SEED)
    assert np.array_equal(order_small, order_large)  # identical order regardless of fetch_block_size


def test_locality_ranking_none_equals_block_and_beats_sample() -> None:
    # The whole throughput story in one assertion: NoShuffle and BlockShuffle are fully local,
    # SampleShuffle is fully scattered. Any benchmark where `block` sits well below `none` is
    # therefore not measuring "block shuffle" at all.
    assert _contiguous_fetches(_loc_fetch_blocks(NoShuffle())) == 3
    assert _contiguous_fetches(_loc_fetch_blocks(BlockShuffle())) == 3
    assert _contiguous_fetches(_loc_fetch_blocks(SampleShuffle())) == 0


@pytest.mark.parametrize(
    "strategy",
    [
        BlockShuffle(buffer_size=6),
        BlockShuffle(buffer_size=6, min_fill=1),
        BlockShuffle(buffer_size=_LOC_TOTAL),
    ],
)
def test_emission_buffer_leaves_fetch_order_unchanged(strategy: BlockShuffle) -> None:
    # A buffer reorders *decoded* samples; it must never change what gets fetched or in what
    # order, so it cannot affect fetch-locality or throughput — only startup latency and mixing.
    baseline = BlockShuffle()
    assert strategy.emission_buffer() is not None
    assert baseline.emission_buffer() is None
    assert _loc_fetch_blocks(strategy) == _loc_fetch_blocks(baseline)


def test_only_block_shuffle_defines_an_emission_buffer() -> None:
    assert NoShuffle().emission_buffer() is None
    assert SampleShuffle().emission_buffer() is None
    assert BlockShuffle().emission_buffer() is None  # block, but no buffer requested
    assert BlockShuffle(buffer_size=4).emission_buffer() is not None
