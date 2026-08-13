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


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(buffer_size=6), NoShuffle()])
def test_epoch_order_is_a_permutation(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> None:
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = strategy.epoch_order(sample_index, fetch_size=16, seed=0)
    assert np.array_equal(np.sort(indices), np.arange(sample_index.total_samples))
    assert block_bounds[-1] == sample_index.total_samples
    _assert_blocks_segment_local(sample_index, indices, block_bounds)


@pytest.mark.parametrize("strategy", [SampleShuffle(), BlockShuffle(), BlockShuffle(buffer_size=6), NoShuffle()])
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


def test_block_shuffle_blocks_are_fetch_sized() -> None:
    # There is no separate block-size knob: a block is always one fetch wide, so BlockShuffle cuts
    # the sample space into exactly the same blocks as the natural-order strategy, only reordered.
    sample_index = _sample_index(100, 33, 1, 50)
    _, block_bounds = BlockShuffle().epoch_order(sample_index, fetch_size=16, seed=0)
    _, natural_bounds = NoShuffle().epoch_order(sample_index, fetch_size=16, seed=0)
    block_sizes = np.diff(np.concatenate([[0], block_bounds]))
    natural_sizes = np.diff(np.concatenate([[0], natural_bounds]))
    assert sorted(block_sizes.tolist()) == sorted(natural_sizes.tolist())
    assert block_sizes.max() == 16  # a full block is exactly one fetch wide


def test_block_shuffle_keeps_natural_order_within_blocks() -> None:
    # Within-block order must stay natural: reordering samples inside a block
    # would defeat decoder caching across consecutive samples.
    sample_index = _sample_index(100, 33, 1, 50)
    indices, block_bounds = BlockShuffle().epoch_order(sample_index, fetch_size=7, seed=0)
    assert not np.array_equal(indices, np.arange(sample_index.total_samples))
    for block in _blocks(indices, block_bounds):
        assert np.array_equal(block, np.arange(block[0], block[0] + len(block)))


def test_contiguous_shard_partitions_evenly() -> None:
    sample_index = _sample_index(100, 33, 50)
    indices, block_bounds = BlockShuffle().epoch_order(sample_index, fetch_size=8, seed=0)

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
    # Drive `_fetch_chunks` directly: it must split a block wider than a fetch and greedily pack
    # whole small blocks up to fetch_size. A 20-wide block (wider than fetch_size=16) then blocks
    # of 2, 2, 6 exercises both paths in one go.
    indices = np.arange(30, dtype=np.int64)
    block_bounds = np.array([20, 22, 24, 30], dtype=np.int64)

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


# --- Fetch locality: the property that actually drives throughput -------------------------------
#
# A server fetch is cheap when it reads one contiguous, segment-local span (decoders stay warm and
# stored data is read once); it is expensive when it scatters across segments (cold decode heads,
# random reads). `NoShuffle` and `BlockShuffle` block at `fetch_size`, so every fetch is one
# contiguous span; `SampleShuffle` scatters every sample. There is no separate block-size knob to
# get this wrong. These tests hand-check that guarantee on the two-segment dataset below so a
# refactor that reintroduces a partial-locality mode — or routes a config to the wrong strategy —
# fails here instead of only showing up as an unexplained throughput delta.
#
# Two segments: "a" is global 0..7, "b" is global 8..11; with fetch_size == 4 every result is small
# enough to read by eye.

_LOC_SEG_A, _LOC_SEG_B = 8, 4
_LOC_TOTAL = _LOC_SEG_A + _LOC_SEG_B
_LOC_FETCH = 4
_LOC_SEED = 3
_SEGMENT_OF = np.array([0] * _LOC_SEG_A + [1] * _LOC_SEG_B)


def _loc_chunks(strategy: SampleShuffle | BlockShuffle | NoShuffle) -> list[list[int]]:
    """The server fetches a strategy produces for one epoch of the two-segment dataset."""
    sample_index = _sample_index(_LOC_SEG_A, _LOC_SEG_B)
    indices, bounds = strategy.epoch_order(sample_index, fetch_size=_LOC_FETCH, seed=_LOC_SEED)
    return [c.tolist() for c in _fetch_chunks(indices, bounds, fetch_size=_LOC_FETCH)]


def _contiguous_fetches(chunks: list[list[int]]) -> int:
    """How many fetches are a single ascending run of consecutive indices (one contiguous read)."""
    return sum(len(c) <= 1 or bool(np.all(np.diff(c) == 1)) for c in chunks)


def _covers_every_sample(chunks: list[list[int]]) -> bool:
    return sorted(i for c in chunks for i in c) == list(range(_LOC_TOTAL))


def _no_fetch_crosses_a_segment(chunks: list[list[int]]) -> bool:
    return all(len(set(_SEGMENT_OF[c].tolist())) == 1 for c in chunks)


def test_no_shuffle_fetches_are_all_contiguous() -> None:
    chunks = _loc_chunks(NoShuffle())
    assert chunks == [[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11]]
    assert _contiguous_fetches(chunks) == len(chunks)


def test_block_shuffle_has_same_locality_as_no_shuffle() -> None:
    # Blocks are one fetch wide, so every fetch is still one contiguous, segment-local span —
    # identical read locality to NoShuffle, only the block *order* is permuted. This is why `none`
    # and `block` benchmark within noise of each other.
    chunks = _loc_chunks(BlockShuffle())
    assert _contiguous_fetches(chunks) == len(chunks)
    assert _covers_every_sample(chunks)
    assert _no_fetch_crosses_a_segment(chunks)
    assert chunks != _loc_chunks(NoShuffle())  # order really is permuted


def test_block_shuffle_locality_cannot_be_downgraded() -> None:
    # The footgun behind the original `block` regression was a block size smaller than the fetch,
    # which silently turned each contiguous fetch into a scattered one. That knob no longer exists,
    # so `block` locality can't be downgraded by configuration — only `buffer_size` / `min_fill`
    # remain, and both are emission-time, not fetch-time.
    assert not hasattr(BlockShuffle(), "block_size")
    assert "block_size" not in BlockShuffle.__dataclass_fields__


def test_sample_shuffle_scatters_and_ignores_fetch_size() -> None:
    # Per-sample shuffle has no block structure: it scatters every sample (zero locality), and its
    # emission order depends on neither a block size (there is none) nor `fetch_size` — fetching
    # only batches the network I/O.
    assert _contiguous_fetches(_loc_chunks(SampleShuffle())) == 0
    assert _covers_every_sample(_loc_chunks(SampleShuffle()))
    sample_index = _sample_index(_LOC_SEG_A, _LOC_SEG_B)
    order_small, _ = SampleShuffle().epoch_order(sample_index, fetch_size=2, seed=_LOC_SEED)
    order_large, _ = SampleShuffle().epoch_order(sample_index, fetch_size=999, seed=_LOC_SEED)
    assert np.array_equal(order_small, order_large)  # identical order regardless of fetch_size


def test_locality_ranking_none_equals_block_and_beats_sample() -> None:
    # The whole throughput story in one assertion: NoShuffle and BlockShuffle are fully local,
    # SampleShuffle is fully scattered. Any benchmark where `block` sits well below `none` is
    # therefore not measuring "block shuffle" at all.
    assert _contiguous_fetches(_loc_chunks(NoShuffle())) == 3
    assert _contiguous_fetches(_loc_chunks(BlockShuffle())) == 3
    assert _contiguous_fetches(_loc_chunks(SampleShuffle())) == 0


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
    assert _loc_chunks(strategy) == _loc_chunks(baseline)


def test_only_block_shuffle_defines_an_emission_buffer() -> None:
    assert NoShuffle().emission_buffer() is None
    assert SampleShuffle().emission_buffer() is None
    assert BlockShuffle().emission_buffer() is None  # block, but no buffer requested
    assert BlockShuffle(buffer_size=4).emission_buffer() is not None
