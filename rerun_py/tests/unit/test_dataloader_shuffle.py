"""Tests for `rerun.experimental.dataloader._shuffle`."""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pytest
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._shuffle import (
    BlockShuffle,
    NoShuffle,
    SampleShuffle,
    ShuffleBuffer,
    _contiguous_shard,
    _fetch_chunks,
)

if TYPE_CHECKING:
    from collections.abc import Generator


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


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(block_size=7), NoShuffle()])
def test_epoch_order_is_a_permutation(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> None:
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = strategy.epoch_order(sample_index, fetch_size=16, seed=0)
    assert np.array_equal(np.sort(indices), np.arange(sample_index.total_samples))
    assert block_bounds[-1] == sample_index.total_samples
    _assert_blocks_segment_local(sample_index, indices, block_bounds)


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(block_size=7), NoShuffle()])
def test_epoch_order_empty(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> None:
    indices, block_bounds = strategy.epoch_order(_sample_index(), fetch_size=16, seed=0)
    assert len(indices) == 0
    assert len(block_bounds) == 0


def test_no_shuffle_is_natural_order() -> None:
    sample_index = _sample_index(10, 5)
    indices, _ = NoShuffle().epoch_order(sample_index, fetch_size=4, seed=3)
    assert np.array_equal(indices, np.arange(15))


def test_sample_shuffle_seed_determinism() -> None:
    sample_index = _sample_index(64, 64)
    order_a, _ = SampleShuffle().epoch_order(sample_index, fetch_size=16, seed=1)
    order_b, _ = SampleShuffle().epoch_order(sample_index, fetch_size=16, seed=1)
    order_c, _ = SampleShuffle().epoch_order(sample_index, fetch_size=16, seed=2)
    assert np.array_equal(order_a, order_b)
    assert not np.array_equal(order_a, order_c)


def test_block_shuffle_block_size_defaults_to_fetch_size() -> None:
    sample_index = _sample_index(64)
    _, bounds_default = BlockShuffle().epoch_order(sample_index, fetch_size=16, seed=0)
    _, bounds_explicit = BlockShuffle(block_size=16).epoch_order(sample_index, fetch_size=99, seed=0)
    assert np.array_equal(bounds_default, bounds_explicit)


def test_block_shuffle_keeps_natural_order_within_blocks() -> None:
    # Within-block order must stay natural: reordering samples inside a block
    # would defeat decoder caching across consecutive samples.
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = BlockShuffle(block_size=7).epoch_order(sample_index, fetch_size=16, seed=0)
    assert not np.array_equal(indices, np.arange(sample_index.total_samples))
    for block in _blocks(indices, block_bounds):
        assert np.array_equal(block, np.arange(block[0], block[0] + len(block)))


def test_block_shuffle_rejects_invalid_block_size() -> None:
    with pytest.raises(ValueError, match="block_size"):
        BlockShuffle(block_size=0)


def test_contiguous_shard_partitions_evenly() -> None:
    sample_index = _sample_index(100, 33, 50)
    indices, block_bounds = BlockShuffle(block_size=8).epoch_order(sample_index, fetch_size=16, seed=0)

    world_size = 4
    shards = [_contiguous_shard(indices, block_bounds, rank=rank, world_size=world_size) for rank in range(world_size)]

    sizes = [len(shard_indices) for shard_indices, _ in shards]
    assert max(sizes) - min(sizes) <= world_size - 1
    assert np.array_equal(np.sort(np.concatenate([shard_indices for shard_indices, _ in shards])), np.sort(indices))
    for shard_indices, shard_bounds in shards:
        assert shard_bounds[-1] == len(shard_indices)
        # Pieces of a block cut at the shard boundary stay within the block's
        # span (and thus one segment), but are no longer gap-free themselves.
        _assert_blocks_segment_local(sample_index, shard_indices, shard_bounds, contiguous=False)
        for block in _blocks(shard_indices, shard_bounds):
            assert block.max() - block.min() < 8, "shard block pieces must stay within one block span"


def test_fetch_chunks_respect_block_bounds() -> None:
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = BlockShuffle(block_size=24).epoch_order(sample_index, fetch_size=16, seed=0)

    chunks = _fetch_chunks(indices, block_bounds, fetch_size=16)

    assert all(len(chunk) <= 16 for chunk in chunks)
    assert np.array_equal(np.concatenate(chunks), indices)
    # Each chunk stays within a small number of contiguous spans: no chunk
    # mixes a split-block tail with the head of an unrelated block.
    bound_set = {int(b) for b in block_bounds}
    position = 0
    for chunk in chunks:
        position += len(chunk)
        if len(chunk) < 16:
            assert position in bound_set, "short chunks may only end at a block boundary"


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
