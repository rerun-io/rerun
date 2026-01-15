"""Utility functions for data conversion."""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np

if TYPE_CHECKING:
    from collections.abc import Iterable


def unwrap_singleton(value: object) -> object:
    """Unwrap single-element lists or arrays to their scalar value."""
    if isinstance(value, list) and len(value) == 1:
        return value[0]
    if isinstance(value, np.ndarray) and value.shape[:1] == (1,):
        return value[0]
    return value


def to_float32_vector(value: object, expected_dim: int, label: str) -> np.ndarray:
    """
    Convert a value to a float32 numpy array with expected dimensions.

    Args:
        value: Input value to convert
        expected_dim: Expected dimension of the output vector
        label: Label for error messages

    Returns:
        Float32 numpy array with shape (expected_dim,)

    Raises:
        ValueError: If value is None or has incorrect dimensions

    """
    if value is None:
        raise ValueError(f"Missing {label} value.")
    value = unwrap_singleton(value)
    array = np.asarray(value, dtype=np.float32)
    if array.ndim == 0:
        array = array.reshape(1)
    if array.ndim == 2 and array.shape[0] == 1:
        array = array[0]
    # Skip dimension check if expected_dim is -1 (variable length)
    if expected_dim != -1 and array.shape[0] != expected_dim:
        raise ValueError(f"{label} has dim {array.shape[0]} but expected {expected_dim}.")
    return array


def normalize_times(values: Iterable[object]) -> np.ndarray:
    """
    Normalize time values to nanosecond precision int64.

    Args:
        values: Iterable of time values (datetime64, timedelta64, float, int, or Pandas Timestamp)

    Returns:
        Int64 array representing nanoseconds

    """
    times = np.asarray(list(values))

    # Handle Pandas Timestamp objects
    if times.dtype == object and len(times) > 0:
        import pandas as pd

        if isinstance(times[0], pd.Timestamp):
            # Convert Pandas Timestamps to int64 nanoseconds
            return np.array([t.value for t in times], dtype="int64")

    if np.issubdtype(times.dtype, np.datetime64):
        return times.astype("datetime64[ns]").astype("int64")
    if np.issubdtype(times.dtype, np.timedelta64):
        return times.astype("timedelta64[ns]").astype("int64")
    if np.issubdtype(times.dtype, np.floating):
        return (times * 1_000_000_000.0).astype("int64")
    return times.astype("int64")


def make_time_grid(min_value: object, max_value: object, fps: int) -> np.ndarray:
    """
    Create a time grid at the specified FPS between min and max values.

    Args:
        min_value: Minimum time value
        max_value: Maximum time value
        fps: Frames per second for the grid

    Returns:
        Array of time values at regular intervals

    """
    min_array = np.asarray(min_value)
    if np.issubdtype(min_array.dtype, np.datetime64):
        step = np.timedelta64(int(1_000_000_000 / fps), "ns")
        if max_value <= min_value:
            return np.array([min_value])
        return np.arange(min_value, max_value, step)
    if max_value <= min_value:
        return np.array([min_value], dtype=np.float64)
    return np.arange(float(min_value), float(max_value), 1.0 / fps)


def get_entity_path(fully_qualified_column: str | None) -> str | None:
    """
    Extract the entity path from a fully qualified column name.

    The fully qualified column format is: "entity_path:ComponentName:field_name"
    This function extracts just the entity_path portion.

    Args:
        fully_qualified_column: Fully qualified column name (e.g., "/robot/joint_states:JointState:positions")

    Returns:
        Entity path (e.g., "/robot/joint_states"), or None if input is None

    Examples:
        >>> get_entity_path("/robot/joint_states:JointState:positions")
        "/robot/joint_states"
        >>> get_entity_path(None)
        None

    """
    if fully_qualified_column is None:
        return None
    return fully_qualified_column.split(":")[0]
