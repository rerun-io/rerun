#!/usr/bin/env python3
"""
Connect to the OSS Server (or Rerun Cloud Server) and test it.

* Add some data to it
* Query out that data again.
"""

from __future__ import annotations

from argparse import ArgumentParser

import rerun as rr


def main() -> None:
    parser = ArgumentParser(description="Test OSS Server")
    parser.add_argument(
        "--url",
        default="rerun+http://localhost:51234",
        help="Which dataset to automatically download and visualize",
    )
    args = parser.parse_args()

    dataset_name = "my_dataset"

    # Create a simple recording:
    filepath = "/tmp/rerun_example_test.rrd"
    rec = rr.RecordingStream("rerun_example_test", recording_id="new_recording_id")
    rec.save(filepath)
    for x in range(20):
        rec.set_time("test_time", sequence=x)
        rec.log("/scalar", rr.Scalars(x))
    rec.flush()

    client = rr.catalog.CatalogClient(args.url)
    if len(client.all_entries()) != 0:
        print(f"Expected no catalogs, found {len(client.all_entries())}")

    print(f"All datasets: {client.dataset_names()}")

    # When testing its useful if you can run the script multiple times
    if dataset_name in client.dataset_names():
        print(f"Deleting existing dataset '{dataset_name}'…")
        dataset = client.get_dataset(name=dataset_name)
        dataset.delete()

    print(f"Creating dataset '{dataset_name}'…")
    dataset = client.create_dataset(dataset_name)

    dataset.register(f"file://{filepath}")

    df = (
        dataset.dataframe_query_view(
            index="test_time",
            contents={"/scalar": ["Scalars:scalars"]},
        )
        .df()
        .drop("log_time")
    )
    print(f"{df}")


if __name__ == "__main__":
    main()
