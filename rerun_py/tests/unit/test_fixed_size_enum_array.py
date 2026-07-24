from __future__ import annotations

import pyarrow as pa
import pytest
from test_types.datatypes import (
    FixedSizeEnumArray,
    FixedSizeEnumArrayBatch,
    FixedSizeWideEnumArray,
    FixedSizeWideEnumArrayBatch,
)


def test_fixed_size_enum_array_batch() -> None:
    a = FixedSizeEnumArray(["Up", "Down", "Right"])  # type: ignore[arg-type]
    b = FixedSizeEnumArray(["Left", "Forward", "Back"])  # type: ignore[arg-type]
    values = [[1, 2, 3], [4, 5, 6]]

    single = FixedSizeEnumArrayBatch(a).as_arrow_array()
    raw = FixedSizeEnumArrayBatch(values).as_arrow_array()  # type: ignore[arg-type]
    instances = FixedSizeEnumArrayBatch([a, b]).as_arrow_array()

    assert single.type.value_type == pa.uint8()

    assert single.to_pylist() == values[:1]
    assert raw.to_pylist() == values
    assert instances.to_pylist() == values


def test_fixed_size_enum_array_accepts_existing_instance() -> None:
    original = FixedSizeEnumArray(["Up", "Down", "Right"])  # type: ignore[arg-type]
    FixedSizeEnumArray(original)  # Does not raise an exception.


def test_fixed_size_enum_array_validates_length() -> None:
    with pytest.raises(ValueError, match="must be a 3-element array"):
        FixedSizeEnumArray(["Up", "Down", "Right", "Left"])  # type: ignore[arg-type]


def test_fixed_size_wide_enum_array_batch() -> None:
    a = FixedSizeWideEnumArray(["Low", "High"])  # type: ignore[arg-type]
    b = FixedSizeWideEnumArray(["High", "Low"])  # type: ignore[arg-type]
    values = [[1, 65536], [65536, 1]]

    single = FixedSizeWideEnumArrayBatch(a).as_arrow_array()
    raw = FixedSizeWideEnumArrayBatch(values).as_arrow_array()  # type: ignore[arg-type]
    instances = FixedSizeWideEnumArrayBatch([a, b]).as_arrow_array()

    assert single.type.value_type == pa.uint32()

    assert single.to_pylist() == values[:1]
    assert raw.to_pylist() == values
    assert instances.to_pylist() == values
