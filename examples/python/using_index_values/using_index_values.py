#!/usr/bin/env python3
"""Demonstrates querying a dataset at specific index values."""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import pyarrow as pa

import rerun as rr
from rerun.server import Server

DATASET_NAME = "dataset"


def query_with_scalar_index_values(path_to_dataset: Path) -> None:
    """
    Query all segments at a fixed set of timestamps.

    When you pass index values directly (not per-segment), only segments
    whose time range actually covers those values will return data.
    Segments that don't overlap the requested timestamps are automatically
    excluded, avoiding unnecessary null rows.
    """
    with Server(datasets={DATASET_NAME: path_to_dataset}) as server:
        dataset = server.client().get_dataset(DATASET_NAME)

        # Pick timestamps to sample at.
        sample_times = np.array(
            [
                np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
                np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
            ],
            dtype=np.datetime64,
        )

        # Query at those exact timestamps across all segments.
        # Only segments whose index range covers a given timestamp will produce
        # a row for it -- other segments are excluded automatically.
        df = dataset.reader(
            index="time_1",
            using_index_values=sample_times,
            fill_latest_at=True,
        )

        print("=== Scalar index values (applied to all matching segments) ===")
        df.show()


def query_with_per_segment_index_values(path_to_dataset: Path) -> None:
    """
    Query specific segments at different timestamps.

    Pass a dict mapping segment IDs to index values when each segment
    needs its own set of sample points.
    """
    with Server(datasets={DATASET_NAME: path_to_dataset}) as server:
        dataset = server.client().get_dataset(DATASET_NAME)

        # Get available segment IDs
        segment_ids = sorted(dataset.segment_ids())
        print(f"Available segments: {segment_ids[:5]}{'…' if len(segment_ids) > 5 else ''}")

        if len(segment_ids) < 2:
            print("Need at least 2 segments for per-segment demo.")
            return

        # Different timestamps for different segments
        per_segment_values = {
            segment_ids[0]: np.array(
                [np.datetime64("2024-01-15T10:34:45.123456789", "ns")],
                dtype=np.datetime64,
            ),
            segment_ids[1]: np.array(
                [
                    np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
                    np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
                ],
                dtype=np.datetime64,
            ),
        }

        df = dataset.reader(
            index="time_1",
            using_index_values=per_segment_values,
            fill_latest_at=True,
        )

        print("\n=== Per-segment index values ===")
        df.show()


def query_with_dataframe_index_values(path_to_dataset: Path) -> None:
    """
    Query using a DataFrame of segment ID / index value pairs.

    This is the most flexible form: a DataFrame with 'rerun_segment_id'
    and index columns lets you specify exactly which (segment, timestamp)
    pairs to query.
    """
    with Server(datasets={DATASET_NAME: path_to_dataset}) as server:
        client = server.client()
        dataset = client.get_dataset(DATASET_NAME)

        segment_ids = sorted(dataset.segment_ids())
        if len(segment_ids) < 2:
            print("Need at least 2 segments for DataFrame demo.")
            return

        # Build a DataFrame with specific (segment_id, timestamp) pairs
        ctx = client.ctx
        index_df = ctx.from_pydict({
            "rerun_segment_id": pa.array([segment_ids[0], segment_ids[1], segment_ids[1]]),
            "time_1": pa.array(
                [1705314885123456789, 1705314885123456789, 1705315485123456789],
                type=pa.timestamp("ns"),
            ),
        })

        df = dataset.reader(
            index="time_1",
            using_index_values=index_df,
            fill_latest_at=True,
        )

        print("\n=== DataFrame index values ===")
        df.show()


def main() -> None:
    parser = argparse.ArgumentParser(description="Query a dataset at specific index values.")
    # TODO(#11760): Remove unneeded args when examples infra is fixed.
    rr.script_add_args(parser)
    args = parser.parse_args()
    # TODO(#11760): Fake output to satisfy examples infra.
    Path(args.save).touch()

    root_path = Path(__file__).parent.parent.parent.parent.resolve()
    path_to_dataset = root_path / "tests/assets/rrd/dataset"

    query_with_scalar_index_values(path_to_dataset)
    query_with_per_segment_index_values(path_to_dataset)
    query_with_dataframe_index_values(path_to_dataset)


if __name__ == "__main__":
    main()
