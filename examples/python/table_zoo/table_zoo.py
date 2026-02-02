"""
A table with all kinds of datatypes and nullability, for testing purposes.

Usage:
  pixi run py-build
  pixi run -e py python tests/python/table_zoo/table_zoo.py [--host HOST] [--port PORT] [--register-to-server]

By default, behaves as before: sends the table to the Rerun Viewer via ViewerClient at rerun+http://127.0.0.1:9876/proxy.
"""

from __future__ import annotations

import argparse
import shutil
import sys
from datetime import datetime, timedelta
from pathlib import Path

import lancedb
import pyarrow as pa
import rerun as rr
from platformdirs import user_cache_dir
from rerun.catalog import CatalogClient


def _build_record_batch() -> tuple[str, pa.RecordBatch]:
    """
    Build the table_zoo RecordBatch and return (name, batch).

    The dummy table content is fully constructed inside this function to keep
    module import time minimal and avoid leaking large constants to the module
    scope.
    """
    # Expanded to 10 rows
    string_non_null = [
        "Hello",
        "WORLD",
        "PyThOn",
        "Arrow",
        "DATA",
        "Test",
        "ReRun",
        "viewer",
        "TABLE",
        "Query",
    ]
    string_nullable = ["Foo", None, "BAR", "baz", None, "QuX", "RUST", None, "Sdk", "API"]
    string_list_nullable = [
        ["the BAR"],
        [],
        ["Clothe", "BARRACK"],
        None,
        ["Cherry", None, "DATE"],
        [None],
        ["APPLE", "Banana"],
        None,
        ["x", "Y", "Z"],
        ["Final"],
    ]

    # Bool columns
    bool_non_null = [True, False, True, True, False, True, False, True, False, True]
    bool_nullable = [True, None, False, True, None, False, True, None, False, None]
    bool_list_nullable = [
        [True, False],
        None,
        [None, True, True],
        [None, None],
        [False],
        [None, True, False],
        [],
        [True, True],
        None,
        [False, None],
    ]

    # Int64 columns
    int64_non_null = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    int64_nullable = [10, None, 30, 40, None, 60, 70, None, 90, 100]
    int64_list_nullable = [
        [100, 200],
        None,
        [300, None, 100],
        [],
        [100],
        [None, 100, 700],
        [800],
        None,
        [900, 1000],
        [],
    ]

    # Float64 columns
    float64_non_null = [1.1, 2.2, 3.3, 4.4, 5.5, 6.6, 7.7, 8.8, 9.9, 10.1]
    float64_nullable = [10.1, None, 30.3, 40.4, None, 60.6, 70.7, None, 90.9, 100.1]
    float64_list_nullable = [
        [100.1, 200.2],
        None,
        [300.3, None, 400.4],
        [],
        [500.5],
        [None, 600.6, 700.7],
        [800.8],
        None,
        [900.9, 1000.1],
        [],
    ]

    # Create timestamps
    base = datetime.today()
    timestamps = [
        base,  # Today
        base - timedelta(days=1, hours=3),  # Yesterday
        base - timedelta(days=3),  # This week (Friday)
        base - timedelta(days=5),  # This week (Wednesday)
        base - timedelta(days=8),  # Last week
        base - timedelta(days=12),  # Last week
        datetime(2024, 12, 25, 10, 30, 45),  # Old (Christmas 2024)
        datetime(2020, 1, 1, 0, 0, 0),  # Old (2020)
        datetime(2023, 6, 15, 14, 20, 30),  # Old (2023)
        base - timedelta(hours=6),  # Today morning
    ]

    # Convert to various units (as integers for Arrow timestamp types)
    ts_sec = [int(ts.timestamp()) for ts in timestamps]
    ts_ms = [int(ts.timestamp() * 1000) for ts in timestamps]
    ts_us = [int(ts.timestamp() * 1_000_000) for ts in timestamps]
    ts_ns = [int(ts.timestamp() * 1_000_000_000) for ts in timestamps]

    # Create nullable versions (with None at indices 1, 4, 7)
    ts_sec_nullable = [ts if i not in [1, 4, 7] else None for i, ts in enumerate(ts_sec)]
    ts_ms_nullable = [ts if i not in [1, 4, 7] else None for i, ts in enumerate(ts_ms)]
    ts_us_nullable = [ts if i not in [1, 4, 7] else None for i, ts in enumerate(ts_us)]
    ts_ns_nullable = [ts if i not in [1, 4, 7] else None for i, ts in enumerate(ts_ns)]

    # Create list versions with various patterns
    ts_sec_list = [
        [ts_sec[0], ts_sec[1]],
        None,
        [ts_sec[2]],
        [],
        [ts_sec[3], None, ts_sec[4]],
        [None],
        [ts_sec[5], ts_sec[6]],
        None,
        [ts_sec[7], ts_sec[8]],
        [ts_sec[9]],
    ]
    ts_ms_list = [
        [ts_ms[0]],
        None,
        [ts_ms[1], ts_ms[2]],
        [None, None],
        [ts_ms[3]],
        [],
        [ts_ms[4], None],
        None,
        [ts_ms[5], ts_ms[6], ts_ms[7]],
        [None, ts_ms[8]],
    ]
    ts_us_list = [
        [ts_us[0], ts_us[1]],
        None,
        [],
        [ts_us[2]],
        [None, ts_us[3]],
        [ts_us[4]],
        None,
        [ts_us[5], None, ts_us[6]],
        [],
        [ts_us[7], ts_us[8], ts_us[9]],
    ]
    ts_ns_list = [
        [],
        None,
        [ts_ns[0], ts_ns[1], ts_ns[2]],
        [None],
        [ts_ns[3], None],
        [ts_ns[4], ts_ns[5]],
        None,
        [],
        [ts_ns[6]],
        [None, ts_ns[7], ts_ns[8]],
    ]

    # Define the schema
    schema = pa.schema([
        # String columns
        pa.field("string_non_null", pa.string(), nullable=False),
        pa.field("string_nullable", pa.string(), nullable=True),
        pa.field("string_list_nullable", pa.list_(pa.string()), nullable=True),
        # Bool columns
        pa.field("bool_non_null", pa.bool_(), nullable=False),
        pa.field("bool_nullable", pa.bool_(), nullable=True),
        pa.field("bool_list_nullable", pa.list_(pa.bool_()), nullable=True),
        # Int64 columns
        pa.field("int64_non_null", pa.int64(), nullable=False),
        pa.field("int64_nullable", pa.int64(), nullable=True),
        pa.field("int64_list_nullable", pa.list_(pa.int64()), nullable=True),
        # Float64 columns
        pa.field("float64_non_null", pa.float64(), nullable=False),
        pa.field("float64_nullable", pa.float64(), nullable=True),
        pa.field("float64_list_nullable", pa.list_(pa.float64()), nullable=True),
        # Timestamp columns (seconds)
        pa.field("ts_sec_non_null", pa.timestamp("s"), nullable=False),
        pa.field("ts_sec_nullable", pa.timestamp("s"), nullable=True),
        pa.field("ts_sec_list_nullable", pa.list_(pa.timestamp("s")), nullable=True),
        # Timestamp columns (milliseconds)
        pa.field("ts_ms_non_null", pa.timestamp("ms"), nullable=False),
        pa.field("ts_ms_nullable", pa.timestamp("ms"), nullable=True),
        pa.field("ts_ms_list_nullable", pa.list_(pa.timestamp("ms")), nullable=True),
        # Timestamp columns (microseconds)
        pa.field("ts_us_non_null", pa.timestamp("us"), nullable=False),
        pa.field("ts_us_nullable", pa.timestamp("us"), nullable=True),
        pa.field("ts_us_list_nullable", pa.list_(pa.timestamp("us")), nullable=True),
        # Timestamp columns (nanoseconds)
        pa.field("ts_ns_non_null", pa.timestamp("ns"), nullable=False),
        pa.field("ts_ns_nullable", pa.timestamp("ns"), nullable=True),
        pa.field("ts_ns_list_nullable", pa.list_(pa.timestamp("ns")), nullable=True),
        # rr.components.Timestamp column
        pa.field(
            "timestamp_component",
            pa.list_(pa.int64()),
            nullable=True,
            metadata={"rerun:component_type": "rerun.components.Timestamp"},
        ),
    ])

    # Create arrays for each column
    arrays = [
        # String arrays
        pa.array(string_non_null, type=pa.string()),
        pa.array(string_nullable, type=pa.string()),
        pa.array(string_list_nullable, type=pa.list_(pa.string())),
        # Bool arrays
        pa.array(bool_non_null, type=pa.bool_()),
        pa.array(bool_nullable, type=pa.bool_()),
        pa.array(bool_list_nullable, type=pa.list_(pa.bool_())),
        # Int64 arrays
        pa.array(int64_non_null, type=pa.int64()),
        pa.array(int64_nullable, type=pa.int64()),
        pa.array(int64_list_nullable, type=pa.list_(pa.int64())),
        # Float64 arrays
        pa.array(float64_non_null, type=pa.float64()),
        pa.array(float64_nullable, type=pa.float64()),
        pa.array(float64_list_nullable, type=pa.list_(pa.float64())),
        # Timestamp arrays (seconds)
        pa.array(ts_sec, type=pa.timestamp("s")),
        pa.array(ts_sec_nullable, type=pa.timestamp("s")),
        pa.array(ts_sec_list, type=pa.list_(pa.timestamp("s"))),
        # Timestamp arrays (milliseconds)
        pa.array(ts_ms, type=pa.timestamp("ms")),
        pa.array(ts_ms_nullable, type=pa.timestamp("ms")),
        pa.array(ts_ms_list, type=pa.list_(pa.timestamp("ms"))),
        # Timestamp arrays (microseconds)
        pa.array(ts_us, type=pa.timestamp("us")),
        pa.array(ts_us_nullable, type=pa.timestamp("us")),
        pa.array(ts_us_list, type=pa.list_(pa.timestamp("us"))),
        # Timestamp arrays (nanoseconds)
        pa.array(ts_ns, type=pa.timestamp("ns")),
        pa.array(ts_ns_nullable, type=pa.timestamp("ns")),
        pa.array(ts_ns_list, type=pa.list_(pa.timestamp("ns"))),
        # rr.components.Timestamp column
        pa.array([[ts] for ts in ts_ns], type=pa.list_(pa.int64())),
    ]

    record_batch = pa.RecordBatch.from_arrays(arrays, schema=schema)
    return "table_zoo", record_batch


