#!/usr/bin/env python3
"""Demonstrates an example workflow of processing datasets and writing to tables."""

from __future__ import annotations

import argparse
import tempfile
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pyarrow as pa
import rerun as rr
from datafusion import DataFrame, col, functions as F
from rerun.server import Server
from rerun.utilities.datafusion.collect import collect_to_string_list

if TYPE_CHECKING:
    from rerun.catalog import CatalogClient, DatasetEntry

DATASET_NAME = "dataset"

STATUS_LOG_TABLE_NAME = "status_log"
RESULTS_TABLE_NAME = "results"


def create_table(client: CatalogClient, directory: Path, table_name: str, schema: pa.Schema) -> DataFrame:
    """
    Create a lance table at a specified location and return its DataFrame.

    This is a convenience function for creating the status log and result tables.
    """
    if table_name in client.table_names():
        return client.get_table(name=table_name).reader()

    url = f"file://{directory}/{table_name}"

    return client.create_table(table_name, schema, url).reader()


def create_status_log_table(client: CatalogClient, directory: Path) -> DataFrame:
    """Create the status log table."""
    schema = pa.schema([
        pa.field("rerun_segment_id", pa.utf8()).with_metadata({rr.SORBET_IS_TABLE_INDEX: "true"}),
        pa.field("is_complete", pa.bool_()),
        pa.field("update_time", pa.timestamp(unit="ms")),
    ])
    return create_table(client, directory, STATUS_LOG_TABLE_NAME, schema)


def create_results_table(client: CatalogClient, directory: Path) -> DataFrame:
    """Create the results table."""
    schema = pa.schema([
        ("rerun_segment_id", pa.utf8()),
        ("first_log_time", pa.timestamp(unit="ns")),
        ("last_log_time", pa.timestamp(unit="ns")),
        ("first_position_obj1", pa.list_(pa.float32(), 3)),
        ("first_position_obj2", pa.list_(pa.float32(), 3)),
        ("first_position_obj3", pa.list_(pa.float32(), 3)),
    ])
    return create_table(client, directory, RESULTS_TABLE_NAME, schema)


def find_missing_segments(segment_table: DataFrame, status_log_table: DataFrame) -> list[str]:
    """Query the status log table for segments that have not processed."""
    status_log_table = status_log_table.filter(col("is_complete"))
    segments = segment_table.join(status_log_table, on="rerun_segment_id", how="anti")

    segment_list = collect_to_string_list(segments, "rerun_segment_id")

    # This cast is to satisfy mypy type checking. It is not strictly necessary.
    return cast("list[str]", segment_list)


def process_segments(client: CatalogClient, dataset: DatasetEntry, segment_list: list[str]) -> None:
    """
    Example code for processing some segments within a dataset.

    This example performs a simple aggregation of some of the values stored in the dataset that
    might be useful for further processing or metrics extraction. In this work flow we first write
    to the status log table that we have started work but set the `is_complete` column to `False`.
    When the work is complete we write an additional row setting this column to `True`. Alternate
    workflows may only include writing to the table when work is complete. It is sometimes favorable
    to keep track of when jobs start and finish so you can produce additional metrics around
    when the jobs ran and how long they took.
    """
    status_log_table = client.get_table(name=STATUS_LOG_TABLE_NAME)
    status_log_table.append(
        rerun_segment_id=segment_list,
        is_complete=[False] * len(segment_list),
        update_time=[datetime.now()] * len(segment_list),
    )

    df = dataset.filter_segments(segment_list).reader(index="time_1")

    df = df.aggregate(
        "rerun_segment_id",
        [
            F.min(col("log_time")).alias("first_log_time"),
            F.max(col("log_time")).alias("last_log_time"),
            F.first_value(
                col("/obj1:Points3D:positions")[0],
                filter=col("/obj1:Points3D:positions").is_not_null(),
                order_by=col("time_1"),
            ).alias("first_position_obj1"),
            F.first_value(
                col("/obj2:Points3D:positions")[0],
                filter=col("/obj2:Points3D:positions").is_not_null(),
                order_by=col("time_1"),
            ).alias("first_position_obj2"),
            F.first_value(
                col("/obj3:Points3D:positions")[0],
                filter=col("/obj3:Points3D:positions").is_not_null(),
                order_by=col("time_1"),
            ).alias("first_position_obj3"),
        ],
    )

    df.write_table(RESULTS_TABLE_NAME)

    # This command will replace the existing rows with a `True` completion status.
    # If instead you wish to measure how long it takes your workflow to run, you
    # can use an append statement as in the previous write.
    status_log_table.upsert(
        rerun_segment_id=segment_list,
        is_complete=[True] * len(segment_list),
        update_time=[datetime.now()] * len(segment_list),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Process some segments in a dataset.")
    parser.add_argument("--temp-dir", type=str, default=None, help="Temporary directory to store tables.")
    # TODO(#11760): Remove unneeded args when examples infra is fixed.
    rr.script_add_args(parser)
    args = parser.parse_args()
    # TODO(#11760): Fake output to satisfy examples infra.
    Path(args.save).touch()
    temp_dir = args.temp_dir
    if args.temp_dir is not None:
        run_example(Path(temp_dir))
    else:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            run_example(temp_path)


def run_example(temp_path: Path) -> None:
    root_path = Path(__file__).parent.parent.parent.parent.resolve()
    with Server(datasets={DATASET_NAME: root_path / "tests/assets/rrd/dataset"}) as srv:
        client = srv.client()
        dataset = client.get_dataset(name=DATASET_NAME)

        status_log_table = create_status_log_table(client, temp_path)
        results_table = create_results_table(client, temp_path)

        segment_table = dataset.segment_table().select("rerun_segment_id").distinct()

        missing_segments = None
        while missing_segments is None or len(missing_segments) != 0:
            missing_segments = find_missing_segments(segment_table, status_log_table)
            print(f"{len(missing_segments)} of {segment_table.count()} segments have not processed.")

            if len(missing_segments) > 0:
                process_segments(client, dataset, missing_segments[0:3])

        # Show the final results
        print("Results table:")
        results_table.show()

        # Show the final status log table
        print("Final status log table:")
        status_log_table.show()


if __name__ == "__main__":
    main()
