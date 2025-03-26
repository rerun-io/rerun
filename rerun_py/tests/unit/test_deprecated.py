from __future__ import annotations

import warnings

import pytest
from rerun.error_utils import deprecated_param, set_strict_mode, strict_mode

was_strict_mode = strict_mode()
set_strict_mode(False)


# Define a function with a deprecated parameter for testing
@deprecated_param("old_param", use_instead="new_param", since="1.0")
def test_function(new_param: str | None = None, old_param: str | None = None) -> str | None:
    return new_param or old_param


def test_deprecated_param_warning() -> None:
    # Test that a warning is raised when the deprecated parameter is used
    with pytest.warns(DeprecationWarning) as record:
        result = test_function(old_param="value")

    # Check that exactly one warning was raised
    assert len(record) == 1

    # Check the warning message content
    warning_message = str(record[0].message)
    assert "old_param" in warning_message
    assert "test_function" in warning_message
    assert "use new_param instead" in warning_message
    assert "since version 1.0" in warning_message

    # Test that the function still works correctly
    assert result == "value"


def test_no_warning_without_deprecated_param() -> None:
    # Test that no warning is raised when the deprecated parameter is not used
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")  # Ensure all warnings are shown
        result = test_function(new_param="new_value")

    # Check that no warnings were raised
    assert len(record) == 0

    # Test that the function still works correctly
    assert result == "new_value"


def test_positional_args_handling() -> None:
    # Testing with positional arguments (where deprecated param isn't named)
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        result = test_function("positional_value")  # Passed to new_param

    # No warning should be raised
    assert len(record) == 0
    assert result == "positional_value"


set_strict_mode(was_strict_mode)
