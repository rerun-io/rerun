"""Low-overhead metrics for normal dataloader runs."""

from __future__ import annotations

import os
import threading
import weakref
from contextlib import contextmanager
from contextvars import ContextVar
from multiprocessing import shared_memory
from typing import TYPE_CHECKING

import torch

if TYPE_CHECKING:
    from collections.abc import Iterator

# PyTorch worker IDs are dense from zero, so 256 worker slots cover unusually large loaders while keeping the table small.
_DEFAULT_MAX_WORKERS = 256

# Publishers write unsigned 64-bit values under a sequence lock so the parent can
# read coherent per-process values without synchronizing with the dataloader hot path.
_HEADER_WORDS = 3  # sequence, pid, slot ID
_COUNTER_NAMES = (
    "samples_fetched",
    "samples_skipped",
    "fetch_direct_original_ranges",
    "fetch_direct_merged_ranges",
    "fetch_grpc_requests",
    "delivered_bytes",
    "fetch_stall_ns",
    "decode_elapsed_ns",
    "video_frames_decoded",
    "video_output_frames",
)
_NUM_COUNTERS = len(_COUNTER_NAMES)
_SLOT_WORDS = _HEADER_WORDS + _NUM_COUNTERS
_SLOT_BYTES = _SLOT_WORDS * 8

_active_worker_metrics: ContextVar[_WorkerMetricsPublisher | None] = ContextVar(
    "rerun_dataloader_worker_metrics", default=None
)


def _ratio(numerator: int, denominator: int) -> float | None:
    return numerator / denominator if denominator > 0 else None


def _summary_from_totals(totals: dict[str, int], prefix: str) -> dict[str, float]:
    measured_path_ns = totals["fetch_stall_ns"] + totals["decode_elapsed_ns"]
    direct_requests = totals["fetch_direct_merged_ranges"]
    transport_requests = direct_requests + totals["fetch_grpc_requests"]
    values = {
        "fetch_wait_fraction": _ratio(totals["fetch_stall_ns"], measured_path_ns),
        "decode_fraction": _ratio(totals["decode_elapsed_ns"], measured_path_ns),
        "fetch_requests_per_sample": (
            _ratio(transport_requests, totals["samples_fetched"]) if transport_requests > 0 else None
        ),
        "fetch_request_merge_factor": _ratio(totals["fetch_direct_original_ranges"], direct_requests),
        "decoded_frames_per_output_frame": _ratio(totals["video_frames_decoded"], totals["video_output_frames"]),
        "skipped_samples": (
            float(totals["samples_skipped"]) if totals["samples_fetched"] > 0 or totals["samples_skipped"] > 0 else None
        ),
    }
    return {f"{prefix}{name}": value for name, value in values.items() if value is not None}


class _WorkerAccumulator:
    __slots__ = _COUNTER_NAMES

    samples_fetched: int
    samples_skipped: int
    fetch_direct_original_ranges: int
    fetch_direct_merged_ranges: int
    fetch_grpc_requests: int
    delivered_bytes: int
    fetch_stall_ns: int
    decode_elapsed_ns: int
    video_frames_decoded: int
    video_output_frames: int

    def __init__(self) -> None:
        for name in _COUNTER_NAMES:
            setattr(self, name, 0)

    def restore(self, values: list[int]) -> None:
        for name, value in zip(_COUNTER_NAMES, values, strict=True):
            setattr(self, name, value)


