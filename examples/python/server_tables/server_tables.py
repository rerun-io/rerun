#!/usr/bin/env python3
"""Demonstrates an example workflow of processing datasets and writing to tables."""

from __future__ import annotations

import tempfile
from datetime import datetime
from pathlib import Path

import pyarrow as pa
from datafusion import DataFrame, col, functions as F
from rerun.catalog import CatalogClient, DatasetEntry

CATALOG = "rerun+http://localhost:51234"
DATASET_NAME = "dataset"

STATUS_TABLE_NAME = "status"
RESULTS_TABLE_NAME = "results"


def create_table(client: CatalogClient, directory: Path, table_name: str, schema: pa.Schema) -> DataFrame:
    """
    Create a lance table at a specified location and return its DataFrame.

    This is a convenience function for creating the status and result tables.
    """
    if table_name in client.table_names():
        return client.get_table(name=table_name)

    url = f"file://{directory}/{table_name}"

    return client.create_table_entry(table_name, schema, url).df()


def create_status_table(client: CatalogClient, directory: Path) -> DataFrame:
    """Create the status table."""
    schema = pa.schema([
        ("rerun_partition_id", pa.utf8()),
        ("is_complete", pa.bool_()),
        ("update_time", pa.timestamp(unit="ms")),
    ])
    return create_table(client, directory, STATUS_TABLE_NAME, schema)


def create_results_table(client: CatalogClient, directory: Path) -> DataFrame:
    """Create the results table."""
    schema = pa.schema([
        ("rerun_partition_id", pa.utf8()),
        ("first_log_time", pa.timestamp(unit="ns")),
        ("last_log_time", pa.timestamp(unit="ns")),
        ("first_position_obj1", pa.list_(pa.float32(), 3)),
        ("first_position_obj2", pa.list_(pa.float32(), 3)),
        ("first_position_obj3", pa.list_(pa.float32(), 3)),
    ])
    return create_table(client, directory, RESULTS_TABLE_NAME, schema)


def find_missing_partitions(partition_table: DataFrame, status_table: DataFrame) -> list[str]:
    """Query the status table for partitions that have not processed."""
    status_table = status_table.filter(col("is_complete"))
    partitions = partition_table.join(status_table, on="rerun_partition_id", how="anti").collect()
    return [str(r) for rss in partitions for rs in rss for r in rs]


def process_partitions(client: CatalogClient, dataset: DatasetEntry, partition_list: list[str]) -> None:
    """
    Example code for processing some partitions within a dataset.

    This example performs a simple aggregation of some of the values stored in the dataset that
    might be useful for further processing or metrics extraction. In this work flow we first write
    to the status table that we have started work but set the `is_complete` column to `False`.
    When the work is complete we write an additional row setting this column to `True`. Alternate
    workflows may only include writing to the table when work is complete. It is sometimes favorable
    to keep track of when jobs start and finish so you can produce additional metrics around
    when the jobs ran and how long they took.
    """
    client.append_to_table(
        STATUS_TABLE_NAME,
        rerun_partition_id=partition_list,
        is_complete=[False] * len(partition_list),
        update_time=[datetime.now()] * len(partition_list),
    )

    df = dataset.dataframe_query_view(index="time_1", contents="/**").filter_partition_id(*partition_list).df()

    df = df.aggregate(
        "rerun_partition_id",
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

    client.append_to_table(
        STATUS_TABLE_NAME,
        rerun_partition_id=partition_list,
        is_complete=[True] * len(partition_list),  # Add the `True` value to prevent this from processing again
        update_time=[datetime.now()] * len(partition_list),
    )


def main(temp_path: Path) -> None:
    client = CatalogClient(CATALOG)
    dataset = client.get_dataset(name=DATASET_NAME)

    status_table = create_status_table(client, temp_path)
    results_table = create_results_table(client, temp_path)

    # TODO(tsaucer) replace with partition table query
    partition_table = (
        dataset.dataframe_query_view(index="time_1", contents="/**").df().select("rerun_partition_id").distinct()
    )

    missing_partitions = None
    while missing_partitions is None or len(missing_partitions) != 0:
        missing_partitions = find_missing_partitions(partition_table, status_table)
        print(f"{len(missing_partitions)} of {partition_table.count()} partitions have not processed.")

        if len(missing_partitions) > 0:
            process_partitions(client, dataset, missing_partitions[0:3])

    # Show the final results
    print("Results table:")
    results_table.show()

    # Show the final status table
    print("Final status table:")
    status_table.show()


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = Path(temp_dir)
        main(temp_path)
