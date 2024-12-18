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
    register_cmd = subparsers.add_parser("register", help="Register a new recording")
    update_cmd = subparsers.add_parser("update", help="Update metadata for a recording")

    update_cmd.add_argument("id", help="ID of the recording to update")
    update_cmd.add_argument("key", help="Key of the metadata to update")
    update_cmd.add_argument("value", help="Value of the metadata to update")

    register_cmd.add_argument("storage_url", help="Storage URL to register")

    args = parser.parse_args()

    # Register the new rrd
    conn = rr.remote.connect("http://0.0.0.0:51234")

    catalog = pl.from_arrow(conn.query_catalog().read_all())

    if args.subcommand == "print":
        print(catalog)

    elif args.subcommand == "register":
        extra_metadata = pa.Table.from_pydict({"extra": [42]})
        id = conn.register(args.storage_url, extra_metadata)
        print(f"Registered new recording with ID: {id}")

    elif args.subcommand == "update":
        id = catalog.filter(catalog["id"].str.starts_with(args.id)).select(pl.first("id")).item()

        if id is None:
            print("ID not found")
            exit(1)
        print(f"Updating metadata for {id}")

        new_metadata = pa.Table.from_pydict({"id": [id], args.key: [args.value]})
        print(new_metadata)

        conn.update_catalog(new_metadata)
