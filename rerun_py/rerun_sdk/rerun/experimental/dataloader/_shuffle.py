"""Shuffle strategies and the post-decode shuffle buffer for the iterable dataset."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, TypeVar

import numpy as np

if TYPE_CHECKING:
    from collections.abc import Generator

    from ._sample_index import SampleIndex

T = TypeVar("T")


class ShuffleStrategy(ABC):
    """
    Determines the order in which an epoch's samples are fetched.

    See [the training guide](https://rerun.io/docs/howto/train/dataloader) for the trade-offs.
    """

    @abstractmethod
    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
        """
        Return `(indices, block_bounds)` for one epoch: every global sample index once, in emission order.

        `block_bounds` are cumulative end positions of blocks within `indices`;
        each block must be a contiguous, segment-local span of the global index space.
        """


@dataclass(frozen=True)
class SampleShuffle(ShuffleStrategy):
    """
    Uniform per-sample shuffle: maximal decorrelation, minimal fetch locality.

    Every fetch scatters across all segments; prefer
    [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] when fetch
    throughput is the bottleneck.
    """

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:  # noqa: ARG002
        return _sample_order(sample_index, seed=seed)


@dataclass(frozen=True)
class BlockShuffle(ShuffleStrategy):
    """
    Shuffle segment-local blocks of consecutive samples instead of individual samples.

    Every fetch then reads one contiguous span, so stored data is read about
    once per epoch instead of once per fetch. Samples within a block keep
    their natural order, so decoders can reuse their cache across consecutive
    samples; set `shuffle_buffer_size` on
    [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset]
    to decorrelate batches.

    Parameters
    ----------
    block_size
        Samples per block, at least 1. Defaults to the dataset's `fetch_size`.

    """

    block_size: int | None = None

    def __post_init__(self) -> None:
        if self.block_size is not None and self.block_size < 1:
            raise ValueError(f"block_size must be at least 1, got {self.block_size}")

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
        block_size = self.block_size if self.block_size is not None else fetch_size
        return _blockwise_order(sample_index, block_size=block_size, shuffle_blocks=True, seed=seed)


@dataclass(frozen=True)
class NoShuffle(ShuffleStrategy):
    """Natural order (segment by segment, along the timeline): maximal fetch locality, no randomness."""

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:  # noqa: ARG002
        return _blockwise_order(sample_index, block_size=fetch_size, shuffle_blocks=False)


class ShuffleBuffer:
    """
    Stream-shuffles an iterator through a fixed-size reservoir (the WebDataset algorithm).

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
        """Yield the items of `items`, shuffled through the reservoir; closes `items` when done."""
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


def _sample_order(sample_index: SampleIndex, *, seed: int) -> tuple[np.ndarray, np.ndarray]:
    """Return `(indices, block_bounds)` for a uniform per-sample permutation; every sample is its own block."""
    total = int(sample_index.segment_offsets[-1])
    rng = np.random.default_rng(seed=seed)
    indices = rng.permutation(total).astype(np.int64)
    return indices, np.arange(1, total + 1, dtype=np.int64)


def _blockwise_order(
    sample_index: SampleIndex,
    *,
    block_size: int,
    shuffle_blocks: bool,
    seed: int = 0,
) -> tuple[np.ndarray, np.ndarray]:
    """
    Return `(indices, block_bounds)` cutting the global index space into segment-local blocks.

    With `shuffle_blocks`, the block order is permuted; samples within a block
    always keep their natural order (this preserves decoder cache locality).
    """
    offsets = sample_index.segment_offsets
    total = int(offsets[-1])
    if total == 0:
        return np.empty(0, dtype=np.int64), np.empty(0, dtype=np.int64)

    # Global block id of every sample; blocks never cross a segment boundary.
    block_ids = np.empty(total, dtype=np.int64)
    num_blocks = 0
    for i in range(len(offsets) - 1):
        start = int(offsets[i])
        end = int(offsets[i + 1])
        block_ids[start:end] = num_blocks + np.arange(end - start, dtype=np.int64) // block_size
        num_blocks += (end - start + block_size - 1) // block_size

    if shuffle_blocks:
        rng = np.random.default_rng(seed=seed)
        emitted_blocks = rng.permutation(num_blocks)
    else:
        emitted_blocks = np.arange(num_blocks)

    # Stable-sort samples by their block's emission position; ties (samples
    # within a block) keep their natural order, so each block stays a
    # contiguous, in-order span.
    block_position = np.empty(num_blocks, dtype=np.int64)
    block_position[emitted_blocks] = np.arange(num_blocks)
    indices = np.argsort(block_position[block_ids], kind="stable").astype(np.int64)

    block_bounds = np.cumsum(np.bincount(block_ids, minlength=num_blocks)[emitted_blocks])
    return indices, block_bounds


def _contiguous_shard(
    indices: np.ndarray,
    block_bounds: np.ndarray,
    *,
    rank: int,
    world_size: int,
) -> tuple[np.ndarray, np.ndarray]:
    """
    Return the `rank`-th contiguous sample slice, with the last rank taking the remainder.

    Sample-granular cuts keep per-rank counts within `world_size - 1` of each
    other (uneven counts stall the DDP all-reduce); a block cut in two stays
    contiguous on both sides, so fetches remain chunk-local.
    """
    per_shard = len(indices) // world_size
    start = rank * per_shard
    end = start + per_shard if rank < world_size - 1 else len(indices)
    inner_bounds = block_bounds[(block_bounds > start) & (block_bounds < end)] - start
    return indices[start:end], np.append(inner_bounds, end - start)


def _fetch_chunks(indices: np.ndarray, block_bounds: np.ndarray, *, fetch_size: int) -> list[np.ndarray]:
    """
    Split `indices` into fetch-sized chunks that respect block boundaries.

    Whole blocks are packed greedily up to `fetch_size`; longer blocks are
    split at `fetch_size` strides, so every fetch reads few contiguous spans.
    """
    chunks: list[np.ndarray] = []
    chunk_start = 0
    packed_end = 0
    for bound in block_bounds:
        bound = int(bound)
        if bound - chunk_start <= fetch_size:
            packed_end = bound
            continue
        if packed_end > chunk_start:
            chunks.append(indices[chunk_start:packed_end])
            chunk_start = packed_end
        while bound - chunk_start > fetch_size:
            chunks.append(indices[chunk_start : chunk_start + fetch_size])
            chunk_start += fetch_size
        packed_end = bound
    if chunk_start < len(indices):
        chunks.append(indices[chunk_start:])
    return chunks