def _get_cache_dir() -> Path:
    """Return an OS-appropriate cache directory for storing Lance tables."""
    p = Path(user_cache_dir(appname="rerun", appauthor=False))
    p.mkdir(parents=True, exist_ok=True)
    return p


def _write_recordbatch_to_lance(reader: pa.RecordBatchReader, path: Path | str) -> str:
    """
    Write a PyArrow RecordBatchReader to a new Lance DB table and return file:// URI.

    Safe to run multiple times: overwrites any existing Lance table at the same location.
    """
    path = Path(path)
    table_name = path.name
    db_path = path.parent
    db_path.mkdir(parents=True, exist_ok=True)

    # Remove any existing Lance table directory to avoid AlreadyExists errors.
    table_path = db_path / f"{table_name}.lance"
    if table_path.exists():
        shutil.rmtree(table_path, ignore_errors=True)

    db = lancedb.connect(str(db_path))
    # Try using an overwrite-capable API if available; fall back otherwise.
    try:
        db.create_table(name=table_name, data=pa.Table.from_batches(reader), mode="overwrite")
    except TypeError:
        db.create_table(name=table_name, data=pa.Table.from_batches(reader))

    return f"file://{table_path.absolute().as_posix()}"


def _run_viewer_mode(host: str, port: int) -> None:
    name, batch = _build_record_batch()
    addr = f"rerun+http://{host}:{port}/proxy"
    client = rr.experimental.ViewerClient(addr=addr)
    client.send_table(name, batch)


