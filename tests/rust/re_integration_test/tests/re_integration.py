#!/usr/bin/env python3
"""
Connect to the OSS Server (or Rerun Cloud Server) and
* Add some data to it
* Query out that data again
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

    # Create a simple recording:
    filepath = "/tmp/rerun_example_test.rrd"
    rec = rr.RecordingStream("rerun_example_test", recording_id="new_recording_id")
    rec.save(filepath)
    for x in range(20):
        rec.set_time("test_time", sequence=x)
        rec.log(chr(ord("a") + x % 3), rr.Scalars(x))
    rec.flush()

    client = rr.catalog.CatalogClient(args.url)
    assert len(client.all_entries()) == 0

    dataset = client.create_dataset("my_dataset")
    dataset.register(f"file://{filepath}")

    candidate_url = dataset.partition_url("new_recording_id") + "#test_time=5"

    print("Run the command:")
    print(f'pixi run rerun "{candidate_url}"')


if __name__ == "__main__":
    main()
