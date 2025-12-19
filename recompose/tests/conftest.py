"""Pytest configuration for recompose tests."""

import pytest


@pytest.fixture(autouse=True)
def reset_state(monkeypatch: pytest.MonkeyPatch):
    """Reset all state and disable colors for predictable test output."""
    from recompose.context import set_automation_context, set_context, set_recompose_context
    from recompose.output import reset_output_manager

    # Disable colors FIRST, before any output manager is created
    monkeypatch.setenv("NO_COLOR", "1")

    # Reset all context state
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()  # Now reset so it picks up NO_COLOR

    yield

    # Clean up after test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()
