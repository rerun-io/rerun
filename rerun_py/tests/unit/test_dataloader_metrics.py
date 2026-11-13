"""Tests for always-on dataloader pipeline metrics."""

from __future__ import annotations

import pickle
import time
from concurrent.futures import ThreadPoolExecutor
from types import SimpleNamespace
from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa
import pytest
import torch
from rerun.experimental.dataloader import DataloaderMetrics, NumericDecoder, _utils
from rerun.experimental.dataloader._metrics import (
    _record_video_frames,
    _worker_metrics_scope,
    _WorkerMetricsPublisher,
)
from rerun.experimental.dataloader._utils import (
    FetchedBlock,
    FetchMetricsAccumulator,
    PreparedBlock,
    PreparedField,
)
from rerun.experimental.dataloader.decoders import DecodeRequest, FieldBatch

if TYPE_CHECKING:
    from collections.abc import Iterator


class _MeasuredIterable(torch.utils.data.IterableDataset[int]):
    """Publish deterministic measurements from each spawned worker."""

    def __init__(self, publisher: _WorkerMetricsPublisher) -> None:
        self._publisher = publisher

    def __iter__(self) -> Iterator[int]:
        worker = torch.utils.data.get_worker_info()
        assert worker is not None
        samples = worker.id + 1
        self._publisher.record_fetch(
            stall_ns=(worker.id + 1) * 20,
            samples_fetched=samples,
            direct_original_ranges=samples * 2,
            direct_merged_ranges=1,
            grpc_requests=0,
            delivered_bytes=(worker.id + 1) * 1000,
        )
        self._publisher.record_skipped_samples(samples)
        self._publisher.record_video_frames(decoded=(worker.id + 1) * 5, output=samples)
        self._publisher.record_decode(elapsed_ns=(worker.id + 1) * 80)
        self._publisher.flush()
        yield from range(samples)


def _publisher_for_worker(
    metrics: DataloaderMetrics,
    monkeypatch: pytest.MonkeyPatch,
    worker_id: int,
) -> _WorkerMetricsPublisher:
    monkeypatch.setattr(torch.utils.data, "get_worker_info", lambda: SimpleNamespace(id=worker_id))
    return cast("_WorkerMetricsPublisher", pickle.loads(pickle.dumps(metrics._worker_publisher())))


def test_metrics_reject_invalid_worker_limit() -> None:
    with pytest.raises(ValueError, match="max_workers"):
        DataloaderMetrics(max_workers=0)


def test_summary_derives_compact_pipeline_indicators() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()
        publisher.record_fetch(
            stall_ns=30,
            samples_fetched=10,
            direct_original_ranges=20,
            direct_merged_ranges=2,
            grpc_requests=3,
            delivered_bytes=1000,
        )
        publisher.record_skipped_samples(3)
        publisher.record_video_frames(decoded=55, output=10)
        publisher.record_decode(elapsed_ns=70)
        publisher.flush()

        assert metrics.summary() == pytest.approx({
            "fetch_wait_fraction": 0.3,
            "decode_fraction": 0.7,
            "fetch_requests_per_sample": 0.5,
            "fetch_request_merge_factor": 10.0,
            "decoded_frames_per_output_frame": 5.5,
            "skipped_samples": 3.0,
        })
        assert metrics.summary(prefix="dataloader/") == pytest.approx({
            "dataloader/fetch_wait_fraction": 0.3,
            "dataloader/decode_fraction": 0.7,
            "dataloader/fetch_requests_per_sample": 0.5,
            "dataloader/fetch_request_merge_factor": 10.0,
            "dataloader/decoded_frames_per_output_frame": 5.5,
            "dataloader/skipped_samples": 3.0,
        })
    finally:
        metrics.close()


def test_summary_counts_grpc_requests_without_direct_ranges() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()
        publisher.record_fetch(
            stall_ns=100,
            samples_fetched=2,
            direct_original_ranges=0,
            direct_merged_ranges=0,
            grpc_requests=4,
            delivered_bytes=100,
        )
        publisher.flush()

        report = metrics.summary()
        assert report["fetch_requests_per_sample"] == pytest.approx(2.0)
        assert "fetch_request_merge_factor" not in report
    finally:
        metrics.close()


