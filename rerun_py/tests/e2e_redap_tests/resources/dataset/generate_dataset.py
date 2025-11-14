"""
Generate test dataset for e2e_redap_tests.

This script generates 20 .rrd files with test data containing:
- Three timelines (timestamp, duration, sequence) with intentionally unordered data
- Three objects with Points3D data
- Static text documents

The generated files are used for testing gRPC calls and query operations.
"""

import typing
from datetime import datetime, timedelta
import numpy as np
import random
import rerun as rr


_T = typing.TypeVar("_T")


def maybe_val(val: _T) -> _T | None:
    """Return value with 80% probability, None otherwise."""
    if random.random() > 0.2:
        return val
    else:
        return None


def generate_nanosecond_time(base_time: np.datetime64, minute_delta: int) -> np.datetime64:
    """Generate a timestamp with nanosecond precision."""
    base_ns = base_time.astype(np.int64)
    new_time_ns = base_ns + minute_delta * 60 * 1_000_000_000
    return np.datetime64(0, "ns") + np.timedelta64(new_time_ns, "ns")


def generate_data(filename: str, n_rows: int) -> None:
    """Generate a single .rrd file with test data."""
    # Intentionally create a timestamp that has values all the way down to nanosecond
    base_time = np.datetime64("2024-01-15T10:30:45.123456789")
    timestamps = [maybe_val(generate_nanosecond_time(base_time, i * 2)) for i in range(n_rows)]

    # Generate durations in minutes
    durations = [maybe_val(timedelta(minutes=30 + i * 5)) for i in range(n_rows)]

    # Generate sequence numbers
    sequence_numbers = list(maybe_val(x) for x in range(1, n_rows + 1))

    obj_x1 = list(maybe_val(float(x)) for x in range(1, n_rows + 1))
    obj_x2 = list(maybe_val(float(x)) for x in range(1, n_rows + 1))
    obj_x3 = list(maybe_val(float(x)) for x in range(1, n_rows + 1))

    obj1_indices = list(range(n_rows))
    random.shuffle(obj1_indices)

    obj2_indices = list(range(n_rows))
    random.shuffle(obj2_indices)

    obj3_indices = list(range(n_rows))
    random.shuffle(obj3_indices)

    rr.init(filename, spawn=True)
    rr.save(f"{filename}.rrd")

    rr.log("/text1", rr.TextDocument("Before text"), static=True)

    for idx in range(0, n_rows):
        rr.reset_time()

        timestamp = timestamps[obj1_indices[idx]]
        duration = durations[obj2_indices[idx]]
        sequence = sequence_numbers[obj3_indices[idx]]

        obj1_pos = obj_x1[obj1_indices[idx]]
        obj2_pos = obj_x2[obj2_indices[idx]]
        obj3_pos = obj_x3[obj3_indices[idx]]

        if timestamp is not None:
            rr.set_time("time_1", timestamp=timestamp)

        if duration is not None:
            rr.set_time("time_2", duration=duration)

        if sequence is not None:
            rr.set_time("time_3", sequence=sequence)

        if obj1_pos is not None:
            rr.log("/obj1", rr.Points3D([[obj1_pos, 0.0, 0.0]]))

        if obj2_pos is not None:
            rr.log("/obj2", rr.Points3D([[obj2_pos, 1.0, 0.0]]))

        if obj3_pos is not None:
            rr.log("/obj3", rr.Points3D([[obj3_pos, 2.0, 0.0]]))

    rr.log("/text2", rr.TextDocument("After text"), static=True)


if __name__ == "__main__":
    # Generate 20 files with alternating row counts (25 and 50)
    # This ensures we have enough partitions to test multi-threaded DataFusion operations
    for idx in range(1, 21):
        if idx % 2 == 1:
            generate_data(f"file{idx}", 25)
        else:
            generate_data(f"file{idx}", 50)
