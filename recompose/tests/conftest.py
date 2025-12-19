"""Pytest configuration for recompose tests."""

import pytest


@pytest.fixture(autouse=True)
def reset_state(monkeypatch: pytest.MonkeyPatch):
    """Reset all state between tests and disable colors for CLI invocations."""
    from recompose.cli import _reset_console
    from recompose.context import set_automation_context, set_context, set_recompose_context
    from recompose.output import reset_output_manager

    # Disable colors for any CLI/subprocess invocations within tests
    # Must unset FORCE_COLOR because Rich ignores NO_COLOR when FORCE_COLOR is set
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")

    # Reset all context state and consoles so they pick up new env vars
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()
    _reset_console()

    yield

    # Clean up after test
    set_context(None)
    set_automation_context(None)
    set_recompose_context(None)
    reset_output_manager()
    _reset_console()