def test_summary_aggregates_workers_and_restarted_generations(monkeypatch: pytest.MonkeyPatch) -> None:
    metrics = DataloaderMetrics(max_workers=2)
    try:
        first = _publisher_for_worker(metrics, monkeypatch, 0)
        first.record_fetch(
            stall_ns=20,
            samples_fetched=2,
            direct_original_ranges=4,
            direct_merged_ranges=1,
            grpc_requests=0,
            delivered_bytes=100,
        )
        first.close()

        restarted = _publisher_for_worker(metrics, monkeypatch, 0)
        restarted.record_fetch(
            stall_ns=40,
            samples_fetched=3,
            direct_original_ranges=6,
            direct_merged_ranges=1,
            grpc_requests=0,
            delivered_bytes=200,
        )
        restarted.close()

        second = _publisher_for_worker(metrics, monkeypatch, 1)
        second.record_fetch(
            stall_ns=60,
            samples_fetched=4,
            direct_original_ranges=8,
            direct_merged_ranges=2,
            grpc_requests=0,
            delivered_bytes=300,
        )
        second.close()

        report = metrics.summary()
        assert report["fetch_wait_fraction"] == pytest.approx(1.0)
        assert report["decode_fraction"] == pytest.approx(0.0)
        assert report["fetch_requests_per_sample"] == pytest.approx(4 / 9)
        assert report["fetch_request_merge_factor"] == pytest.approx(4.5)
        assert report["skipped_samples"] == pytest.approx(0.0)
    finally:
        metrics.close()


def test_parent_and_worker_zero_publish_to_distinct_slots(monkeypatch: pytest.MonkeyPatch) -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        parent = metrics._worker_publisher()
        parent.record_fetch(
            stall_ns=10,
            samples_fetched=1,
            direct_original_ranges=2,
            direct_merged_ranges=1,
            grpc_requests=0,
            delivered_bytes=100,
        )

        worker = _publisher_for_worker(metrics, monkeypatch, 0)
        worker.record_fetch(
            stall_ns=20,
            samples_fetched=2,
            direct_original_ranges=6,
            direct_merged_ranges=2,
            grpc_requests=0,
            delivered_bytes=200,
        )
        worker.close()

        parent.record_fetch(
            stall_ns=30,
            samples_fetched=3,
            direct_original_ranges=12,
            direct_merged_ranges=3,
            grpc_requests=0,
            delivered_bytes=300,
        )

        report = metrics.summary()
        assert report["fetch_requests_per_sample"] == pytest.approx(1.0)
        assert report["fetch_request_merge_factor"] == pytest.approx(20 / 6)
    finally:
        metrics.close()


def test_fetch_metrics_accumulator_records_query_metrics() -> None:
    queries = [
        SimpleNamespace(
            fetch_direct_original_ranges=5,
            fetch_direct_merged_ranges=1,
            fetch_grpc_requests=2,
            delivered_bytes=100,
        ),
        SimpleNamespace(
            fetch_direct_original_ranges=1,
            fetch_direct_merged_ranges=1,
            fetch_grpc_requests=1,
            delivered_bytes=200,
        ),
    ]
    metrics = FetchMetricsAccumulator()

    metrics.record_queries(cast("list[_utils.QueryMetrics]", queries))

    assert metrics.direct_original_ranges == 6
    assert metrics.direct_merged_ranges == 2
    assert metrics.grpc_requests == 3
    assert metrics.delivered_bytes == 300


def test_video_decoder_metrics_use_the_active_worker_publisher() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()
        with _worker_metrics_scope(publisher):
            _record_video_frames(decoded=11, output=2)
        publisher.flush()

        assert metrics.summary() == {"decoded_frames_per_output_frame": 5.5}
    finally:
        metrics.close()


def test_video_decoder_metrics_support_parallel_updates() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()
        updates = [(index % 5 + 1, 1) for index in range(1000)]
        with ThreadPoolExecutor(max_workers=8) as executor:
            futures = [
                executor.submit(publisher.record_video_frames, decoded=decoded, output=output)
                for decoded, output in updates
            ]
            for future in futures:
                future.result()
        publisher.flush()

        assert metrics.summary() == {
            "decoded_frames_per_output_frame": sum(decoded for decoded, _ in updates) / len(updates)
        }
    finally:
        metrics.close()