class _WorkerMetricsPublisher:
    """Picklable worker-side writer for one shared-memory metrics slot."""

    __slots__ = (
        "_accumulator",
        "_buffer",
        "_dirty",
        "_lock",
        "_max_workers",
        "_shm",
        "_shm_name",
        "_slot",
    )

    def __init__(self, shm_name: str, max_workers: int) -> None:
        self._shm_name = shm_name
        self._max_workers = max_workers
        self._shm: shared_memory.SharedMemory | None = None
        self._buffer: memoryview | None = None
        self._slot: int | None = None
        self._accumulator: _WorkerAccumulator | None = None
        self._dirty = False
        # Hot-path updates happen at fetch/decode run boundaries; skips are exceptional,
        # so lock cost is not paid for each normal sample or decoded frame.
        self._lock = threading.Lock()

    def __getstate__(self) -> tuple[str, int]:
        return self._shm_name, self._max_workers

    def __setstate__(self, state: tuple[str, int]) -> None:
        self._shm_name, self._max_workers = state
        self._shm = None
        self._buffer = None
        self._slot = None
        self._accumulator = None
        self._dirty = False
        self._lock = threading.Lock()

    def _ensure_attached(self) -> bool:
        if self._buffer is not None:
            return True
        try:
            worker = torch.utils.data.get_worker_info()
            if worker is None:
                slot = self._max_workers
            elif 0 <= worker.id < self._max_workers:
                slot = worker.id
            else:
                return False
            shm = shared_memory.SharedMemory(name=self._shm_name)
        except (FileNotFoundError, OSError):
            return False

        self._shm = shm
        shm_buffer = shm.buf
        assert shm_buffer is not None
        self._buffer = shm_buffer.cast("Q")
        self._slot = slot
        self._accumulator = _WorkerAccumulator()

        start = slot * _SLOT_WORDS
        sequence_before = int(self._buffer[start])
        if sequence_before % 2 == 0:
            previous = [int(self._buffer[start + index]) for index in range(1, _SLOT_WORDS)]
            sequence_after = int(self._buffer[start])
            if previous[0] != 0 and sequence_before == sequence_after and sequence_after % 2 == 0:
                self._accumulator.restore(previous[2:])
        return True

    def _record_fetch_completion_unlocked(
        self,
        *,
        samples_fetched: int,
        direct_original_ranges: int,
        direct_merged_ranges: int,
        grpc_requests: int,
        delivered_bytes: int,
    ) -> None:
        assert self._accumulator is not None
        self._accumulator.samples_fetched += max(0, samples_fetched)
        self._accumulator.fetch_direct_original_ranges += max(0, direct_original_ranges)
        self._accumulator.fetch_direct_merged_ranges += max(0, direct_merged_ranges)
        self._accumulator.fetch_grpc_requests += max(0, grpc_requests)
        self._accumulator.delivered_bytes += max(0, delivered_bytes)
        self._dirty = True

    def record_fetch(
        self,
        *,
        stall_ns: int,
        samples_fetched: int,
        direct_original_ranges: int,
        direct_merged_ranges: int,
        grpc_requests: int,
        delivered_bytes: int,
    ) -> None:
        """Record one completed synchronous fetch block and its consumer wait."""
        with self._lock:
            if not self._ensure_attached():
                return
            self._record_fetch_completion_unlocked(
                samples_fetched=samples_fetched,
                direct_original_ranges=direct_original_ranges,
                direct_merged_ranges=direct_merged_ranges,
                grpc_requests=grpc_requests,
                delivered_bytes=delivered_bytes,
            )
            assert self._accumulator is not None
            self._accumulator.fetch_stall_ns += max(0, stall_ns)
            self._publish()

    def record_fetch_completion(
        self,
        *,
        samples_fetched: int,
        direct_original_ranges: int,
        direct_merged_ranges: int,
        grpc_requests: int,
        delivered_bytes: int,
    ) -> None:
        """Record physical work as soon as an asynchronous fetch block completes."""
        with self._lock:
            if not self._ensure_attached():
                return
            self._record_fetch_completion_unlocked(
                samples_fetched=samples_fetched,
                direct_original_ranges=direct_original_ranges,
                direct_merged_ranges=direct_merged_ranges,
                grpc_requests=grpc_requests,
                delivered_bytes=delivered_bytes,
            )
            self._publish()

    def record_fetch_wait(self, stall_ns: int) -> None:
        """Record consumer time waiting for an asynchronous fetch result."""
        with self._lock:
            if not self._ensure_attached():
                return
            assert self._accumulator is not None
            self._accumulator.fetch_stall_ns += max(0, stall_ns)
            self._dirty = True
            self._publish()

    def record_decode(self, *, elapsed_ns: int) -> None:
        """Record one completed decode block."""
        with self._lock:
            if not self._ensure_attached():
                return
            assert self._accumulator is not None
            self._accumulator.decode_elapsed_ns += max(0, elapsed_ns)
            self._dirty = True
            self._publish()

    def record_skipped_samples(self, count: int = 1) -> None:
        """Record live samples dropped because at least one field was unresolved."""
        with self._lock:
            if not self._ensure_attached():
                return
            assert self._accumulator is not None
            self._accumulator.samples_skipped += max(0, count)
            self._dirty = True
            self._publish()

    def record_video_frames(self, *, decoded: int = 0, output: int = 0) -> None:
        """Record actual codec output and distinct requested output frames."""
        with self._lock:
            if not self._ensure_attached():
                return
            assert self._accumulator is not None
            self._accumulator.video_frames_decoded += max(0, decoded)
            self._accumulator.video_output_frames += max(0, output)
            self._dirty = True

    def flush(self) -> None:
        """Publish counters accumulated since the last completed fetch or decode block."""
        with self._lock:
            if self._dirty and self._buffer is not None:
                self._publish()

    def _publish(self) -> None:
        assert self._buffer is not None and self._slot is not None and self._accumulator is not None
        start = self._slot * _SLOT_WORDS
        sequence = int(self._buffer[start])
        odd_sequence = sequence + 1 if sequence % 2 == 0 else sequence + 2
        self._buffer[start] = odd_sequence

        values = [
            os.getpid(),
            self._slot,
            *(getattr(self._accumulator, name) for name in _COUNTER_NAMES),
        ]
        for offset, value in enumerate(values, start=1):
            self._buffer[start + offset] = value
        self._buffer[start] = odd_sequence + 1
        self._dirty = False

    def close(self) -> None:
        """Close this process's attachment without unlinking parent-owned storage."""
        with self._lock:
            if self._dirty and self._buffer is not None:
                self._publish()
            if self._buffer is not None:
                self._buffer.release()
                self._buffer = None
            if self._shm is not None:
                self._shm.close()
                self._shm = None

    def __del__(self) -> None:
        self.close()


