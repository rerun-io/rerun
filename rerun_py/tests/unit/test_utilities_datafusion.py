from __future__ import annotations

import importlib
from unittest.mock import Mock, patch

import pytest
import rerun.utilities.datafusion.functions.url_generation
from rerun.error_utils import RerunOptionalDependencyError


def test_smoke() -> None:
    """Just check that we can import the module."""


def test_partition_url_import_normal() -> None:
    """Check that we can import partition_url when datafusion is available."""
    from rerun.utilities.datafusion.functions.url_generation import partition_url

    assert partition_url is not None


def test_partition_url_without_datafusion() -> None:
    """Check that calling partition_url raises RerunOptionalDependencyError when datafusion is unavailable."""
    # Mock the import to make datafusion unavailable
    with patch.dict("sys.modules", {"datafusion": None}):
        importlib.reload(rerun.utilities.datafusion.functions.url_generation)
        # Import the module - this should work
        from rerun.utilities.datafusion.functions.url_generation import partition_url

        # Create a mock dataset for testing
        mock_dataset = Mock()

        # But calling the function should raise an error
        with pytest.raises(RerunOptionalDependencyError, match="'datafusion' could not be imported"):
            partition_url(mock_dataset)
