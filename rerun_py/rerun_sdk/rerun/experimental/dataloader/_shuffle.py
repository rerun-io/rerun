"""Shuffle strategies and the post-decode shuffle buffer for the iterable dataset."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, ClassVar, TypeVar

import numpy as np

if TYPE_CHECKING:
    from collections.abc import Generator, Iterator

    from ._sample_index import SampleIndex

T = TypeVar("T")

_BLOCK_DESCRIPTOR_CHUNK_SIZE = 1 << 20


class _WorkOrder(ABC):
    """A lazily materialized epoch order that can be sharded without copying its backing arrays."""

    @property
    @abstractmethod
    def num_samples(self) -> int:
        """Number of samples in this order or shard."""

    @abstractmethod
    def shard(self, *, rank: int, world_size: int) -> _WorkOrder:
        """
        Return one contiguous sample-granular shard, with the final shard taking the remainder.

        Shard sizes differ by at most `world_size - 1`; block boundaries may be clipped at shard edges.
        """

    @abstractmethod
    def compact(self) -> _WorkOrder:
        """Release unneeded parent-shard data, copying retained data when necessary; may return `self`."""

    @abstractmethod
    def fetch_blocks(self, *, fetch_block_size: int) -> Iterator[np.ndarray]:
        """Materialize at most one fetch block at a time."""

    @abstractmethod
    def materialize(self) -> tuple[np.ndarray, np.ndarray]:
        """Materialize the order as `(indices, block_bounds)`."""


@dataclass(frozen=True)
class _NaturalWorkOrder(_WorkOrder):
    segment_offsets: np.ndarray
    block_size: int
    sample_start: int
    sample_end: int

    @property
    def num_samples(self) -> int:
        return self.sample_end - self.sample_start

    def shard(self, *, rank: int, world_size: int) -> _NaturalWorkOrder:
        start, end = _shard_interval(self.num_samples, rank=rank, world_size=world_size)
        return _NaturalWorkOrder(
            self.segment_offsets,
            self.block_size,
            self.sample_start + start,
            self.sample_start + end,
        )

    def compact(self) -> _NaturalWorkOrder:
        return self

    def _ranges(self) -> Iterator[tuple[int, int]]:
        if self.num_samples == 0:
            return

        position = self.sample_start
        segment = int(np.searchsorted(self.segment_offsets[1:], position, side="right"))
        while position < self.sample_end:
            while segment + 1 < len(self.segment_offsets) and position >= int(self.segment_offsets[segment + 1]):
                segment += 1

            segment_start = int(self.segment_offsets[segment])
            segment_end = min(int(self.segment_offsets[segment + 1]), self.sample_end)
            next_block_end = segment_start + ((position - segment_start) // self.block_size + 1) * self.block_size
            end = min(segment_end, next_block_end)
            yield position, end - position
            position = end

    def fetch_blocks(self, *, fetch_block_size: int) -> Iterator[np.ndarray]:
        yield from _pack_ranges(self._ranges(), fetch_block_size=fetch_block_size)

    def materialize(self) -> tuple[np.ndarray, np.ndarray]:
        indices = np.arange(self.sample_start, self.sample_end, dtype=np.int64)
        lengths = np.fromiter((length for _, length in self._ranges()), dtype=np.int64)
        return indices, np.cumsum(lengths)


@dataclass(frozen=True)
class _RangeWorkOrder(_WorkOrder):
    block_starts: np.ndarray
    block_bounds: np.ndarray
    sample_start: int
    sample_end: int

    @property
    def num_samples(self) -> int:
        return self.sample_end - self.sample_start

    def shard(self, *, rank: int, world_size: int) -> _RangeWorkOrder:
        start, end = _shard_interval(self.num_samples, rank=rank, world_size=world_size)
        return _RangeWorkOrder(
            self.block_starts,
            self.block_bounds,
            self.sample_start + start,
            self.sample_start + end,
        )

    def _block_span(self) -> tuple[int, int]:
        if self.num_samples == 0:
            return 0, 0
        start = int(np.searchsorted(self.block_bounds, self.sample_start, side="right"))
        end = int(np.searchsorted(self.block_bounds, self.sample_end - 1, side="right")) + 1
        return start, end

    def compact(self) -> _RangeWorkOrder:
        if self.num_samples == 0:
            empty = np.empty(0, dtype=np.int64)
            return _RangeWorkOrder(empty, empty, 0, 0)

        first, last = self._block_span()
        if (
            first == 0
            and last == len(self.block_starts)
            and self.sample_start == 0
            and self.sample_end == int(self.block_bounds[-1])
        ):
            return self

        starts = self.block_starts[first:last].copy()
        logical_start = int(self.block_bounds[first - 1]) if first > 0 else 0
        starts[0] += self.sample_start - logical_start
        bounds = self.block_bounds[first:last].copy()
        bounds -= self.sample_start
        bounds[-1] = self.num_samples
        return _RangeWorkOrder(starts, bounds, 0, self.num_samples)

    def _ranges(self) -> Iterator[tuple[int, int]]:
        first, last = self._block_span()
        for block in range(first, last):
            logical_end = int(self.block_bounds[block])
            logical_start = int(self.block_bounds[block - 1]) if block > 0 else 0
            clipped_start = max(logical_start, self.sample_start)
            clipped_end = min(logical_end, self.sample_end)
            source_start = int(self.block_starts[block]) + clipped_start - logical_start
            yield source_start, clipped_end - clipped_start

    def fetch_blocks(self, *, fetch_block_size: int) -> Iterator[np.ndarray]:
        yield from _pack_ranges(self._ranges(), fetch_block_size=fetch_block_size)

    def materialize(self) -> tuple[np.ndarray, np.ndarray]:
        indices = np.empty(self.num_samples, dtype=np.int64)
        first, last = self._block_span()
        bounds = np.empty(last - first, dtype=np.int64)
        position = 0
        for i, (start, length) in enumerate(self._ranges()):
            indices[position : position + length] = np.arange(start, start + length, dtype=np.int64)
            position += length
            bounds[i] = position
        return indices, bounds


@dataclass(frozen=True)
class _MaterializedWorkOrder(_WorkOrder):
    indices: np.ndarray
    block_bounds: np.ndarray
    sample_start: int
    sample_end: int

    @property
    def num_samples(self) -> int:
        return self.sample_end - self.sample_start

    def shard(self, *, rank: int, world_size: int) -> _MaterializedWorkOrder:
        start, end = _shard_interval(self.num_samples, rank=rank, world_size=world_size)
        return _MaterializedWorkOrder(
            self.indices,
            self.block_bounds,
            self.sample_start + start,
            self.sample_start + end,
        )

    def compact(self) -> _MaterializedWorkOrder:
        if self.sample_start == 0 and self.sample_end == len(self.indices):
            return self
        indices, block_bounds = self.materialize()
        indices = indices.copy()
        block_bounds = block_bounds.copy()
        return _MaterializedWorkOrder(indices, block_bounds, 0, len(indices))

    def fetch_blocks(self, *, fetch_block_size: int) -> Iterator[np.ndarray]:
        yield from _iter_materialized_fetch_blocks(
            self.indices,
            self.block_bounds,
            sample_start=self.sample_start,
            sample_end=self.sample_end,
            fetch_block_size=fetch_block_size,
        )

    def materialize(self) -> tuple[np.ndarray, np.ndarray]:
        if self.num_samples == 0:
            return np.empty(0, dtype=np.int64), np.empty(0, dtype=np.int64)
        first = int(np.searchsorted(self.block_bounds, self.sample_start, side="right"))
        last = int(np.searchsorted(self.block_bounds, self.sample_end, side="left"))
        inner = self.block_bounds[first:last] - self.sample_start
        return self.indices[self.sample_start : self.sample_end], np.append(inner, self.num_samples)


class ShuffleStrategy(ABC):
    """
    Determines the order in which an epoch's samples are fetched.

    See [the training guide](https://rerun.io/docs/train/dataloader) for the trade-offs.
    """

    RECIPE_TAG: ClassVar[str]

    @abstractmethod
    def epoch_order(
        self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int
    ) -> tuple[np.ndarray, np.ndarray]:
        """
        Return `(indices, block_bounds)` for one epoch: every global sample index once, in emission order.

        `block_bounds` are cumulative end positions of blocks within `indices`;
        each block must be a contiguous, segment-local span of the global index space.
        """

    def _work_order(self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int) -> _WorkOrder:
        """Return a shardable order whose fetch blocks are materialized lazily."""
        indices, block_bounds = self.epoch_order(
            sample_index,
            fetch_block_size=fetch_block_size,
            seed=seed,
        )
        return _MaterializedWorkOrder(indices, block_bounds, 0, len(indices))

    def emission_buffer(self) -> ShuffleBuffer | None:
        """
        The post-decode buffer samples are emitted through, or `None` to emit in fetch order.

        Only [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] defines
        one, because it is the only strategy whose fetch order stays deliberately
        correlated: emission is where its batches get decorrelated.
        [`SampleShuffle`][rerun.experimental.dataloader.SampleShuffle] already
        fetches a uniform permutation, so a buffer would add nothing, and
        [`NoShuffle`][rerun.experimental.dataloader.NoShuffle] is a deterministic
        baseline that a buffer would only contaminate.

        """
        return None


@dataclass(frozen=True)
class SampleShuffle(ShuffleStrategy):
    """
    Uniform per-sample shuffle: maximal decorrelation, minimal fetch locality.

    Every fetch scatters across all segments; prefer
    [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] when fetch
    throughput is the bottleneck.
    """

    RECIPE_TAG: ClassVar[str] = "sample"

    def epoch_order(
        self,
        sample_index: SampleIndex,
        *,
        fetch_block_size: int,
        seed: int,
    ) -> tuple[np.ndarray, np.ndarray]:
        # A per-sample shuffle has no blocks: every sample is emitted independently, so the order is
        # a plain uniform permutation and each "block" is a single sample. It depends on neither a
        # block size (there is none) nor `fetch_block_size` (fetching only batches the network I/O).
        del fetch_block_size
        total = int(sample_index.segment_offsets[-1])
        rng = np.random.default_rng(seed=seed)
        indices = rng.permutation(total).astype(np.int64)
        block_bounds = np.arange(1, total + 1, dtype=np.int64)
        return indices, block_bounds


@dataclass(frozen=True)
class BlockShuffle(ShuffleStrategy):
    """
    Shuffle fetch-sized blocks of consecutive samples instead of individual samples.

    Each block is one fetch's worth of consecutive, segment-local samples. The block *order* is
    shuffled, but samples keep their natural order *within* a block, so every fetch still reads one
    contiguous span: stored data is read about once per epoch instead of once per fetch, and decoders
    reuse their cache across a block. Set `buffer_size` to decorrelate batches at emission time.

    Parameters
    ----------
    buffer_size
        Size of the post-decode buffer that randomizes emission order
        without changing the fetch order. `None` (the default) emits in fetch
        order. This is the only strategy that takes one — see
        [`emission_buffer`][rerun.experimental.dataloader.ShuffleStrategy.emission_buffer].

        !!! warning
            The buffer holds up to `buffer_size` **decoded** samples per
            `DataLoader` worker, as your decoders produce them — full-resolution
            frames for video fields. Budget
            `buffer_size * bytes_per_sample * num_workers`; a few thousand video
            samples per worker is tens of gigabytes.
    min_fill
        Samples buffered before emission starts. Defaults to `buffer_size // 2`,
        which is also how long the first sample is delayed; lower it to shorten
        that warm-up at the cost of less mixing over the first few batches.

    """

    RECIPE_TAG: ClassVar[str] = "block"

    buffer_size: int | None = None
    min_fill: int | None = None

    def __post_init__(self) -> None:
        if self.buffer_size is None and self.min_fill is not None:
            raise ValueError("min_fill requires buffer_size to be set")
        if self.buffer_size is not None:
            self.emission_buffer()  # validates `buffer_size` / `min_fill` eagerly

    def epoch_order(
        self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int
    ) -> tuple[np.ndarray, np.ndarray]:
        # A block is one fetch wide.
        return _block_order(sample_index, block_size=fetch_block_size, shuffle=True, seed=seed)

    def _work_order(self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int) -> _WorkOrder:
        return _block_work_order(sample_index, block_size=fetch_block_size, seed=seed)

    def emission_buffer(self) -> ShuffleBuffer | None:
        if self.buffer_size is None:
            return None
        return ShuffleBuffer(self.buffer_size, min_fill=self.min_fill)


@dataclass(frozen=True)
class NoShuffle(ShuffleStrategy):
    """Natural order (segment by segment, along the timeline): maximal fetch locality, no randomness."""

    RECIPE_TAG: ClassVar[str] = "none"

    def epoch_order(
        self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int
    ) -> tuple[np.ndarray, np.ndarray]:
        # A block is one fetch wide; without a shuffle the block size only affects `block_bounds`.
        return _block_order(sample_index, block_size=fetch_block_size, shuffle=False, seed=seed)

    def _work_order(self, sample_index: SampleIndex, *, fetch_block_size: int, seed: int) -> _WorkOrder:
        del seed
        offsets = sample_index.segment_offsets
        return _NaturalWorkOrder(offsets, fetch_block_size, 0, int(offsets[-1]))


class ShuffleBuffer:
    """
    Stream-shuffles an iterator through a fixed-size buffer (the WebDataset algorithm).

    Each emitted item is a uniformly random member of the buffer; the input
    itself is still consumed in its original order.

    Parameters
    ----------
    buffer_size
        Maximum number of items held; must be at least 2.
    min_fill
        Buffered items required before emission starts.
        Defaults to `buffer_size // 2`.

    """

    def __init__(self, buffer_size: int, *, min_fill: int | None = None) -> None:
        if buffer_size < 2:
            raise ValueError(f"buffer_size must be at least 2, got {buffer_size}")
        if min_fill is not None and not 1 <= min_fill <= buffer_size:
            raise ValueError(f"min_fill must be in [1, buffer_size], got {min_fill}")
        self.buffer_size = buffer_size
        self.min_fill = min_fill if min_fill is not None else buffer_size // 2

    def shuffle(self, items: Generator[T, None, None], *, rng: np.random.Generator) -> Generator[T, None, None]:
        """Yield the items of `items`, shuffled through the buffer; closes `items` when done."""
        buffer: list[T] = []
        try:
            for item in items:
                buffer.append(item)
                if len(buffer) < self.buffer_size:
                    # Take a second item per emission so the buffer keeps
                    # growing toward capacity after emission has started.
                    try:
                        buffer.append(next(items))
                    except StopIteration:
                        pass
                if len(buffer) >= self.min_fill:
                    yield _pick(buffer, rng)
            while buffer:
                yield _pick(buffer, rng)
        finally:
            items.close()


def _pick(buffer: list[T], rng: np.random.Generator) -> T:
    """Remove and return a uniformly random element, O(1) via swap-with-last."""
    j = int(rng.integers(len(buffer)))
    buffer[j], buffer[-1] = buffer[-1], buffer[j]
    return buffer.pop()


def _shard_interval(length: int, *, rank: int, world_size: int) -> tuple[int, int]:
    """Return the rank's contiguous interval, assigning all remainder samples to the final rank."""
    per_shard = length // world_size
    start = rank * per_shard
    end = start + per_shard if rank < world_size - 1 else length
    return start, end


def _materialize_ranges(ranges: list[tuple[int, int]], total: int) -> np.ndarray:
    if len(ranges) == 1:
        start, length = ranges[0]
        return np.arange(start, start + length, dtype=np.int64)

    indices = np.empty(total, dtype=np.int64)
    position = 0
    for start, length in ranges:
        indices[position : position + length] = np.arange(start, start + length, dtype=np.int64)
        position += length
    return indices


def _pack_ranges(ranges: Iterator[tuple[int, int]], *, fetch_block_size: int) -> Iterator[np.ndarray]:
    packed: list[tuple[int, int]] = []
    packed_size = 0
    for start, length in ranges:
        while length > 0:
            if packed and packed_size + length > fetch_block_size:
                yield _materialize_ranges(packed, packed_size)
                packed = []
                packed_size = 0

            if length > fetch_block_size:
                yield np.arange(start, start + fetch_block_size, dtype=np.int64)
                start += fetch_block_size
                length -= fetch_block_size
                continue

            packed.append((start, length))
            packed_size += length
            length = 0
            if packed_size == fetch_block_size:
                yield _materialize_ranges(packed, packed_size)
                packed = []
                packed_size = 0

    if packed:
        yield _materialize_ranges(packed, packed_size)


def _block_work_order(sample_index: SampleIndex, *, block_size: int, seed: int) -> _RangeWorkOrder:
    offsets = sample_index.segment_offsets
    segment_sizes = np.diff(offsets)
    blocks_per_segment = (segment_sizes + block_size - 1) // block_size
    segment_block_offsets = np.concatenate([[0], np.cumsum(blocks_per_segment)])
    num_blocks = int(segment_block_offsets[-1])
    total = int(offsets[-1])
    if num_blocks == 0:
        empty = np.empty(0, dtype=np.int64)
        return _RangeWorkOrder(empty, empty, 0, 0)

    emitted_blocks = np.random.default_rng(seed=seed).permutation(num_blocks)
    ordered_starts = np.empty(num_blocks, dtype=np.int64)
    ordered_lengths = np.empty(num_blocks, dtype=np.int64)
    for position in range(0, num_blocks, _BLOCK_DESCRIPTOR_CHUNK_SIZE):
        end = min(position + _BLOCK_DESCRIPTOR_CHUNK_SIZE, num_blocks)
        block_ids = emitted_blocks[position:end]
        segments = np.searchsorted(segment_block_offsets[1:], block_ids, side="right")
        local_blocks = block_ids - segment_block_offsets[segments]
        starts = offsets[segments] + local_blocks * block_size
        ordered_starts[position:end] = starts
        ordered_lengths[position:end] = np.minimum(block_size, offsets[segments + 1] - starts)

    del emitted_blocks
    np.cumsum(ordered_lengths, out=ordered_lengths)
    return _RangeWorkOrder(ordered_starts, ordered_lengths, 0, total)


def _block_order(
    sample_index: SampleIndex,
    *,
    block_size: int,
    shuffle: bool,
    seed: int,
) -> tuple[np.ndarray, np.ndarray]:
    """
    Return `(indices, block_bounds)` cutting the global index space into segment-local blocks of `block_size`.

    With `shuffle`, the block order is permuted; samples within a block keep their natural order
    (this preserves decoder cache locality). This is a generic block-cutting primitive; the
    strategies always pass `fetch_block_size` as the block size, so in practice each block is one fetch wide.
    """
    if shuffle:
        return _block_work_order(sample_index, block_size=block_size, seed=seed).materialize()

    offsets = sample_index.segment_offsets
    return _NaturalWorkOrder(offsets, block_size, 0, int(offsets[-1])).materialize()


def _iter_materialized_fetch_blocks(
    indices: np.ndarray,
    block_bounds: np.ndarray,
    *,
    sample_start: int,
    sample_end: int,
    fetch_block_size: int,
) -> Iterator[np.ndarray]:
    if sample_start == sample_end:
        return

    fetch_start = sample_start
    packed_end = sample_start
    first = int(np.searchsorted(block_bounds, sample_start, side="right"))
    for raw_bound in block_bounds[first:]:
        bound = min(int(raw_bound), sample_end)
        if bound - fetch_start <= fetch_block_size:
            packed_end = bound
        else:
            if packed_end > fetch_start:
                yield indices[fetch_start:packed_end]
                fetch_start = packed_end
            while bound - fetch_start > fetch_block_size:
                yield indices[fetch_start : fetch_start + fetch_block_size]
                fetch_start += fetch_block_size
            packed_end = bound
        if bound == sample_end:
            break

    if fetch_start < sample_end:
        yield indices[fetch_start:sample_end]
