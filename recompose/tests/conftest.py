"""Pytest configuration for recompose tests."""

import os

import pytest

# Set NO_COLOR before any imports happen (module-level, runs at conftest load time)
os.environ["NO_COLOR"] = "1"


@pytest.fixture(autouse=True)
def reset_state():
    """Reset all state between tests."""
    from recompose.context import set_automation_context, set_context, set_recompose_context
    from recompose.output import reset_output_manager

    # Reset all context state
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()

    yield

    # Clean up after test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()
