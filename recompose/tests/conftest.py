"""Pytest configuration for recompose tests."""

import pytest


@pytest.fixture(autouse=True)
def reset_context():
    """Reset context state between tests."""
    from recompose.context import set_automation_context, set_context, set_recompose_context

    # Reset all context state before each test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    yield
    # Clean up after test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