def _cleanup_metrics_resources(shm: shared_memory.SharedMemory) -> None:
    try:
        shm.close()
    finally:
        try:
            shm.unlink()
        except FileNotFoundError:
            pass


class DataloaderMetrics:
    """
    Live cumulative metrics for a Rerun dataset.

    Access this object through `dataset.metrics`. Calling
    [`summary`][rerun.experimental.dataloader.DataloaderMetrics.summary] returns
    a flat dictionary of available derived metrics, suitable for experiment
    trackers. The values are a best-effort aggregate of completed worker
    measurements, not an atomic boundary with concurrently running workers.
    """

    def __init__(self, *, max_workers: int = _DEFAULT_MAX_WORKERS) -> None:
        if max_workers < 1:
            raise ValueError(f"max_workers must be at least 1, got {max_workers}")
        self._shm = shared_memory.SharedMemory(create=True, size=(max_workers + 1) * _SLOT_BYTES)
        shm_buffer = self._shm.buf
        assert shm_buffer is not None
        shm_buffer[:] = b"\0" * len(shm_buffer)
        self._publisher = _WorkerMetricsPublisher(self._shm.name, max_workers)
        self._max_workers = max_workers
        self._finalizer = weakref.finalize(self, _cleanup_metrics_resources, self._shm)

    def _worker_publisher(self) -> _WorkerMetricsPublisher:
        return self._publisher

    def summary(self, *, prefix: str = "") -> dict[str, float]:
        """
        Return available cumulative metrics as a flat dictionary.

        `prefix` is prepended to every key, for example `"dataloader/"` for
        Weights & Biases or `"dataloader."` for MLflow.
        """
        totals = dict.fromkeys(_COUNTER_NAMES, 0)
        shm_buffer = self._shm.buf
        assert shm_buffer is not None
        buffer = shm_buffer.cast("Q")
        try:
            for slot in range(self._max_workers + 1):
                start = slot * _SLOT_WORDS
                values: list[int] | None = None
                for _ in range(3):
                    sequence_before = int(buffer[start])
                    if sequence_before == 0:
                        break
                    if sequence_before % 2:
                        continue
                    candidate = [int(buffer[start + index]) for index in range(1, _SLOT_WORDS)]
                    sequence_after = int(buffer[start])
                    if sequence_before == sequence_after and sequence_after % 2 == 0:
                        values = candidate
                        break
                if values is None or values[0] == 0:
                    continue
                for name, value in zip(_COUNTER_NAMES, values[2:], strict=True):
                    totals[name] += value
        finally:
            buffer.release()

        return _summary_from_totals(totals, prefix)

    def close(self) -> None:
        """Release the shared-memory metrics store."""
        if self._finalizer.alive:
            self._publisher.close()
            self._finalizer()


@contextmanager
def _worker_metrics_scope(metrics: _WorkerMetricsPublisher | None) -> Iterator[None]:
    """Make a dataset's worker publisher available to decoder internals on this thread."""
    token = _active_worker_metrics.set(metrics)
    try:
        yield
    finally:
        _active_worker_metrics.reset(token)


def _record_video_frames(*, decoded: int = 0, output: int = 0) -> None:
    """Record video decoder work on the active dataset worker, if any."""
    metrics = _active_worker_metrics.get()
    if metrics is not None:
        metrics.record_video_frames(decoded=decoded, output=output)
