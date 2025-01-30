"""Script to show how to interact with a remote storage node via python APIs."""

from __future__ import annotations

import argparse

import polars as pl
import pyarrow as pa
import rerun as rr

if __name__ == "__main__":
    parser = argparse.ArgumentParser()

    subparsers = parser.add_subparsers(dest="subcommand")

    print_cmd = subparsers.add_parser("print", help="Print everything")
    print_schema_cmd = subparsers.add_parser("print-schema", help="Print schema for a recording in the catalog")
    register_cmd = subparsers.add_parser("register", help="Register a new recording")
    update_cmd = subparsers.add_parser("update", help="Update metadata for a recording")

    update_cmd.add_argument("id", help="ID of the recording to update")
    update_cmd.add_argument("key", help="Key of the metadata to update")
    update_cmd.add_argument("value", help="Value of the metadata to update")

    register_cmd.add_argument("storage_url", help="Storage URL to register")

    print_cmd.add_argument("--columns", nargs="*", help="Define which columns to print")
    print_cmd.add_argument("--recording-ids", nargs="*", help="Select specific recordings to print")

    print_schema_cmd.add_argument("recording_id", help="Recording ID to print schema for")

    args = parser.parse_args()

    # Register the new rrd
    conn = rr.remote.connect("http://0.0.0.0:51234")

    if args.subcommand == "print":
        catalog = pl.from_arrow(conn.query_catalog(args.columns, args.recording_ids).read_all())
        print(catalog)

    elif args.subcommand == "register":
        extra_metadata = pa.Table.from_pydict({"extra": [42]})
        id = conn.register(args.storage_url, extra_metadata)
        print(f"Registered new recording with ID: {id}")

    elif args.subcommand == "update":
        catalog = pl.from_arrow(conn.query_catalog().read_all())

        id = (
            catalog.filter(catalog["rerun_recording_id"].str.starts_with(args.id))
            .select(pl.first("rerun_recording_id"))
            .item()
        )

        if id is None:
            print("ID not found")
            exit(1)
        print(f"Updating metadata for {id}")

        new_metadata = pa.Table.from_pydict({"rerun_recording_id": [id], args.key: [args.value]})
        print(new_metadata)

        conn.update_catalog(new_metadata)

    elif args.subcommand == "print-schema":
        id = args.recording_id

        schema = conn.get_recording_schema(id)
        for column in schema:
            print(column)
