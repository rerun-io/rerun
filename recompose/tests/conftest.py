"""Pytest configuration for recompose tests."""

import pytest

from . import flow_test_app


@pytest.fixture(autouse=True)
def setup_flow_test_app_context():
    """Set up the flow_test_app context for all tests.

    This ensures that tests which call flows from flow_test_app
    have the proper module-based entry point configured.
    """
    flow_test_app.app.setup_context()
    yield
