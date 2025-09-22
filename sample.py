from __future__ import annotations

import argparse
from pathlib import Path

import rerun as rr
from rerun.experimental.launch_server import start_rerun_server
from rerun.experimental.write_dataframe import write_dataframe_to_rrd


def cli() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "dataset",
        type=str,
        help="Path to the dataset to convert. Must be rrd already for now.",
    )
    parser.add_argument("output_dir", type=Path, help="Directory to write the output rrd files to.")
    return parser


def main() -> None:
    args = cli().parse_args()

    # Start the rerun server in the background
    start_rerun_server(args.dataset)

    CATALOG_URL = "rerun+http://localhost:51234"
    client = rr.catalog.CatalogClient(CATALOG_URL)
    all_entries = client.all_entries()
    first_entry = all_entries[0].name
    dataset = client.get_dataset_entry(name=first_entry)
    partitions = dataset.partition_table().df().to_arrow_table()["rerun_partition_id"].to_pylist()
    # Run the main processing
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_dataframe_to_rrd(dataset, args.output_dir, partitions)

    # The server will be automatically cleaned up when the script exits


if __name__ == "__main__":
    main()
