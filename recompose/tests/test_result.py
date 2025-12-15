"""Tests for the Result type."""

import pytest

from recompose import Err, Ok, Result


def test_ok_creates_success_result():
    result = Ok(42)
    assert result.ok is True
    assert result.failed is False
    assert result.value() == 42
    assert result.error is None


def test_err_creates_failure_result():
    result = Err("something went wrong")
    assert result.ok is False
    assert result.failed is True
    assert result.error == "something went wrong"
    with pytest.raises(RuntimeError):
        result.value()


def test_err_with_traceback():
    result = Err("error", traceback="traceback info")
    assert result.traceback == "traceback info"


def test_result_is_immutable():
    result = Ok(42)
    with pytest.raises(Exception):  # Pydantic raises ValidationError
        result.status = "failure"  # Public fields are immutable


def test_value_success():
    result = Ok("hello")
    assert result.value() == "hello"


def test_value_failure_raises():
    result = Err("oops")
    with pytest.raises(RuntimeError, match="Attempted to get value from a failed result"):
        result.value()


def test_value_or_success():
    result = Ok(42)
    assert result.value_or(0) == 42


def test_value_or_failure():
    result: Result[int] = Err("oops")
    assert result.value_or(0) == 0


def test_ok_with_none_value():
    result = Ok(None)
    assert result.ok is True
    assert result.value() is None
