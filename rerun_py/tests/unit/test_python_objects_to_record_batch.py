from __future__ import annotations

import pyarrow as pa
import pytest
from rerun.catalog._entry import _python_objects_to_record_batch


def test_list_column_mismatch_error_message() -> None:
    """Test that list column mismatch provides a helpful error message."""
    schema = pa.schema([
        pa.field("id", pa.utf8()),
        pa.field("tags", pa.list_(pa.utf8())),
    ])

    with pytest.raises(ValueError) as exc_info:
        _python_objects_to_record_batch(
            schema,
            {
                "id": "row1",
                "tags": ["a", "b", "c"],  # Should be [["a", "b", "c"]] for single row
            },
        )

    error_message = str(exc_info.value)
    assert "tags" in error_message
    assert "3 rows" in error_message
    assert "expected 1" in error_message
    assert "Hint" in error_message
    assert "[[...]]" in error_message  # NOLINT


def test_list_column_properly_wrapped() -> None:
    """Test that properly wrapped list columns work correctly."""
    schema = pa.schema([
        pa.field("id", pa.utf8()),
        pa.field("tags", pa.list_(pa.utf8())),
    ])

    batch = _python_objects_to_record_batch(
        schema,
        {
            "id": "row1",
            "tags": [["a", "b", "c"]],
        },
    )

    assert batch is not None
    assert batch.num_rows == 1
    assert batch.column("id").to_pylist() == ["row1"]
    assert batch.column("tags").to_pylist() == [["a", "b", "c"]]


def test_list_column_multiple_rows() -> None:
    """Test list columns with multiple rows."""
    schema = pa.schema([
        pa.field("id", pa.utf8()),
        pa.field("tags", pa.list_(pa.utf8())),
    ])

    batch = _python_objects_to_record_batch(
        schema,
        {
            "id": ["row1", "row2"],
            "tags": [["a", "b"], ["c", "d", "e"]],
        },
    )

    assert batch is not None
    assert batch.num_rows == 2
    assert batch.column("id").to_pylist() == ["row1", "row2"]
    assert batch.column("tags").to_pylist() == [["a", "b"], ["c", "d", "e"]]


def test_non_list_column_mismatch_no_hint() -> None:
    """Test that non-list columns don't get the list-specific hint."""
    schema = pa.schema([
        pa.field("id", pa.utf8()),
        pa.field("value", pa.int32()),
    ])

    with pytest.raises(ValueError) as exc_info:
        _python_objects_to_record_batch(
            schema,
            {
                "id": "row1",
                "value": [1, 2, 3],
            },
        )

    error_message = str(exc_info.value)
    assert "Hint" not in error_message
