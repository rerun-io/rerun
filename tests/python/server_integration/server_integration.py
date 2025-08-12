#!/usr/bin/env python3
"""
A collection of many small examples, in one file.

It uses a lot of different aspects of the Rerun API in order to test it.

Example usage:
* Run all tests: `examples/python/test_api/test_api.py`
* Run specific test: `examples/python/test_api/test_api.py --test rects`
"""

from __future__ import annotations

import argparse
import logging

import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F

CATALOG_URL = "rerun+http://localhost:51234"
DATASET = "dataset"


def aggregation_test() -> None:
    client = rr.catalog.CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET)

    results = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .unnest_columns("/obj1:Points3D:positions")
        .aggregate(
            [],
            [
                F.min(col("/obj1:Points3D:positions")[0]).alias("min_x"),
                F.max(col("/obj1:Points3D:positions")[0]).alias("max_x"),
            ],
        )
        .collect()
    )

    assert results[0][0][0] == pa.scalar(1.0, type=pa.float32())
    assert results[0][1][0] == pa.scalar(50.0, type=pa.float32())


def partition_ordering_test() -> None:
    client = rr.catalog.CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET)

    for time_index in ["time_1", "time_2", "time_3"]:
        streams = (
            dataset.dataframe_query_view(index=time_index, contents="/**")
            .fill_latest_at()
            .df()
            .select("rerun_partition_id", time_index)
            .execute_stream_partitioned()
        )

        prior_partition_ids = set()
        for rb_reader in streams:
            prior_partition = ""
            prior_timestamp = 0
            for rb in iter(rb_reader):
                rb = rb.to_pyarrow()
                for idx in range(rb.num_rows):
                    partition = rb[0][idx].as_py()
                    timestamp = rb[1][idx].as_py()

                    assert partition >= prior_partition
                    if partition == prior_partition and timestamp is not None:
                        assert timestamp >= prior_timestamp
                    else:
                        assert partition not in prior_partition_ids
                        prior_partition_ids.add(partition)

                    prior_partition = partition
                    if timestamp is not None:
                        prior_timestamp = timestamp


def main() -> None:
    tests = {
        "aggregation_test": aggregation_test,
        "test_partition_ordering": partition_ordering_test,
    }

    parser = argparse.ArgumentParser(description="Tests the gRPC interface between rerun SDK and server")
    parser.add_argument(
        "--test",
        type=str,
        default="most",
        help="What test to run",
        choices=["most", "all"] + list(tests.keys()),
    )

    args = parser.parse_args()

    if args.test in ["most", "all"]:
        print(f"Running {args.test} tests…")

        for name, test in tests.items():
            logging.info(f"Starting {name}")
            test()

    else:
        tests[args.test]()


if __name__ == "__main__":
    main()
