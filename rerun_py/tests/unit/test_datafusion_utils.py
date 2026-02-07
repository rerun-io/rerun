from __future__ import annotations

from datetime import datetime, timezone
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from datafusion import SessionContext
from rerun.utilities.datafusion.collect import collect_to_string_list

if TYPE_CHECKING:
    from datafusion import DataFrame


@pytest.fixture
def df() -> DataFrame:
    ctx = SessionContext()
    # create a RecordBatch and a new DataFrame from it
    batch = pa.RecordBatch.from_arrays(
        [
            pa.array(["Hello", "World", None], type=pa.string_view()),
            pa.array([4, None, 6]),
            pa.array([2.0, -1.0, None], type=pa.float32()),
            pa.array([
                datetime(2022, 12, 31, tzinfo=timezone.utc),
                datetime(2027, 6, 26, tzinfo=timezone.utc),
                None,
            ]),
            pa.array([False, None, True]),
        ],
        names=["a", "b", "c", "d", "e"],
    )
    return ctx.create_dataframe([[batch]])


@pytest.mark.parametrize(
    ("column_name", "remove_nulls", "expected_result"),
    [
        ("a", True, ["Hello", "World"]),
        ("a", False, ["Hello", "World", None]),
        ("b", False, ["4", None, "6"]),
        ("c", False, ["2.0", "-1.0", None]),
        ("d", False, ["2022-12-31 00:00:00+00:00", "2027-06-26 00:00:00+00:00", None]),
        ("e", False, ["False", None, "True"]),
    ],
)
def test_string_functions(df: DataFrame, column_name: str, remove_nulls: bool, expected_result: list[str]) -> None:
    assert collect_to_string_list(df, column_name, remove_nulls=remove_nulls) == expected_result
