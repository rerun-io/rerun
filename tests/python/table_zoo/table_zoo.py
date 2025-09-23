"""
A table with all kinds of datatypes and nullability, for testing purposes.

```
pixi run py-build
pixi run -e py python tests/python/table_zoo/table_zoo.py
"""

from __future__ import annotations

from datetime import datetime, timedelta

import pyarrow as pa
import rerun as rr

# Expanded to 10 rows
string_non_null = ["hello", "world", "python", "arrow", "data", "test", "rerun", "viewer", "table", "query"]
string_nullable = ["foo", None, "bar", "baz", None, "qux", "rust", None, "sdk", "api"]
string_list_nullable = [
    ["the bar"],
    [],
    ["clothe", "barrack"],
    None,
    ["cherry", None, "date"],
    [None],
    ["apple", "banana"],
    None,
    ["x", "y", "z"],
    ["final"],
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
int64_list_nullable = [[100, 200], None, [300, None, 100], [], [100], [None, 100, 700], [800], None, [900, 1000], []]

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

# Create timestamps (today is Sept 15, 2025)
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

# Create the RecordBatch
record_batch = pa.RecordBatch.from_arrays(arrays, schema=schema)

client = rr.experimental.ViewerClient(addr="rerun+http://0.0.0.0:9876/proxy")
client.send_table(
    "table_zoo",
    record_batch,
)
