"""Pytest configuration for recompose tests."""

import pytest


@pytest.fixture(autouse=True)
def disable_colors(monkeypatch: pytest.MonkeyPatch):
    """Disable colors in tests to get predictable output."""
    monkeypatch.setenv("NO_COLOR", "1")


@pytest.fixture(autouse=True)
def reset_context():
    """Reset context state between tests."""
    from recompose.context import set_automation_context, set_context, set_recompose_context
    from recompose.output import reset_output_manager

    # Reset all context state before each test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()  # Reset so it picks up NO_COLOR
    yield
    # Clean up after test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()
