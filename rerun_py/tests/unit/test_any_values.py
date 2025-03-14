from __future__ import annotations

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
from rerun.error_utils import RerunWarning


def test_any_value() -> None:
    values = rr.AnyValues(foo=[1.0, 2.0, 3.0], bar="hello")

    batches = list(values.as_component_batches())

    foo_batch = batches[0]
    bar_batch = batches[1]

    assert foo_batch.component_descriptor() == rr.ComponentDescriptor("foo")
    assert bar_batch.component_descriptor() == rr.ComponentDescriptor("bar")
    assert len(foo_batch.as_arrow_array()) == 3
    assert len(bar_batch.as_arrow_array()) == 1
    assert np.all(foo_batch.as_arrow_array().to_numpy() == np.array([1.0, 2.0, 3.0]))


def test_any_value_datatypes() -> None:
    values = rr.AnyValues(my_points=rr.datatypes.Vec2DBatch([(0, 1), (2, 3), (4, 5)]))

    batches = list(values.as_component_batches())

    foo_batch = batches[0]

    assert foo_batch.component_descriptor() == rr.ComponentDescriptor("my_points")
    assert len(foo_batch.as_arrow_array()) == 3


def test_bad_any_value() -> None:
    class Foo:
        pass

    with pytest.warns(RerunWarning) as warnings:
        values = rr.AnyValues(bad_data=[Foo()])

        batches = list(values.as_component_batches())

        assert len(batches) == 0
        assert len(warnings) == 1
        assert "Converting data for 'bad_data':" in str(warnings[0].message)

    with pytest.warns(RerunWarning) as warnings:
        values = rr.AnyValues(good_data=1)

        batches = list(values.as_component_batches())

        assert len(batches) == 1
        assert len(warnings) == 0

        # Now using a different type fails
        values = rr.AnyValues(good_data="foo")

        batches = list(values.as_component_batches())

        assert len(batches) == 0
        assert len(warnings) == 1

        assert "Converting data for 'good_data':" in str(warnings[0].message)


def test_none_any_value() -> None:
    with pytest.warns(RerunWarning) as warnings:
        # Log as None -- ignored with no warnings
        values = rr.AnyValues(none_data=None)
        batches = list(values.as_component_batches())

        assert len(batches) == 0
        assert len(warnings) == 0

        # Log as None -- ignored with warning
        values = rr.AnyValues(none_data=None, drop_untyped_nones=False)
        batches = list(values.as_component_batches())

        assert len(batches) == 0
        assert len(warnings) == 1

        assert (
            "Converting data for 'none_data': ValueError(Cannot convert None to arrow array. Type is unknown.)"
            in str(warnings[0].message)
        )

        # Log as not None
        values = rr.AnyValues(none_data=7, drop_untyped_nones=False)
        batches = list(values.as_component_batches())

        assert len(batches) == 1
        assert len(warnings) == 1  # no new warnings

        # Log as None is now logged successfully
        values = rr.AnyValues(none_data=None, drop_untyped_nones=False)
        batches = list(values.as_component_batches())

        assert len(batches) == 1
        assert len(warnings) == 1  # no new warnings


def test_iterable_any_value() -> None:
    SHORT_TEXT = "short"
    LONG_TEXT = "longer_text"

    SHORT_BYTES = b"ABCD"
    LONG_BYTES = b"ABCDEFGH"

    values = rr.AnyValues(str_values=SHORT_TEXT, bytes_values=SHORT_BYTES)
    batches = list(values.as_component_batches())

    assert len(batches) == 2
    assert batches[0].as_arrow_array() == pa.array([SHORT_TEXT], type=pa.string())
    assert batches[1].as_arrow_array() == pa.array([SHORT_BYTES], type=pa.binary())

    # Issue #8781 - ensure subsequent calls do not truncate data
    values = rr.AnyValues(str_values=LONG_TEXT, bytes_values=LONG_BYTES)
    batches = list(values.as_component_batches())

    assert len(batches) == 2
    assert batches[0].as_arrow_array() == pa.array([LONG_TEXT], type=pa.string())
    assert batches[1].as_arrow_array() == pa.array([LONG_BYTES], type=pa.binary())

    # Ensure iterables of these types are handled as arrays
    values = rr.AnyValues(str_values=[SHORT_TEXT, LONG_TEXT], bytes_values=[SHORT_BYTES, LONG_BYTES])
    batches = list(values.as_component_batches())

    assert len(batches) == 2
    assert batches[0].as_arrow_array() == pa.array([SHORT_TEXT, LONG_TEXT], type=pa.string())
    assert batches[1].as_arrow_array() == pa.array([SHORT_BYTES, LONG_BYTES], type=pa.binary())
