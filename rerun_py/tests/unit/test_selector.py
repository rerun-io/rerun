from __future__ import annotations

import pyarrow as pa
import pyarrow.compute as pc
import pytest
from rerun.experimental import Selector


def test_parse_and_execute_field() -> None:
    arr = pa.StructArray.from_arrays(
        [pa.array([1.0, 2.0, 3.0])],
        names=["x"],
    )
    result = Selector(".x").execute(arr)
    assert result is not None
    assert result == pa.array([1.0, 2.0, 3.0])


def test_nested_field() -> None:
    inner = pa.StructArray.from_arrays(
        [pa.array([10, 20])],
        names=["value"],
    )
    outer = pa.StructArray.from_arrays(
        [inner],
        names=["nested"],
    )
    result = Selector(".nested.value").execute(outer)
    assert result is not None
    assert result == pa.array([10, 20])


def test_invalid_query() -> None:
    with pytest.raises(ValueError, match="Failed to parse selector"):
        Selector("invalid query without dot")


def test_repr_and_str() -> None:
    assert "Selector" in repr(Selector(".field"))
    assert ".field" in str(Selector(".field"))


def test_pipe_with_callable() -> None:
    arr = pa.StructArray.from_arrays(
        [pa.array([1.0, 2.0, 3.0])],
        names=["x"],
    )
    result = Selector(".x").pipe(lambda a: pc.multiply(a, 2)).execute(arr)
    assert result is not None
    assert result == pa.array([2.0, 4.0, 6.0])


def test_pipe_with_plain_callable() -> None:
    arr = pa.StructArray.from_arrays(
        [pa.array([1.0, 2.0, 3.0])],
        names=["x"],
    )
    result = Selector(".x").pipe(lambda a: pa.array([v.as_py() ** 2 for v in a])).execute(arr)
    assert result is not None
    assert result == pa.array([1.0, 4.0, 9.0])


def test_pipe_with_selector() -> None:
    inner = pa.StructArray.from_arrays(
        [pa.array([10, 20])],
        names=["value"],
    )
    outer = pa.StructArray.from_arrays(
        [inner],
        names=["nested"],
    )
    result = Selector(".nested").pipe(Selector(".value")).execute(outer)
    assert result is not None
    assert result == pa.array([10, 20])


def test_pipe_callable_returning_none() -> None:
    arr = pa.StructArray.from_arrays(
        [pa.array([1, 2, 3])],
        names=["x"],
    )
    result = Selector(".x").pipe(lambda _a: None).execute(arr)
    assert result is None


def test_execute_per_row() -> None:
    inner_type = pa.struct([pa.field("value", pa.int64())])
    arr = pa.array(
        [
            [{"value": 1}, {"value": 2}],
            [{"value": 3}],
        ],
        type=pa.list_(inner_type),
    )
    result = Selector(".value").execute_per_row(arr)
    assert result is not None
    assert result == pa.array([[1, 2], [3]], type=pa.list_(pa.int64()))


def test_pipe_chaining() -> None:
    inner = pa.StructArray.from_arrays(
        [pa.array([5.0, 10.0])],
        names=["val"],
    )
    outer = pa.StructArray.from_arrays(
        [inner],
        names=["data"],
    )
    result = Selector(".data").pipe(Selector(".val")).pipe(lambda a: pc.add(a, 1)).execute(outer)
    assert result is not None
    assert result == pa.array([6.0, 11.0])
