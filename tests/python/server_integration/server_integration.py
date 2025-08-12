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
import rerun as rr

from datafusion import col, functions as F
import pyarrow as pa

CATALOG_URL = "rerun+http://localhost:51234"
DATASET = "dataset"


def aggregation_test() -> None:

    client = rr.catalog.CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET)

    results = (
        dataset
        .dataframe_query_view(index="my_timestamp", contents="/**")
        .df()
        .unnest_columns("/obj1:Points3D:positions")
        .aggregate(
            [],
            [
                F.min(col("/obj1:Points3D:positions")[0]).alias("min_x"),
                F.max(col("/obj1:Points3D:positions")[0]).alias("max_x"),
            ]
        )
        .collect()
    )

    assert results[0][0][0] == pa.scalar(-17.0, type=pa.float32())
    assert results[0][1][0] == pa.scalar(19.0, type=pa.float32())


def main() -> None:
    tests = {
        "aggregation_test": aggregation_test,
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

        threads = []
        for name, test in tests.items():

            logging.info(f"Starting {name}")
            test()

    else:
        tests[args.test]()


if __name__ == "__main__":
    main()