def _run_register_mode(host: str, port: int, cache_dir: Path) -> None:
    name, batch = _build_record_batch()

    # Build a RecordBatchReader for Lance writing
    reader = pa.RecordBatchReader.from_batches(batch.schema, [batch])

    cache_root = cache_dir
    cache_root.mkdir(parents=True, exist_ok=True)

    # Place table under cache_root; last component is table name
    uri = _write_recordbatch_to_lance(reader, cache_root / name)

    c = CatalogClient(f"rerun+http://{host}:{port}")

    try:
        entry = c.get_table(name)
        entry.delete()
    except Exception:
        pass

    c.register_table(name, uri)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate a table with diverse datatypes and either send to Viewer or register on server."
    )
    # Compute the exact default cache directory used when --cache-dir is not provided.
    default_cache_dir = _get_cache_dir() / "lancedb"
    parser.add_argument(
        "--host", default="0.0.0.0", help="Server host or IP. Default matches current viewer behavior: 0.0.0.0"
    )
    parser.add_argument(
        "--port", type=int, help="Server port. Defaults: 9876 for viewer mode; 51234 for register mode."
    )
    parser.add_argument(
        "--register-to-server",
        "-r",
        action="store_true",
        help="Register the table to a local redap server using CatalogClient instead of sending to Viewer.",
    )
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=None,
        help=f"Optional cache directory to store Lance table when registering. Defaults to: {default_cache_dir}",
    )

    args = parser.parse_args()

    if args.register_to_server:
        # Default port for catalog server if not specified
        port = args.port if args.port is not None else 51234
        try:
            _run_register_mode(args.host, port, args.cache_dir or default_cache_dir)
        except Exception as e:
            print(f"Error in register-to-server mode: {e}", file=sys.stderr)
            return 1
    else:
        # Default port for viewer if not specified
        port = args.port if args.port is not None else 9876
        _run_viewer_mode(args.host, port)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