def test_disabled_pipeline_does_not_read_metrics_clock(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        _utils.time,
        "perf_counter_ns",
        lambda: (_ for _ in ()).throw(AssertionError("disabled metrics read the clock")),
    )
    blocks = [np.array([1]), np.array([2])]
    samples = _utils._pipeline_blocks(
        blocks,
        fetch=lambda block: int(block[0]),
        process=lambda value: iter([{"value": value}]),
    )

    assert list(samples) == [{"value": 1}, {"value": 2}]


def test_pipeline_records_completed_unconsumed_lookahead() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()

        def fetch(block: np.ndarray) -> FetchedBlock:
            value = int(block[0])
            target_count = value
            return FetchedBlock(
                targets=cast("list[_utils.Target]", [SimpleNamespace()] * target_count),
                fetched_groups=[],
                fetch_direct_original_ranges=2 if value == 1 else 10,
                fetch_direct_merged_ranges=1 if value == 1 else 5,
                delivered_bytes=value * 100,
            )

        def process(fetched: FetchedBlock) -> Iterator[_utils.DecodedSample]:
            if fetched.delivered_bytes == 100:
                deadline = time.monotonic() + 5
                while metrics.summary().get("fetch_requests_per_sample") != pytest.approx(2.0):
                    if time.monotonic() >= deadline:
                        raise AssertionError("lookahead fetch did not publish its completed metrics")
                    time.sleep(0.001)
            yield cast("_utils.DecodedSample", {"value": torch.tensor(fetched.delivered_bytes)})

        samples = _utils._pipeline_blocks(
            [np.array([1]), np.array([2])],
            fetch=fetch,
            process=process,
            metrics=publisher,
        )
        assert next(samples)["value"] == 100
        samples.close()

        assert metrics.summary()["fetch_requests_per_sample"] == pytest.approx(2.0)
    finally:
        metrics.close()


def test_pipeline_and_decode_record_block_metrics() -> None:
    metrics = DataloaderMetrics(max_workers=1)
    try:
        publisher = metrics._worker_publisher()
        blocks = [np.array([1]), np.array([2])]
        samples = _utils._pipeline_blocks(
            blocks,
            fetch=lambda block: FetchedBlock(
                targets=cast("list[_utils.Target]", [SimpleNamespace(), SimpleNamespace()]),
                fetched_groups=[],
                fetch_direct_original_ranges=4,
                fetch_direct_merged_ranges=1,
                delivered_bytes=int(block[0]) * 100,
            ),
            process=lambda _value: iter([{"value": torch.tensor(1)}]),
            metrics=publisher,
        )
        assert len(list(samples)) == 2

        requests = [
            DecodeRequest(
                sample_position=index,
                segment_id="segment",
                index_value=index,
                decode_row_indices=(index,),
                output_row_indices=(index,),
                starts_at_keyframe=False,
            )
            for index in range(2)
        ]
        column = pa.array([[1.0], [2.0]])
        prepared = PreparedBlock(
            num_samples=2,
            fields={
                "state": PreparedField(
                    batch=FieldBatch(column=column, is_windowed=False),
                    requests=requests,
                    num_segments=1,
                )
            },
        )
        decoded = _utils._decode_iter(
            prepared=prepared,
            decoders={"state": NumericDecoder()},
            metrics=publisher,
        )
        assert len(list(decoded)) == 2
        publisher.flush()

        report = metrics.summary()
        assert report["fetch_requests_per_sample"] == pytest.approx(0.5)
        assert report["fetch_request_merge_factor"] == pytest.approx(4.0)
        assert report["decode_fraction"] > 0
        assert report["skipped_samples"] == pytest.approx(0.0)
    finally:
        metrics.close()


def test_spawned_dataloader_workers_aggregate_into_dataset_metrics() -> None:
    metrics = DataloaderMetrics(max_workers=2)
    try:
        dataset = _MeasuredIterable(metrics._worker_publisher())
        loader = torch.utils.data.DataLoader(
            dataset,
            batch_size=None,
            num_workers=2,
            multiprocessing_context="spawn",
        )

        assert sorted(loader) == [0, 0, 1]
        assert metrics.summary() == pytest.approx({
            "fetch_wait_fraction": 0.2,
            "decode_fraction": 0.8,
            "fetch_requests_per_sample": 2 / 3,
            "fetch_request_merge_factor": 3.0,
            "decoded_frames_per_output_frame": 5.0,
            "skipped_samples": 3.0,
        })
    finally:
        metrics.close()
