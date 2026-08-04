"""Shuffle strategies and the post-decode shuffle buffer for the iterable dataset."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, ClassVar, TypeVar

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

    # Stable identifier recorded as the manifest header's `shuffle_strategy` (provenance).
    # Decoupled from the class name on purpose, so renaming a class never changes the header.
    RECIPE_TAG: ClassVar[str]

    @abstractmethod
    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
        """
        Return `(indices, block_bounds)` for one epoch: every global sample index once, in emission order.

        `block_bounds` are cumulative end positions of blocks within `indices`;
        each block must be a contiguous, segment-local span of the global index space.
        """

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

        Owning the buffer here is what keeps a live run and a manifest replay
        in step: both read it off the same strategy object, so they cannot be
        configured with different buffers by accident.
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

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:  # noqa: ARG002
        # A per-sample shuffle has no blocks: every sample is emitted independently, so the order is
        # a plain uniform permutation and each "block" is a single sample. It depends on neither a
        # block size (there is none) nor `fetch_size` (fetching only batches the network I/O).
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

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
        # A block is one fetch wide: this is the sole place that policy is set.
        return _block_order(sample_index, block_size=fetch_size, shuffle=True, seed=seed)

    def emission_buffer(self) -> ShuffleBuffer | None:
        if self.buffer_size is None:
            return None
        return ShuffleBuffer(self.buffer_size, min_fill=self.min_fill)


@dataclass(frozen=True)
class NoShuffle(ShuffleStrategy):
    """Natural order (segment by segment, along the timeline): maximal fetch locality, no randomness."""

    RECIPE_TAG: ClassVar[str] = "none"

    def epoch_order(self, sample_index: SampleIndex, *, fetch_size: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
        # A block is one fetch wide; without a shuffle the block size only affects `block_bounds`.
        return _block_order(sample_index, block_size=fetch_size, shuffle=False, seed=seed)


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

    def emit_order(self, n: int, *, rng: np.random.Generator) -> np.ndarray:
        """
        Return the emission permutation for `n` items: `emit_order[k]` is the input position emitted `k`-th.

        Rolls the very same buffer [`shuffle`][rerun.experimental.dataloader.ShuffleBuffer.shuffle]
        uses, but over positions `[0, n)` instead of decoded samples. A baked
        manifest and the live buffer therefore share one implementation and can
        never diverge. Values are `int64`.
        """

        def _positions() -> Generator[int, None, None]:
            yield from range(n)

        return np.fromiter(self.shuffle(_positions(), rng=rng), dtype=np.int64, count=n)


def _pick(buffer: list[T], rng: np.random.Generator) -> T:
    """Remove and return a uniformly random element, O(1) via swap-with-last."""
    j = int(rng.integers(len(buffer)))
    buffer[j], buffer[-1] = buffer[-1], buffer[j]
    return buffer.pop()


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
    strategies always pass `fetch_size` as the block size, so in practice each block is one fetch wide.
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

    if shuffle:
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
