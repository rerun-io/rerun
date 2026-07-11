#!/usr/bin/env python3
"""Measure REDAP scrub/seek queries against a dataset.

This is an opt-in harness for issue #11315 style validation. It connects to a
REDAP server, optionally registers a remote RRD prefix, samples seek points from
the dataset's index-range metadata, then runs repeated narrow index queries
under `rerun.experimental.query_metrics()`.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import time
from typing import Any

from datafusion import col, lit

from rerun.catalog import CatalogClient
from rerun.experimental import QueryMetrics, query_metrics


@dataclasses.dataclass
class SeekPoint:
    segment_id: str
    value: Any


def _json_value(value: Any) -> Any:
    if hasattr(value, "as_py"):
        value = value.as_py()
    if hasattr(value, "isoformat"):
        return value.isoformat()
    if hasattr(value, "total_seconds"):
        return value.total_seconds()
    return value


def _query_metrics_dict(metrics: QueryMetrics) -> dict[str, Any]:
    return {
        "query_chunks": metrics.query_chunks,
        "query_segments": metrics.query_segments,
        "query_bytes": metrics.query_bytes,
        "filters_pushed_down": metrics.filters_pushed_down,
        "filters_applied_client_side": metrics.filters_applied_client_side,
        "fetch_grpc_requests": metrics.fetch_grpc_requests,
        "fetch_grpc_bytes": metrics.fetch_grpc_bytes,
        "fetch_direct_requests": metrics.fetch_direct_requests,
        "fetch_direct_bytes": metrics.fetch_direct_bytes,
        "fetch_direct_retries": metrics.fetch_direct_retries,
        "fetch_direct_original_ranges": metrics.fetch_direct_original_ranges,
        "fetch_direct_merged_ranges": metrics.fetch_direct_merged_ranges,
        "fetch_requests": metrics.fetch_requests,
        "fetch_bytes": metrics.fetch_bytes,
        "time_to_first_chunk_ms": (
            metrics.time_to_first_chunk.total_seconds() * 1000 if metrics.time_to_first_chunk is not None else None
        ),
        "total_duration_ms": metrics.total_duration.total_seconds() * 1000,
        "error_kind": metrics.error_kind,
        "direct_terminal_reason": metrics.direct_terminal_reason,
    }


def _dataset(args: argparse.Namespace):
    client = CatalogClient(url=args.redap_url, token=args.redap_token)

    if args.hf_repo is not None:
        dataset = client.create_dataset(args.dataset, exist_ok=True)
        handle = dataset.register_huggingface(
            args.hf_repo,
            revision=args.hf_revision,
            path=args.hf_path,
            layer_name=args.layer_name,
            token=args.hf_token,
            limit=args.hf_limit,
        )
        handle.wait(timeout_secs=args.registration_timeout_secs)
        return dataset

    if args.register_prefix is not None:
        dataset = client.create_dataset(args.dataset, exist_ok=True)
        handle = dataset.register_prefix(args.register_prefix, layer_name=args.layer_name)
        handle.wait(timeout_secs=args.registration_timeout_secs)
        return dataset

    return client.get_dataset(args.dataset)


def _sample_seek_points(dataset: Any, index: str, count: int) -> list[SeekPoint]:
    start_col = f"{index}:start"
    batches = (
        dataset
        .get_index_ranges()
        .select("rerun_segment_id", start_col)
        .sort("rerun_segment_id")
        .limit(max(count * 4, count))
        .collect()
    )

    points: list[SeekPoint] = []
    seen: set[tuple[str, str]] = set()
    for batch in batches:
        segment_ids = batch.column(0)
        starts = batch.column(1)
        for row_idx in range(batch.num_rows):
            start = starts[row_idx]
            if not start.is_valid:
                continue
            segment_id = segment_ids[row_idx].as_py()
            key = (segment_id, repr(start))
            if key in seen:
                continue
            seen.add(key)
            points.append(SeekPoint(segment_id=segment_id, value=start))
            if len(points) >= count:
                return points

    return points


def _run_seek(dataset: Any, index: str, point: SeekPoint) -> dict[str, Any]:
    start = time.perf_counter()
    with query_metrics() as collector:
        rows = sum(
            batch.num_rows
            for batch in (
                dataset
                .filter_segments(point.segment_id)
                .reader(index=index)
                .filter(col(index) == lit(point.value))
                .collect()
            )
        )
    elapsed_ms = (time.perf_counter() - start) * 1000

    metrics = collector.last_query()
    if metrics is None:
        raise RuntimeError(f"seek for segment {point.segment_id!r} at {point.value!r} produced no query metrics")

    return {
        "segment_id": point.segment_id,
        "seek_value": _json_value(point.value),
        "rows": rows,
        "wall_time_ms": elapsed_ms,
        **_query_metrics_dict(metrics),
    }


def _run_full_scan(dataset: Any, index: str) -> dict[str, Any]:
    start = time.perf_counter()
    with query_metrics() as collector:
        rows = sum(batch.num_rows for batch in dataset.reader(index=index).collect())
    elapsed_ms = (time.perf_counter() - start) * 1000

    metrics = collector.last_query()
    if metrics is None:
        raise RuntimeError("full scan produced no query metrics")

    return {"rows": rows, "wall_time_ms": elapsed_ms, **_query_metrics_dict(metrics)}


def main() -> None:
    parser = argparse.ArgumentParser(description="Measure REDAP narrow seek/scrub query metrics.")
    parser.add_argument("--redap-url", required=True, help="REDAP URL, e.g. rerun+http://localhost:51234")
    parser.add_argument("--redap-token", default=None, help="Optional REDAP auth token")
    parser.add_argument("--dataset", required=True, help="Dataset name")
    parser.add_argument("--index", required=True, help="Timeline/index name to scrub on")
    parser.add_argument("--layer-name", default="base", help="Layer name used when registering a remote source")
    parser.add_argument(
        "--register-prefix",
        default=None,
        help="Optional remote RRD prefix to register into --dataset before measuring",
    )
    parser.add_argument(
        "--hf-repo",
        default=None,
        help="Optional Hugging Face dataset repo id containing .rrd files, e.g. rerun/droid_sample",
    )
    parser.add_argument("--hf-revision", default="main", help="Hugging Face revision to resolve")
    parser.add_argument("--hf-path", default="", help="Optional subdirectory inside the Hugging Face dataset repo")
    parser.add_argument("--hf-token", default=None, help="Optional Hugging Face token")
    parser.add_argument("--hf-limit", type=int, default=None, help="Optional maximum number of HF .rrd files to register")
    parser.add_argument("--registration-timeout-secs", type=int, default=600)
    parser.add_argument("--seek-count", type=int, default=8)
    parser.add_argument(
        "--include-full-scan",
        action="store_true",
        help="Also collect the whole dataset once for comparison. Avoid this on very large datasets.",
    )
    parser.add_argument("--json", action="store_true", help="Emit JSON instead of a compact text table")
    args = parser.parse_args()

    if args.register_prefix is not None and args.hf_repo is not None:
        raise ValueError("--register-prefix and --hf-repo are mutually exclusive")

    dataset = _dataset(args)
    seek_points = _sample_seek_points(dataset, args.index, args.seek_count)
    if not seek_points:
        raise RuntimeError(f"could not sample any seek points from index {args.index!r}")

    result = {
        "redap_url": args.redap_url,
        "dataset": args.dataset,
        "index": args.index,
        "full_scan": _run_full_scan(dataset, args.index) if args.include_full_scan else None,
        "seeks": [_run_seek(dataset, args.index, point) for point in seek_points],
    }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
        return

    full = result["full_scan"]
    if full is not None:
        print(
            "full_scan "
            f"rows={full['rows']} chunks={full['query_chunks']} "
            f"query_bytes={full['query_bytes']} fetch_bytes={full['fetch_bytes']} "
            f"wall_ms={full['wall_time_ms']:.1f}"
        )

    print("seek segment_id seek_value rows chunks query_bytes fetch_bytes direct_bytes grpc_bytes wall_ms")
    for seek in result["seeks"]:
        print(
            f"seek {seek['segment_id']} {seek['seek_value']} {seek['rows']} "
            f"{seek['query_chunks']} {seek['query_bytes']} {seek['fetch_bytes']} "
            f"{seek['fetch_direct_bytes']} {seek['fetch_grpc_bytes']} {seek['wall_time_ms']:.1f}"
        )


if __name__ == "__main__":
    main()
